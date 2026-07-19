use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{self, BufRead, BufReader};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gpt-oss:120b-cloud";

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

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    stream: bool,
    messages: Vec<RequestMessage<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<WebSearchTool>,
}

#[derive(Debug, Serialize)]
struct RequestMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct WebSearchTool {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ResponseMessage>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatStreamChunk {
    #[serde(default)]
    done: bool,
    message: Option<ResponseMessage>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    #[allow(dead_code)]
    id: Option<String>,
}

struct StreamOutcome {
    content: String,
}

struct WorkerResult {
    index: usize,
    output: Result<String, String>,
}

pub fn analyze_company_streaming<F>(
    ticker: &str,
    comparison_ticker: Option<&str>,
    alpha_vantage_context: Option<&str>,
    mut on_partial: F,
) -> Result<String, Box<dyn Error>>
where
    F: FnMut(&str),
{
    let host =
        std::env::var("ROMINALS_OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string());
    let model =
        std::env::var("ROMINALS_OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());
    let url = format!("{}/api/chat", host.trim_end_matches('/'));

    let client = reqwest::blocking::Client::builder()
        .user_agent("rominals-ollama/0.1")
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    on_partial("Running parallel research workers...\n");
    let section_outputs = run_parallel_workers(
        &client,
        &url,
        &model,
        ticker,
        comparison_ticker,
        alpha_vantage_context,
        |status| {
            on_partial(status);
        },
    )?;

    on_partial("Workers complete. Streaming final consolidated thesis...\n");
    let synthesis_prompt = build_synthesis_prompt(
        ticker,
        comparison_ticker,
        alpha_vantage_context,
        &section_outputs,
    );

    let stream = stream_chat_request(&client, &url, &model, &synthesis_prompt, true, |chunk| {
        on_partial(chunk);
    })?;

    if !stream.content.is_empty() {
        return Ok(stream.content);
    }

    let fallback = run_chat_with_fallback(&client, &url, &model, &synthesis_prompt)?;
    if !fallback.is_empty() {
        on_partial(&fallback);
        return Ok(fallback);
    }

    Err(io::Error::other("Ollama returned an empty final thesis").into())
}

fn run_parallel_workers<F>(
    client: &reqwest::blocking::Client,
    url: &str,
    model: &str,
    ticker: &str,
    comparison_ticker: Option<&str>,
    alpha_vantage_context: Option<&str>,
    mut on_status: F,
) -> Result<Vec<(String, String)>, Box<dyn Error>>
where
    F: FnMut(&str),
{
    let (tx, rx) = mpsc::channel::<WorkerResult>();

    for (index, (title, objective)) in SECTION_DEFS.iter().enumerate() {
        let tx = tx.clone();
        let worker_prompt = build_section_prompt(
            ticker,
            comparison_ticker,
            alpha_vantage_context,
            title,
            objective,
        );
        let client = client.clone();
        let url = url.to_string();
        let model = model.to_string();

        thread::spawn(move || {
            let output = run_chat_with_fallback(&client, &url, &model, &worker_prompt)
                .map_err(|err| err.to_string());

            let _ = tx.send(WorkerResult { index, output });
        });
    }
    drop(tx);

    let mut completed = 0usize;
    let mut results: Vec<Option<String>> = vec![None; SECTION_DEFS.len()];
    while completed < SECTION_DEFS.len() {
        let worker_result = rx.recv().map_err(|err| {
            io::Error::other(format!("Worker channel closed unexpectedly: {err}"))
        })?;

        completed = completed.saturating_add(1);
        let rendered = match worker_result.output {
            Ok(text) => text,
            Err(err) => format!("Section failed: {err}"),
        };
        results[worker_result.index] = Some(rendered);

        on_status(&format_worker_status(completed, &results));
    }

    let final_sections = results
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let title = SECTION_DEFS[index].0.to_string();
            let text = text.unwrap_or_else(|| "Section did not return output.".to_string());
            (title, text)
        })
        .collect();

    Ok(final_sections)
}

fn format_worker_status(completed: usize, results: &[Option<String>]) -> String {
    let mut status = format!(
        "Parallel worker progress: {completed}/{}\n",
        SECTION_DEFS.len()
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
    alpha_vantage_context: Option<&str>,
    section_title: &str,
    objective: &str,
) -> String {
    let comparison_text = comparison_ticker
        .map(|comp| format!("Compare against {comp} where relevant."))
        .unwrap_or_default();
    let data_context = alpha_vantage_context
        .unwrap_or("Alpha Vantage context unavailable for this run. Use web search as needed.");

    format!(
        "You are one worker in a parallel investment research pipeline.\n\
Ticker: {ticker}\n\
{comparison_text}\n\
Worker scope: {section_title}\n\
Objective: {objective}\n\
Rules: concise, data-dense, plain text only, no markdown, no filler.\n\
Use the context below first and web search only when needed to refresh stale metrics.\n\
\n\
Context:\n{data_context}\n\
\n\
Return only this section, with 4-8 bullet-like lines in plain text."
    )
}

fn build_synthesis_prompt(
    ticker: &str,
    comparison_ticker: Option<&str>,
    alpha_vantage_context: Option<&str>,
    sections: &[(String, String)],
) -> String {
    let subject = match comparison_ticker {
        Some(comp) => format!("{ticker} vs. comp {comp}"),
        None => ticker.to_string(),
    };
    let data_context = alpha_vantage_context.unwrap_or("Alpha Vantage context unavailable.");

    let mut section_block = String::new();
    for (title, text) in sections {
        section_block.push_str(&format!("{title}\n{text}\n\n"));
    }

    format!(
        "Analyze {subject} as a potential long. Be concise, data-dense, no filler. \
Use current numbers (search if needed) and prioritize the structured context below. \
Return plain text only (no markdown syntax such as **, //, #, or backticks).\n\
\n\
Alpha Vantage context:\n{data_context}\n\
\n\
Parallel worker outputs:\n{section_block}\n\
\n\
Final required structure:\n\
TAM/Market\n\
Relative Valuation\n\
Fundamentals\n\
Catalysts\n\
Macro\n\
Risks\n\
Technicals (brief)\n\
Competitors\n\
Alpha / What's Mispriced\n\
Verdict: Long / No / Watch-list, with strongest reason and what changes your mind."
    )
}

fn run_chat_with_fallback(
    client: &reqwest::blocking::Client,
    url: &str,
    model: &str,
    prompt: &str,
) -> Result<String, Box<dyn Error>> {
    let first = send_chat_request(client, url, model, prompt, true)?;
    let first_content = format_analysis_text(&first.content);
    if !first_content.is_empty() {
        return Ok(first_content);
    }

    if !first.tool_calls.is_empty() {
        let fallback_prompt = format!(
            "{prompt}\nIf external tools are unavailable, respond directly with your best synthesis."
        );
        let second = send_chat_request(client, url, model, &fallback_prompt, false)?;
        let second_content = format_analysis_text(&second.content);
        if !second_content.is_empty() {
            return Ok(second_content);
        }
    }

    Err(io::Error::other("Ollama worker returned empty content").into())
}

fn stream_chat_request<F>(
    client: &reqwest::blocking::Client,
    url: &str,
    model: &str,
    prompt: &str,
    include_web_search_tool: bool,
    mut on_partial: F,
) -> Result<StreamOutcome, Box<dyn Error>>
where
    F: FnMut(&str),
{
    let request = ChatRequest {
        model,
        stream: true,
        messages: vec![RequestMessage {
            role: "user",
            content: prompt,
        }],
        tools: if include_web_search_tool {
            vec![WebSearchTool { kind: "web_search" }]
        } else {
            Vec::new()
        },
    };

    let resp = client.post(url).json(&request).send()?;
    if !resp.status().is_success() {
        let status = resp.status();
        let error_body = resp
            .text()
            .unwrap_or_else(|err| format!("<failed to read error body: {err}>"));
        return Err(io::Error::other(format!(
            "HTTP {status} from Ollama analysis endpoint: {error_body}"
        ))
        .into());
    }

    let mut saw_tool_calls = false;
    let mut raw_output = String::new();
    let mut raw_thinking = String::new();
    let reader = BufReader::new(resp);

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let chunk: ChatStreamChunk = serde_json::from_str(line).map_err(|err| {
            io::Error::other(format!(
                "Failed to decode Ollama stream chunk: {err}. chunk={line}"
            ))
        })?;

        if let Some(error) = chunk.error {
            return Err(io::Error::other(format!("Ollama analysis error: {error}")).into());
        }

        if let Some(message) = chunk.message {
            if !message.tool_calls.is_empty() {
                saw_tool_calls = true;
            }

            if !message.thinking.is_empty() {
                raw_thinking.push_str(&message.thinking);
            }

            if !message.content.is_empty() {
                raw_output.push_str(&message.content);
                let formatted = format_analysis_text(&raw_output);
                if !formatted.is_empty() {
                    on_partial(&formatted);
                }
            } else if raw_output.is_empty() && !raw_thinking.is_empty() {
                let staged = format!(
                    "Streaming model reasoning...\n\n{}",
                    format_analysis_text(&raw_thinking)
                );
                on_partial(&staged);
            }
        }

        if chunk.done {
            break;
        }
    }

    let content = if raw_output.is_empty() {
        format_analysis_text(&raw_thinking)
    } else {
        format_analysis_text(&raw_output)
    };

    let _ = saw_tool_calls;
    Ok(StreamOutcome { content })
}

fn send_chat_request(
    client: &reqwest::blocking::Client,
    url: &str,
    model: &str,
    prompt: &str,
    include_web_search_tool: bool,
) -> Result<ResponseMessage, Box<dyn Error>> {
    let request = ChatRequest {
        model,
        stream: false,
        messages: vec![RequestMessage {
            role: "user",
            content: prompt,
        }],
        tools: if include_web_search_tool {
            vec![WebSearchTool { kind: "web_search" }]
        } else {
            Vec::new()
        },
    };

    let resp = client.post(url).json(&request).send()?;
    if !resp.status().is_success() {
        let status = resp.status();
        let error_body = resp
            .text()
            .unwrap_or_else(|err| format!("<failed to read error body: {err}>"));
        return Err(io::Error::other(format!(
            "HTTP {status} from Ollama analysis endpoint: {error_body}"
        ))
        .into());
    }

    let body: ChatResponse = resp.json()?;
    if let Some(error) = body.error {
        return Err(io::Error::other(format!("Ollama analysis error: {error}")).into());
    }

    body.message.ok_or_else(|| {
        io::Error::other("Ollama analysis response did not include a message body").into()
    })
}

fn format_analysis_text(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n");
    let mut cleaned_lines = Vec::new();

    for source_line in normalized.lines() {
        let mut line = source_line
            .replace("**", "")
            .replace("__", "")
            .replace('`', "");

        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with('#') {
            line = trimmed_start
                .trim_start_matches('#')
                .trim_start()
                .to_string();
        }

        let trimmed = line.trim();
        if trimmed == "//" {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("// ") {
            line = rest.to_string();
        }

        cleaned_lines.push(line.trim_end().to_string());
    }

    let mut output = String::new();
    let mut pending_blank = false;
    for line in cleaned_lines {
        if line.is_empty() {
            if pending_blank {
                continue;
            }
            pending_blank = true;
        } else {
            pending_blank = false;
        }

        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&line);
    }

    output.trim().to_string()
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
    fn synthesis_prompt_contains_required_sections() {
        let sections = vec![("TAM / Market".to_string(), "Example".to_string())];
        let prompt = build_synthesis_prompt("AAPL", None, None, &sections);
        assert!(prompt.contains("Alpha / What's Mispriced"));
        assert!(prompt.contains("Verdict: Long / No / Watch-list"));
    }

    #[test]
    fn format_analysis_text_removes_markdown_artifacts() {
        let raw = "## Header\n**Alpha** vs __beta__\n// note\n`code`";
        let cleaned = format_analysis_text(raw);
        assert_eq!(cleaned, "Header\nAlpha vs beta\nnote\ncode");
    }
}
