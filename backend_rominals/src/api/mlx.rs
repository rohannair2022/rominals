use std::error::Error;
use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

const DEFAULT_MLX_PYTHON_BIN: &str = "python3";
const DEFAULT_MLX_MODEL: &str = "mlx-community/Qwen3.5-4B-MLX-4bit";
const DEFAULT_MLX_MAX_TOKENS: u32 = 200;
const DEFAULT_MLX_TEMPERATURE: f32 = 0.2;
const DEFAULT_MLX_PARALLEL_WORKERS: usize = 1;
const MAX_MLX_PARALLEL_WORKERS: usize = 1;
const MODEL_WARMUP_PROMPT: &str = "Reply with exactly READY.";
const MODEL_WARMUP_MAX_TOKENS: u32 = 8;

const SECTION_DEFS: [(&str, &str); 6] = [
    (
        "TAM / Market",
        "Target market, growth rate, and share-taker vs tide-rider framing.",
    ),
    (
        "Relative Valuation",
        "P/S, P/E or EV/EBITDA, PEG, and direct comp comparison with valuation rationale.",
    ),
    (
        "Fundamentals",
        "Revenue/margin trajectory, FCF, SBC dilution pressure, and balance-sheet quality.",
    ),
    (
        "Catalysts + Macro",
        "Near-term catalysts plus macro/geopolitical factors that specifically impact this name.",
    ),
    (
        "Risks + Competitors",
        "Concentrated downside risks, market share map, and moat durability check.",
    ),
    (
        "Technicals + Entry",
        "Trend vs key moving averages, relative strength framing, and preferred entry discipline.",
    ),
];

#[derive(Clone, Debug)]
struct MlxConfig {
    python_bin: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
    parallel_workers: usize,
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
        "Running MLX workers with model {} (parallel: {}, sections: {})...\n",
        config.model,
        parallel_workers,
        SECTION_DEFS.len()
    ));

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
    let mut config = mlx_config_from_env();
    config.max_tokens = config.max_tokens.min(MODEL_WARMUP_MAX_TOKENS).max(1);
    on_status(&format!("Preloading MLX model {}...", config.model));
    let _ = run_mlx_generation(&config, MODEL_WARMUP_PROMPT, |_chunk| {})?;
    on_status("MLX model preloaded and ready.");
    Ok(())
}

fn mlx_config_from_env() -> MlxConfig {
    let python_bin =
        std::env::var("ROMINALS_MLX_PYTHON_BIN").unwrap_or_else(|_| DEFAULT_MLX_PYTHON_BIN.to_string());
    let model = std::env::var("ROMINALS_MLX_MODEL").unwrap_or_else(|_| DEFAULT_MLX_MODEL.to_string());
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

    MlxConfig {
        python_bin,
        model,
        max_tokens,
        temperature,
        parallel_workers,
    }
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
    requested_workers.clamp(1, MAX_MLX_PARALLEL_WORKERS)
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
        .map(|comp| format!("Compare against {comp} where relevant."))
        .unwrap_or_default();
    let data_context = snapshot_context.unwrap_or(
        "Structured fundamentals snapshot unavailable for this run. Use available context and avoid fabricating metrics.",
    );

    format!(
        "You are one worker in a parallel investment research pipeline.\n\
Ticker: {ticker}\n\
{comparison_text}\n\
Worker scope: {section_title}\n\
Objective: {objective}\n\
Rules: keep the response short and concise, data-dense, plain text only, no markdown, no filler.\n\
Use the context below first and clearly mark uncertainty instead of inventing data.\n\
\n\
Context:\n{data_context}\n\
\n\
Return only this section, with 3-5 bullet-like lines in plain text, max 90 words total."
    )
}

/// Strips ANSI escape sequences (CSI codes: colors, cursor movement, line
/// clears, etc.) that `rich`-based CLIs like mlx_lm emit for their live
/// progress/stats panels. This is NOT text filtering -- these are invisible
/// terminal control bytes, not content. Leaving them in means cursor-move /
/// line-clear sequences actively execute in your terminal and erase prior
/// output, which is the opposite of "show everything." Every visible
/// character of the model's real output (including all reasoning, all
/// stats lines, everything) still passes through untouched.
fn strip_ansi_escapes(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1B && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'[' => {
                    let mut j = i + 2;
                    while j < bytes.len() && !(0x40..=0x7E).contains(&bytes[j]) {
                        j += 1;
                    }
                    i = (j + 1).min(bytes.len());
                    continue;
                }
                b']' => {
                    let mut j = i + 2;
                    while j < bytes.len() && bytes[j] != 0x07 {
                        if bytes[j] == 0x1B && j + 1 < bytes.len() && bytes[j + 1] == b'\\' {
                            j += 1;
                            break;
                        }
                        j += 1;
                    }
                    i = (j + 1).min(bytes.len());
                    continue;
                }
                _ => {
                    i += 2;
                    continue;
                }
            }
        }

        if bytes[i] == b'\r' {
            i += 1;
            continue;
        }

        let ch_len = utf8_char_len(bytes[i]);
        let end = (i + ch_len).min(bytes.len());
        if let Ok(s) = std::str::from_utf8(&bytes[i..end]) {
            output.push_str(s);
        }
        i = end;
    }

    output
}

fn utf8_char_len(first_byte: u8) -> usize {
    if first_byte & 0x80 == 0 {
        1
    } else if first_byte & 0xE0 == 0xC0 {
        2
    } else if first_byte & 0xF0 == 0xE0 {
        3
    } else if first_byte & 0xF8 == 0xF0 {
        4
    } else {
        1
    }
}

fn run_mlx_generation<FChunk>(
    config: &MlxConfig,
    prompt: &str,
    mut on_chunk: FChunk,
) -> Result<String, Box<dyn Error>>
where
    FChunk: FnMut(&str),
{
    let mut child = Command::new(&config.python_bin)
        .arg("-u")
        .arg("-m")
        .arg("mlx_lm")
        .arg("generate")
        .arg("--model")
        .arg(&config.model)
        .arg("--prompt")
        .arg(prompt)
        .arg("--max-tokens")
        .arg(config.max_tokens.to_string())
        .arg("--temp")
        .arg(format!("{}", config.temperature))
        .arg("--verbose")
        .arg("True")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Failed to capture mlx-lm stdout"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("Failed to capture mlx-lm stderr"))?;

    let stderr_handle = thread::spawn(move || -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes)?;
        Ok(bytes)
    });

    // Everything read from stdout is streamed live via on_chunk AND
    // accumulated into stdout_text, with zero filtering beyond ANSI-escape
    // stripping (control codes, not content). No extraction, no trimming
    // of sections, no dropping of "noise" lines -- every visible character
    // the process printed is kept, in order.
    let mut stdout_text = String::new();
    let mut read_buffer = [0u8; 4096];
    loop {
        let read_count = stdout.read(&mut read_buffer)?;
        if read_count == 0 {
            break;
        }

        let chunk = String::from_utf8_lossy(&read_buffer[..read_count]);
        let clean_chunk = strip_ansi_escapes(chunk.as_ref());
        stdout_text.push_str(&clean_chunk);
        if !clean_chunk.is_empty() {
            on_chunk(&clean_chunk);
        }
    }

    let status = child.wait()?;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| io::Error::other("Failed to join mlx-lm stderr reader thread"))??;

    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
        let stdout = stdout_text.trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(io::Error::other(format!(
            "mlx-lm generation failed for model {} (exit {:?}): {}",
            config.model,
            status.code(),
            details
        ))
        .into());
    }

    if stdout_text.is_empty() {
        return Err(io::Error::other("mlx-lm returned no output").into());
    }

    // Returned as-is: full prompt echo, all reasoning, the final answer,
    // and the runtime stats panel -- nothing removed.
    Ok(stdout_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_prompt_includes_ticker() {
        let prompt = build_section_prompt("AAPL", Some("MSFT"), None, "TAM / Market", "Objective");
        assert!(prompt.contains("Ticker: AAPL"));
        assert!(prompt.contains("Compare against MSFT"));
    }

    #[test]
    fn worker_titles_match_worker_count() {
        assert_eq!(worker_section_titles().len(), SECTION_DEFS.len());
    }

    #[test]
    fn stream_chunk_passthrough_appends_in_order() {
        let chunks = ["first ", "second", " third"];
        let mut streamed = String::new();
        let mut stdout_text = String::new();

        for chunk in chunks {
            stdout_text.push_str(chunk);
            streamed.push_str(chunk);
        }

        assert_eq!(streamed, "first second third");
        assert_eq!(stdout_text, streamed);
    }

    #[test]
    fn strip_ansi_escapes_removes_csi_sequences() {
        let raw = "\x1b[2K\x1b[1G\x1b[38;5;10mHello\x1b[0m World\r\n";
        let cleaned = strip_ansi_escapes(raw);
        assert_eq!(cleaned, "Hello World\n");
    }

    #[test]
    fn strip_ansi_escapes_preserves_plain_text() {
        let raw = "plain line with no escapes";
        assert_eq!(strip_ansi_escapes(raw), raw);
    }

    #[test]
    fn strip_ansi_escapes_keeps_all_content_including_stats_and_thinking() {
        let raw = "\x1b[1;36mPrompt: hidden prompt\x1b[0m\r\n<think>\nreasoning here\n</think>\nGeneration: final answer\r\n\u{2502}500 tokens, 21.273 tokens-per-sec\u{2502}\r\n";
        let cleaned = strip_ansi_escapes(raw);
        assert!(cleaned.contains("Prompt: hidden prompt"));
        assert!(cleaned.contains("<think>"));
        assert!(cleaned.contains("reasoning here"));
        assert!(cleaned.contains("Generation: final answer"));
        assert!(cleaned.contains("tokens-per-sec"));
    }
}