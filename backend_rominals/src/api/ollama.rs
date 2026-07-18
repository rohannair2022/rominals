use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{self, BufRead, BufReader};
use std::time::Duration;

const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gpt-oss:120b-cloud";

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
    tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    #[allow(dead_code)]
    id: Option<String>,
}

struct StreamOutcome {
    content: String,
    saw_tool_calls: bool,
}

pub fn analyze_company_streaming<F>(
    ticker: &str,
    comparison_ticker: Option<&str>,
    mut on_partial: F,
) -> Result<String, Box<dyn Error>>
where
    F: FnMut(&str),
{
    let host =
        std::env::var("ROMINALS_OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string());
    let model =
        std::env::var("ROMINALS_OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());
    let prompt = build_analysis_prompt(ticker, comparison_ticker);
    let url = format!("{}/api/chat", host.trim_end_matches('/'));

    let client = reqwest::blocking::Client::builder()
        .user_agent("rominals-ollama/0.1")
        .timeout(Duration::from_secs(90))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let first_stream = stream_chat_request(&client, &url, &model, &prompt, true, |formatted| {
        on_partial(formatted);
    })?;

    if !first_stream.content.is_empty() {
        return Ok(first_stream.content);
    }

    if first_stream.saw_tool_calls {
        let fallback_prompt = format!(
            "{prompt}\nIf web-search tools are unavailable, still respond directly with your best current analysis."
        );
        let fallback_response = send_chat_request(&client, &url, &model, &fallback_prompt, false)?;
        let fallback_content = format_analysis_text(&fallback_response.content);
        if !fallback_content.is_empty() {
            on_partial(&fallback_content);
            return Ok(fallback_content);
        }
    }

    Err(io::Error::other("Ollama returned an empty analysis response").into())
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

            if !message.content.is_empty() {
                raw_output.push_str(&message.content);
                let formatted = format_analysis_text(&raw_output);
                if !formatted.is_empty() {
                    on_partial(&formatted);
                }
            }
        }

        if chunk.done {
            break;
        }
    }

    Ok(StreamOutcome {
        content: format_analysis_text(&raw_output),
        saw_tool_calls,
    })
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

fn build_analysis_prompt(ticker: &str, comparison_ticker: Option<&str>) -> String {
    let subject = match comparison_ticker {
        Some(comp) => format!("{ticker} vs. comp {comp}"),
        None => ticker.to_string(),
    };

    format!(
        "Analyze {subject} as a potential long. Be concise, data-dense, no filler. \
Use current numbers (search if needed) - don't rely on stale memory for financials. \
Return plain text only (no markdown syntax such as **, //, #, or backticks). \
Structure: TAM/Market - What market(s) is this actually selling into (e.g. fintech, AI infra, defense)? \
Size + growth rate. Is the company a share-taker or riding the tide? \
Relative Valuation - P/S, P/E (or EV/EBITDA if unprofitable), PEG vs. 1-2 direct comps. \
Is it cheap/expensive and why (growth premium, moat, hype)? \
Fundamentals - Revenue growth (YoY/QoQ), gross margin trend, path to profitability or current margins, \
FCF, SBC as % of revenue (dilution drag), balance sheet health (cash vs. debt). \
Catalysts - Near-term (next 1-2 quarters: earnings, product launches, conferences) and structural \
(TAM expansion, new verticals, contracts). Macro - Rate cuts/hikes, geopolitical tailwinds (war, trade, reshoring), \
sector rotation - how does this name specifically benefit or get hurt? Risks - Dilution risk, \
convertible notes/debt maturities, customer concentration, regulatory, competitive threat, valuation risk if multiple compresses. \
Technicals (brief) - Trend vs. key MAs, relative strength vs. sector, is this a good entry now or \
does it need to cool off / pull back to a level? Competitors - Market share map, who's winning share and why, \
moat durability (patents, switching costs, capex lead). Alpha / What's Mispriced - The one thing the market isn't \
pricing in yet (bull or bear). This is the actual edge - everything above is just groundwork for this. \
End with a verdict: Long / No / Watch-list, with the single strongest reason and the level or catalyst that would change your mind."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_primary_ticker() {
        let prompt = build_analysis_prompt("AAPL", None);
        assert!(prompt.starts_with("Analyze AAPL as a potential long."));
    }

    #[test]
    fn prompt_includes_comparison_ticker_when_present() {
        let prompt = build_analysis_prompt("NVDA", Some("AMD"));
        assert!(prompt.starts_with("Analyze NVDA vs. comp AMD as a potential long."));
    }

    #[test]
    fn format_analysis_text_removes_markdown_artifacts() {
        let raw = "## Header\n**Alpha** vs __beta__\n// note\n`code`";
        let cleaned = format_analysis_text(raw);
        assert_eq!(cleaned, "Header\nAlpha vs beta\nnote\ncode");
    }
}
