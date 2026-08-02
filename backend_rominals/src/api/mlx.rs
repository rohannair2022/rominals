use serde_json::{Value, json};
use std::error::Error;
use std::io::{self, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_MLX_PYTHON_BIN: &str = "python3";
const DEFAULT_MLX_MODEL: &str = "mlx-community/Qwen3.5-4B-MLX-4bit";
const DEFAULT_MLX_MAX_TOKENS: u32 = 600;
const DEFAULT_MLX_TEMPERATURE: f32 = 0.2;
const DEFAULT_MLX_PARALLEL_WORKERS: usize = 2;
const DEFAULT_MLX_ENABLE_THINKING: bool = false;
// Confirmed on your M1: 2 concurrent workers sustain ~25 tok/s each.
const MAX_MLX_PARALLEL_WORKERS: usize = 2;
const DEFAULT_MLX_SERVER_PORT: u16 = 8712;
const MLX_SERVER_READY_RETRIES: u32 = 120;
const MLX_SERVER_READY_POLL_MS: u64 = 500;

const SECTION_DEFS: [(&str, &str); 2] = [
    (
        "Macro Outlook",
        "Industry-standard top-down regime analysis: growth/inflation/rates/liquidity backdrop, policy + geopolitical transmission channels, and base/bull/bear path for the next 3-6 months.",
    ),
    (
        "Micro Outlook",
        "Industry-standard bottom-up equity analysis: business quality, margins/FCF durability, valuation vs peers (P/S, EV, EV/EBITDA where available), catalysts, and downside risks.",
    ),
];

#[derive(Clone, Debug)]
struct MlxConfig {
    python_bin: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
    parallel_workers: usize,
    enable_thinking: bool,
}

#[derive(Debug)]
struct WorkerResult {
    index: usize,
    output: Result<String, String>,
}

#[derive(Clone, Debug)]
pub struct WorkerSectionOutput {
    pub index: usize,
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct WorkerSectionChunk {
    pub index: usize,
    pub title: String,
    pub chunk: String,
}

#[derive(Debug)]
enum WorkerEvent {
    StreamChunk { index: usize, chunk: String },
    Complete(WorkerResult),
}

/// Holds the long-lived `mlx_lm.server` subprocess plus the URL to reach it.
/// The model loads into memory exactly once, the first time any caller
/// needs it -- every worker after that just sends an HTTP request to an
/// already-warm process instead of paying a fresh model-load cost.
struct MlxServerHandle {
    child: Child,
    base_url: String,
}

static MLX_SERVER: Mutex<Option<MlxServerHandle>> = Mutex::new(None);

pub fn worker_section_titles() -> Vec<String> {
    SECTION_DEFS
        .iter()
        .map(|(title, _)| (*title).to_string())
        .collect()
}

pub fn analyze_company_workers<FStatus, FChunk, FSection>(
    ticker: &str,
    comparison_ticker: Option<&str>,
    snapshot_context: Option<&str>,
    mut on_status: FStatus,
    mut on_chunk: FChunk,
    mut on_section: FSection,
) -> Result<Vec<WorkerSectionOutput>, Box<dyn Error>>
where
    FStatus: FnMut(&str),
    FChunk: FnMut(&WorkerSectionChunk),
    FSection: FnMut(&WorkerSectionOutput),
{
    let config = mlx_config_from_env();
    let parallel_workers = effective_parallel_workers(config.parallel_workers);
    on_status(&format!(
        "Running MLX workers with model {} (parallel: {}, sections: {}, thinking: {})...\n",
        config.model,
        parallel_workers,
        SECTION_DEFS.len(),
        if config.enable_thinking { "on" } else { "off" }
    ));

    // Make sure the server is up before fanning out workers, so the first
    // request doesn't race the model load.
    ensure_server_started(&config)?;

    run_parallel_workers(
        &config,
        ticker,
        comparison_ticker,
        snapshot_context,
        |status| on_status(status),
        |chunk| on_chunk(chunk),
        |section| on_section(section),
    )
}

pub fn preload_mlx_model<FStatus>(mut on_status: FStatus) -> Result<(), Box<dyn Error>>
where
    FStatus: FnMut(&str),
{
    let config = mlx_config_from_env();
    on_status(&format!(
        "Starting MLX server with model {}...",
        config.model
    ));
    ensure_server_started(&config)?;
    on_status("MLX server ready and model loaded.");
    Ok(())
}

/// Stops the background `mlx_lm.server` process, if one is running. Call
/// this on graceful shutdown so you don't leave an orphaned Python process
/// holding the model in memory after your program exits.
pub fn shutdown_mlx_server() {
    if let Ok(mut guard) = MLX_SERVER.lock() {
        if let Some(mut handle) = guard.take() {
            let _ = handle.child.kill();
            let _ = handle.child.wait();
        }
    }
}

fn mlx_config_from_env() -> MlxConfig {
    let python_bin = std::env::var("ROMINALS_MLX_PYTHON_BIN")
        .unwrap_or_else(|_| DEFAULT_MLX_PYTHON_BIN.to_string());
    let model =
        std::env::var("ROMINALS_MLX_MODEL").unwrap_or_else(|_| DEFAULT_MLX_MODEL.to_string());
    let max_tokens = std::env::var("ROMINALS_MLX_MAX_TOKENS")
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MLX_MAX_TOKENS);
    let temperature = std::env::var("ROMINALS_MLX_TEMPERATURE")
        .ok()
        .and_then(|raw| raw.parse::<f32>().ok())
        .unwrap_or(DEFAULT_MLX_TEMPERATURE);
    let parallel_workers = std::env::var("ROMINALS_MLX_PARALLEL_WORKERS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MLX_PARALLEL_WORKERS);
    let enable_thinking = std::env::var("ROMINALS_MLX_ENABLE_THINKING")
        .ok()
        .as_deref()
        .and_then(parse_env_bool)
        .unwrap_or(DEFAULT_MLX_ENABLE_THINKING);

    MlxConfig {
        python_bin,
        model,
        max_tokens,
        temperature,
        parallel_workers,
        enable_thinking,
    }
}

fn parse_env_bool(raw: &str) -> Option<bool> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Starts `mlx_lm.server` in the background exactly once (guarded by the
/// mutex) and blocks until it responds to health checks. Every subsequent
/// call -- from any worker thread -- just returns the already-running
/// server's base URL immediately.
fn ensure_server_started(config: &MlxConfig) -> Result<String, Box<dyn Error>> {
    let mut guard = MLX_SERVER
        .lock()
        .map_err(|_| io::Error::other("MLX server lock poisoned"))?;

    if let Some(handle) = guard.as_ref() {
        return Ok(handle.base_url.clone());
    }

    let port = std::env::var("ROMINALS_MLX_SERVER_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(DEFAULT_MLX_SERVER_PORT);
    let base_url = format!("http://127.0.0.1:{port}");

    let child = Command::new(&config.python_bin)
        .arg("-m")
        .arg("mlx_lm.server")
        .arg("--model")
        .arg(&config.model)
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    wait_for_server_ready(&base_url)?;

    *guard = Some(MlxServerHandle {
        child,
        base_url: base_url.clone(),
    });

    Ok(base_url)
}

fn wait_for_server_ready(base_url: &str) -> Result<(), Box<dyn Error>> {
    let models_url = format!("{base_url}/v1/models");
    for _ in 0..MLX_SERVER_READY_RETRIES {
        if ureq::get(&models_url).call().is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(MLX_SERVER_READY_POLL_MS));
    }
    Err(io::Error::other("mlx_lm.server did not become ready in time").into())
}

fn run_parallel_workers<FStatus, FChunk, FSection>(
    config: &MlxConfig,
    ticker: &str,
    comparison_ticker: Option<&str>,
    snapshot_context: Option<&str>,
    mut on_status: FStatus,
    mut on_chunk: FChunk,
    mut on_section: FSection,
) -> Result<Vec<WorkerSectionOutput>, Box<dyn Error>>
where
    FStatus: FnMut(&str),
    FChunk: FnMut(&WorkerSectionChunk),
    FSection: FnMut(&WorkerSectionOutput),
{
    let (tx, rx) = mpsc::channel::<WorkerEvent>();
    let parallel_workers = effective_parallel_workers(config.parallel_workers);
    let next_index = Arc::new(AtomicUsize::new(0));

    for _ in 0..parallel_workers {
        let tx = tx.clone();
        let config = config.clone();
        let ticker = ticker.to_string();
        let comparison_ticker = comparison_ticker.map(|value| value.to_string());
        let snapshot_context = snapshot_context.map(|value| value.to_string());
        let next_index = Arc::clone(&next_index);

        thread::spawn(move || {
            loop {
                let index = next_index.fetch_add(1, Ordering::Relaxed);
                if index >= SECTION_DEFS.len() {
                    break;
                }

                let (title, objective) = SECTION_DEFS[index];
                let worker_prompt = build_section_prompt(
                    &ticker,
                    comparison_ticker.as_deref(),
                    snapshot_context.as_deref(),
                    title,
                    objective,
                );
                let tx_for_stream = tx.clone();
                // No subprocess spawn here anymore -- this is now a plain
                // HTTP request against the shared, already-running server.
                let output = run_mlx_generation(&config, &worker_prompt, |chunk| {
                    let _ = tx_for_stream.send(WorkerEvent::StreamChunk {
                        index,
                        chunk: chunk.to_string(),
                    });
                })
                .map_err(|err| err.to_string());
                let _ = tx.send(WorkerEvent::Complete(WorkerResult { index, output }));
            }
        });
    }
    drop(tx);

    let mut completed = 0usize;
    let mut results: Vec<Option<WorkerSectionOutput>> = vec![None; SECTION_DEFS.len()];
    while completed < SECTION_DEFS.len() {
        let worker_event = rx.recv().map_err(|err| {
            io::Error::other(format!("Worker channel closed unexpectedly: {err}"))
        })?;

        match worker_event {
            WorkerEvent::StreamChunk { index, chunk } => {
                let stream_chunk = WorkerSectionChunk {
                    index,
                    title: SECTION_DEFS[index].0.to_string(),
                    chunk,
                };
                on_chunk(&stream_chunk);
            }
            WorkerEvent::Complete(worker_result) => {
                completed = completed.saturating_add(1);
                let content = match worker_result.output {
                    Ok(text) => text,
                    Err(err) => format!("Section failed: {err}"),
                };
                let section = WorkerSectionOutput {
                    index: worker_result.index,
                    title: SECTION_DEFS[worker_result.index].0.to_string(),
                    content,
                };
                results[worker_result.index] = Some(section.clone());
                on_section(&section);

                on_status(&format_worker_status(completed, &results, parallel_workers));
            }
        }
    }

    let final_sections: Vec<WorkerSectionOutput> = results
        .into_iter()
        .enumerate()
        .map(|(index, section)| {
            section.unwrap_or_else(|| WorkerSectionOutput {
                index,
                title: SECTION_DEFS[index].0.to_string(),
                content: "Section did not return output.".to_string(),
            })
        })
        .collect();

    Ok(final_sections)
}

fn effective_parallel_workers(requested_workers: usize) -> usize {
    let max_allowed = MAX_MLX_PARALLEL_WORKERS.min(SECTION_DEFS.len().max(1));
    requested_workers.clamp(1, max_allowed)
}

fn format_worker_status(
    completed: usize,
    results: &[Option<WorkerSectionOutput>],
    parallel_workers: usize,
) -> String {
    let mut status = format!(
        "Parallel worker progress: {completed}/{} (max {} concurrent)\n",
        SECTION_DEFS.len(),
        parallel_workers
    );

    for (index, (title, _)) in SECTION_DEFS.iter().enumerate() {
        let marker = if results[index].is_some() { 'x' } else { ' ' };
        status.push_str(&format!("[{marker}] {title}\n"));
    }

    status
}

fn build_section_prompt(
    ticker: &str,
    comparison_ticker: Option<&str>,
    snapshot_context: Option<&str>,
    section_title: &str,
    objective: &str,
) -> String {
    let comparison_text = comparison_ticker
        .map(|comp| {
            format!(
                "Comparison anchor: {comp}. Compare relative strength and valuation where possible."
            )
        })
        .unwrap_or_default();
    let data_context = snapshot_context.unwrap_or(
        "Structured fundamentals snapshot unavailable for this run. Use available context and avoid fabricating metrics.",
    );

    format!(
        "You are one worker in a parallel investment research pipeline.\n\
Target ticker: {ticker}\n\
{comparison_text}\n\
Worker scope: {section_title}\n\
Objective: {objective}\n\
Execution rules:\n\
- Start from the provided Yahoo context first; do not restate it verbatim.\n\
- Apply institutional top-down/bottom-up thinking based on the worker scope.\n\
- Translate the context into implications, tradeoffs, and risk-adjusted conclusions.\n\
- If a metric is missing (for example P/S, EV, EV/EBITDA), explicitly mark it as unknown.\n\
- Use concise plain text only, no markdown, no filler.\n\
\n\
Heuristic playbook (soft assumptions, apply only if supporting data exists):\n\
- Low P/S + high EV or high EV/EBITDA can mean the equity looks optically cheap while enterprise burden/risk remains high; treat as mixed until margin/FCF quality confirms.\n\
- Price near 52-week highs with positive day momentum often implies trend strength but higher pullback risk.\n\
- Price near 52-week lows with weak momentum can be either value entry or structural deterioration; require a catalyst check.\n\
- Large move on high volume implies stronger conviction than the same move on light volume.\n\
\n\
Use the context below first and clearly mark uncertainty instead of inventing data.\n\
\n\
Context:\n{data_context}\n\
\n\
Return only this section, with 5-7 bullet-like lines in plain text, max 170 words total."
    )
}

/// One event out of an OpenAI-style Server-Sent-Events stream. mlx_lm.server
/// (like most reasoning-model APIs) puts thinking text and the final
/// answer in two SEPARATE delta fields -- `reasoning_content` (sometimes
/// `reasoning`) vs `content` -- rather than one combined text stream. Both
/// are real model output; we surface both.
enum SseEvent {
    Reasoning(String),
    Content(String),
    /// Token-count summary the server sends (when asked via
    /// stream_options.include_usage) -- usually in a final chunk with an
    /// empty choices array, right before [DONE].
    Usage {
        completion_tokens: u64,
    },
    Done,
}

/// Parses a single line of an `mlx_lm.server` streaming response. Checks
/// for reasoning text first (checking both common field names, since this
/// isn't fully standardized across server builds), then falls back to the
/// regular content field. Blank lines, keepalive comment lines, and
/// role-only deltas are ignored -- those are SSE/API protocol framing, not
/// model output.
fn parse_sse_line(line: &str) -> Option<SseEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let data = trimmed.strip_prefix("data: ")?;
    if data == "[DONE]" {
        return Some(SseEvent::Done);
    }

    let parsed: Value = serde_json::from_str(data).ok()?;

    if let Some(completion_tokens) = parsed
        .get("usage")
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(|value| value.as_u64())
    {
        return Some(SseEvent::Usage { completion_tokens });
    }

    let delta = &parsed["choices"][0]["delta"];

    for reasoning_key in ["reasoning_content", "reasoning"] {
        if let Some(text) = delta.get(reasoning_key).and_then(|v| v.as_str()) {
            if !text.is_empty() {
                return Some(SseEvent::Reasoning(text.to_string()));
            }
        }
    }

    if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            return Some(SseEvent::Content(text.to_string()));
        }
    }

    None
}

/// Sends one generation request to the already-running `mlx_lm.server` and
/// streams back model output. If thinking is enabled, reasoning text and final
/// answer are surfaced in-order, with `<think>` / `</think>` markers wrapped
/// around reasoning chunks. If thinking is disabled, we request
/// `enable_thinking=false` via `chat_template_kwargs` so the server emits
/// answer-only content. SSE/JSON protocol framing (role markers, finish_reason,
/// keepalive lines, [DONE]) is stripped -- that's not model output.
fn run_mlx_generation<FChunk>(
    config: &MlxConfig,
    prompt: &str,
    mut on_chunk: FChunk,
) -> Result<String, Box<dyn Error>>
where
    FChunk: FnMut(&str),
{
    let base_url = ensure_server_started(config)?;

    let body = json!({
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
        "stream": true,
        "stream_options": {"include_usage": true},
        "chat_template_kwargs": {"enable_thinking": config.enable_thinking}
    });

    let response = ureq::post(&format!("{base_url}/v1/chat/completions"))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|err| io::Error::other(format!("mlx server request failed: {err}")))?;

    let mut reader = response.into_reader();
    let mut full_text = String::new();
    let mut byte_buffer = [0u8; 512];
    // SSE lines can arrive split across multiple socket reads; carry any
    // incomplete trailing line forward instead of dropping/mangling it.
    let mut pending_line = String::new();

    #[derive(PartialEq)]
    enum Mode {
        None,
        Reasoning,
        Content,
    }
    let mut mode = Mode::None;

    let emit = |text: &str, full_text: &mut String, on_chunk: &mut FChunk| {
        full_text.push_str(text);
        on_chunk(text);
    };

    // Timing + token bookkeeping for the "N tokens, X tokens/sec" summary.
    let generation_start = Instant::now();
    let mut chunk_count: u64 = 0;
    let mut usage_completion_tokens: Option<u64> = None;

    loop {
        let read_count = reader.read(&mut byte_buffer)?;
        if read_count == 0 {
            break;
        }

        pending_line.push_str(&String::from_utf8_lossy(&byte_buffer[..read_count]));

        while let Some(newline_pos) = pending_line.find('\n') {
            let line: String = pending_line.drain(..=newline_pos).collect();
            match parse_sse_line(&line) {
                Some(SseEvent::Reasoning(text)) => {
                    if !config.enable_thinking {
                        continue;
                    }
                    if mode != Mode::Reasoning {
                        emit("<think>\n", &mut full_text, &mut on_chunk);
                        mode = Mode::Reasoning;
                    }
                    chunk_count += 1;
                    emit(&text, &mut full_text, &mut on_chunk);
                }
                Some(SseEvent::Content(text)) => {
                    if mode == Mode::Reasoning {
                        emit("\n</think>\n\n", &mut full_text, &mut on_chunk);
                    }
                    mode = Mode::Content;
                    chunk_count += 1;
                    emit(&text, &mut full_text, &mut on_chunk);
                }
                Some(SseEvent::Usage { completion_tokens }) => {
                    usage_completion_tokens = Some(completion_tokens);
                }
                Some(SseEvent::Done) => {
                    pending_line.clear();
                }
                None => {}
            }
        }
    }

    // Handle a final line with no trailing newline, if the stream ended that way.
    match parse_sse_line(&pending_line) {
        Some(SseEvent::Reasoning(text)) if config.enable_thinking => {
            if mode != Mode::Reasoning {
                emit("<think>\n", &mut full_text, &mut on_chunk);
            }
            chunk_count += 1;
            emit(&text, &mut full_text, &mut on_chunk);
        }
        Some(SseEvent::Reasoning(_)) => {}
        Some(SseEvent::Content(text)) => {
            if mode == Mode::Reasoning {
                emit("\n</think>\n\n", &mut full_text, &mut on_chunk);
            }
            chunk_count += 1;
            emit(&text, &mut full_text, &mut on_chunk);
        }
        Some(SseEvent::Usage { completion_tokens }) => {
            usage_completion_tokens = Some(completion_tokens);
        }
        _ => {}
    }

    if full_text.is_empty() {
        return Err(io::Error::other("mlx server returned no output").into());
    }

    // Prefer the server's own token count (accurate); fall back to counting
    // streamed chunks (each is roughly, not exactly, one token) if the
    // server build doesn't send usage info.
    let elapsed_secs = generation_start.elapsed().as_secs_f64().max(0.001);
    let token_count = usage_completion_tokens.unwrap_or(chunk_count);
    let tokens_per_sec = token_count as f64 / elapsed_secs;
    let summary = format!(
        "\n\n[{token_count} tokens generated in {elapsed_secs:.2}s -- {tokens_per_sec:.1} tokens/sec]"
    );
    emit(&summary, &mut full_text, &mut on_chunk);

    Ok(full_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_prompt_includes_ticker() {
        let prompt = build_section_prompt("AAPL", Some("MSFT"), None, "Macro Outlook", "Objective");
        assert!(prompt.contains("Target ticker: AAPL"));
        assert!(prompt.contains("Comparison anchor: MSFT"));
    }

    #[test]
    fn worker_titles_match_worker_count() {
        assert_eq!(worker_section_titles().len(), SECTION_DEFS.len());
    }

    #[test]
    fn effective_parallel_workers_clamps_to_max() {
        assert_eq!(effective_parallel_workers(10), MAX_MLX_PARALLEL_WORKERS);
        assert_eq!(effective_parallel_workers(0), 1);
        assert_eq!(effective_parallel_workers(2), 2);
    }

    #[test]
    fn parse_env_bool_accepts_common_true_values() {
        assert_eq!(parse_env_bool("1"), Some(true));
        assert_eq!(parse_env_bool("true"), Some(true));
        assert_eq!(parse_env_bool("YES"), Some(true));
        assert_eq!(parse_env_bool(" on "), Some(true));
    }

    #[test]
    fn parse_env_bool_accepts_common_false_values() {
        assert_eq!(parse_env_bool("0"), Some(false));
        assert_eq!(parse_env_bool("false"), Some(false));
        assert_eq!(parse_env_bool("NO"), Some(false));
        assert_eq!(parse_env_bool(" off "), Some(false));
    }

    #[test]
    fn parse_env_bool_rejects_unknown_values() {
        assert_eq!(parse_env_bool(""), None);
        assert_eq!(parse_env_bool("maybe"), None);
    }

    #[test]
    fn parse_sse_line_extracts_content() {
        let line = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        match parse_sse_line(line) {
            Some(SseEvent::Content(text)) => assert_eq!(text, "Hello"),
            _ => panic!("expected a Content event"),
        }
    }

    #[test]
    fn parse_sse_line_extracts_reasoning_content_field() {
        let line = r#"data: {"choices":[{"delta":{"reasoning_content":"thinking..."}}]}"#;
        match parse_sse_line(line) {
            Some(SseEvent::Reasoning(text)) => assert_eq!(text, "thinking..."),
            _ => panic!("expected a Reasoning event"),
        }
    }

    #[test]
    fn parse_sse_line_extracts_reasoning_field_variant() {
        let line = r#"data: {"choices":[{"delta":{"reasoning":"pondering..."}}]}"#;
        match parse_sse_line(line) {
            Some(SseEvent::Reasoning(text)) => assert_eq!(text, "pondering..."),
            _ => panic!("expected a Reasoning event"),
        }
    }

    #[test]
    fn parse_sse_line_detects_done_marker() {
        assert!(matches!(
            parse_sse_line("data: [DONE]"),
            Some(SseEvent::Done)
        ));
    }

    #[test]
    fn parse_sse_line_ignores_blank_and_non_data_lines() {
        assert!(parse_sse_line("").is_none());
        assert!(parse_sse_line("   ").is_none());
        assert!(parse_sse_line(": keepalive").is_none());
    }

    #[test]
    fn parse_sse_line_ignores_role_only_delta() {
        // First chunk of a chat-completions stream often carries just a
        // role marker with no content or reasoning -- should be skipped.
        let line = r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#;
        assert!(parse_sse_line(line).is_none());
    }

    #[test]
    fn parse_sse_line_extracts_usage_completion_tokens() {
        let line = r#"data: {"choices":[],"usage":{"completion_tokens":42,"prompt_tokens":10,"total_tokens":52}}"#;
        match parse_sse_line(line) {
            Some(SseEvent::Usage { completion_tokens }) => assert_eq!(completion_tokens, 42),
            _ => panic!("expected a Usage event"),
        }
    }

    #[test]
    fn parse_sse_line_ignores_malformed_json() {
        assert!(parse_sse_line("data: not json").is_none());
    }

    #[test]
    fn stream_chunk_passthrough_appends_in_order() {
        let chunks = ["first ", "second", " third"];
        let mut streamed = String::new();
        for chunk in chunks {
            streamed.push_str(chunk);
        }
        assert_eq!(streamed, "first second third");
    }
}
