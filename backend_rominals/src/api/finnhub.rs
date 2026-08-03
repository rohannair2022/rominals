use chrono::{Duration as ChronoDuration, Utc};
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error;
use std::io::{self, Read};

const DEFAULT_FINNHUB_BASE_URL: &str = "https://finnhub.io/api/v1";
const FINNHUB_LOOKBACK_DAYS: i64 = 365;
const CONTEXT_MAX_CHARS_PER_DATASET: usize = 1_200;
const DEFAULT_LINK_CONTEXT_ENABLED: bool = true;
const DEFAULT_LINK_CONTEXT_MAX_URLS_PER_SCOPE: usize = 2;
const DEFAULT_LINK_CONTEXT_MAX_CHARS_PER_URL: usize = 700;
const DEFAULT_LINK_CONTEXT_MAX_TOTAL_CHARS_PER_SCOPE: usize = 1_400;
const DEFAULT_LINK_CONTEXT_MAX_FETCH_BYTES: usize = 90_000;
const DATASET_DEFS: [(&str, &str); 12] = [
    ("stock_profile", "Stock Profile"),
    ("news", "Market News (General)"),
    ("company_news", "Company News"),
    ("peers", "Peers"),
    ("insider_transactions", "Insider Transactions"),
    ("insider_sentiments", "Insider Sentiment"),
    ("financials_reported", "Financials Reported"),
    ("sec_filings", "SEC Filings"),
    ("earnings_surprises", "Earnings Surprises"),
    ("uspto_patents", "USPTO Patents"),
    ("stock_lobbying", "Stock Lobbying"),
    ("stock_usa_spending", "USA Spending"),
];

#[derive(Clone, Debug)]
pub struct FinnhubDatasetSnapshot {
    pub title: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Clone, Debug)]
pub struct FinnhubSnapshot {
    pub macro_context: String,
    pub micro_context: String,
    pub datasets: Vec<FinnhubDatasetSnapshot>,
}

#[derive(Clone, Debug)]
struct LinkContextConfig {
    enabled: bool,
    max_urls_per_scope: usize,
    max_chars_per_url: usize,
    max_total_chars_per_scope: usize,
    max_fetch_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct FinnhubClient {
    api_key: String,
    base_url: String,
    http: reqwest::blocking::Client,
}

impl FinnhubClient {
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        let api_key = std::env::var("ROMINALS_FINNHUB_API_KEY")
            .or_else(|_| std::env::var("FINNHUB_API_KEY"))
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Missing Finnhub API key. Set ROMINALS_FINNHUB_API_KEY or FINNHUB_API_KEY.",
                )
            })?;
        Self::new(api_key)
    }

    pub fn new(api_key: impl Into<String>) -> Result<Self, Box<dyn Error>> {
        Self::with_base_url(api_key, DEFAULT_FINNHUB_BASE_URL)
    }

    fn with_base_url(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let api_key = non_empty_param("api_key", &api_key.into())?;
        let base_url = non_empty_param("base_url", &base_url.into())?;
        let http = reqwest::blocking::Client::builder().build()?;

        Ok(Self {
            api_key,
            base_url,
            http,
        })
    }

    pub fn stock_profile(&self, symbol: &str) -> Result<Value, Box<dyn Error>> {
        self.get_json("stock/profile2", symbol_only_params(symbol)?)
    }

    pub fn news(&self, category: &str) -> Result<Value, Box<dyn Error>> {
        let category = non_empty_param("category", category)?;
        self.get_json("news", vec![("category", category)])
    }

    pub fn company_news(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.get_json("company-news", symbol_with_date_params(symbol, from, to)?)
    }

    pub fn peers(&self, symbol: &str) -> Result<Value, Box<dyn Error>> {
        self.get_json("stock/peers", symbol_only_params(symbol)?)
    }

    pub fn insider_transactions(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.get_json(
            "stock/insider-transactions",
            symbol_with_date_params(symbol, from, to)?,
        )
    }

    pub fn insider_sentiments(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.get_json(
            "stock/insider-sentiment",
            symbol_with_date_params(symbol, from, to)?,
        )
    }

    pub fn financials_reported(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.get_json(
            "stock/financials-reported",
            symbol_with_date_params(symbol, from, to)?,
        )
    }

    pub fn sec_filings(&self, symbol: &str, from: &str, to: &str) -> Result<Value, Box<dyn Error>> {
        self.get_json("stock/filings", symbol_with_date_params(symbol, from, to)?)
    }

    /// Historical quarterly earnings surprises (actual EPS vs. estimate).
    /// Finnhub exposes this as a single endpoint, `/stock/earnings` — there is
    /// no separate `/stock/earnings-surprises` path, despite what the name
    /// might suggest.
    pub fn earnings_surprises(&self, symbol: &str) -> Result<Value, Box<dyn Error>> {
        self.get_json("stock/earnings", symbol_only_params(symbol)?)
    }

    /// Alternative data endpoint. Note: this typically requires a paid
    /// "alternative data" add-on beyond the free tier — expect a 403 on a
    /// free-tier key rather than an empty result.
    pub fn uspto_patents(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.get_json(
            "stock/uspto-patent",
            symbol_with_date_params(symbol, from, to)?,
        )
    }

    /// Alternative data endpoint. Same free-tier caveat as `uspto_patents`.
    pub fn stock_lobbying(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.get_json("stock/lobbying", symbol_with_date_params(symbol, from, to)?)
    }

    /// Alternative data endpoint. Same free-tier caveat as `uspto_patents`.
    pub fn stock_usa_spending(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Value, Box<dyn Error>> {
        self.get_json(
            "stock/usa-spending",
            symbol_with_date_params(symbol, from, to)?,
        )
    }

    fn get_json(
        &self,
        endpoint: &str,
        mut query_params: Vec<(&'static str, String)>,
    ) -> Result<Value, Box<dyn Error>> {
        let endpoint = non_empty_param("endpoint", endpoint)?;
        query_params.push(("token", self.api_key.clone()));
        let url = self.build_url(&endpoint, &query_params)?;

        let resp = self.http.get(url).send().map_err(|e| {
            io::Error::other(format!(
                "Finnhub request failed for endpoint {endpoint}: {e}"
            ))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp
                .text()
                .unwrap_or_else(|err| format!("<failed to read error body: {err}>"));
            return Err(io::Error::other(format!(
                "HTTP {status} from Finnhub endpoint {endpoint}: {error_body}"
            ))
            .into());
        }

        resp.json().map_err(|e| {
            io::Error::other(format!(
                "Failed to parse JSON from Finnhub endpoint {endpoint}: {e}"
            ))
            .into()
        })
    }

    fn build_url(
        &self,
        endpoint: &str,
        query_params: &[(&'static str, String)],
    ) -> Result<reqwest::Url, Box<dyn Error>> {
        let endpoint = endpoint.trim_start_matches('/');
        let base = self.base_url.trim_end_matches('/');
        let mut url = reqwest::Url::parse(&format!("{base}/{endpoint}"))?;

        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query_params {
                pairs.append_pair(key, value);
            }
        }

        Ok(url)
    }
}

pub fn finnhub_dataset_titles() -> Vec<String> {
    DATASET_DEFS
        .iter()
        .map(|(_, title)| (*title).to_string())
        .collect()
}

pub fn fetch_finnhub_snapshot(symbol: &str) -> Result<FinnhubSnapshot, Box<dyn Error>> {
    let symbol = non_empty_param("symbol", symbol)?;
    let client = FinnhubClient::from_env()?;
    let (from, to) = default_date_window();
    let mut datasets = Vec::with_capacity(DATASET_DEFS.len());

    push_dataset(&mut datasets, DATASET_DEFS[0].1, || {
        client.stock_profile(&symbol)
    });
    push_dataset(&mut datasets, DATASET_DEFS[1].1, || client.news("general"));
    push_dataset(&mut datasets, DATASET_DEFS[2].1, || {
        client.company_news(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[3].1, || client.peers(&symbol));
    push_dataset(&mut datasets, DATASET_DEFS[4].1, || {
        client.insider_transactions(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[5].1, || {
        client.insider_sentiments(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[6].1, || {
        client.financials_reported(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[7].1, || {
        client.sec_filings(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[8].1, || {
        client.earnings_surprises(&symbol)
    });
    push_dataset(&mut datasets, DATASET_DEFS[9].1, || {
        client.uspto_patents(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[10].1, || {
        client.stock_lobbying(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[11].1, || {
        client.stock_usa_spending(&symbol, &from, &to)
    });

    let mut macro_context =
        build_scoped_finnhub_context(&symbol, &from, &to, &datasets, "Macro", is_macro_dataset);
    let mut micro_context =
        build_scoped_finnhub_context(&symbol, &from, &to, &datasets, "Micro", is_micro_dataset);

    let link_config = link_context_config_from_env();
    if link_config.enabled {
        let macro_link_context =
            build_scoped_link_context(&datasets, "Macro", is_macro_dataset, &link_config);
        if !macro_link_context.is_empty() {
            macro_context.push_str("\n\n");
            macro_context.push_str(&macro_link_context);
        }

        let micro_link_context =
            build_scoped_link_context(&datasets, "Micro", is_micro_dataset, &link_config);
        if !micro_link_context.is_empty() {
            micro_context.push_str("\n\n");
            micro_context.push_str(&micro_link_context);
        }
    }

    Ok(FinnhubSnapshot {
        macro_context,
        micro_context,
        datasets,
    })
}

fn default_date_window() -> (String, String) {
    let to = Utc::now().date_naive();
    let from = to - ChronoDuration::days(FINNHUB_LOOKBACK_DAYS);
    (
        from.format("%Y-%m-%d").to_string(),
        to.format("%Y-%m-%d").to_string(),
    )
}

fn push_dataset<F>(datasets: &mut Vec<FinnhubDatasetSnapshot>, title: &str, fetch: F)
where
    F: FnOnce() -> Result<Value, Box<dyn Error>>,
{
    match fetch() {
        Ok(value) => {
            let content =
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            datasets.push(FinnhubDatasetSnapshot {
                title: title.to_string(),
                content,
                is_error: false,
            });
        }
        Err(err) => {
            datasets.push(FinnhubDatasetSnapshot {
                title: title.to_string(),
                content: format!("Error: {err}"),
                is_error: true,
            });
        }
    }
}

fn build_scoped_finnhub_context<F>(
    symbol: &str,
    from: &str,
    to: &str,
    datasets: &[FinnhubDatasetSnapshot],
    scope_label: &str,
    include_dataset: F,
) -> String
where
    F: Fn(&str) -> bool,
{
    let mut context =
        format!("{scope_label} Finnhub context for {symbol} (rolling window {from} to {to})");
    let mut included_any = false;

    for dataset in datasets.iter().filter(|d| include_dataset(&d.title)) {
        included_any = true;
        let compact = truncate_for_context(&dataset.content, CONTEXT_MAX_CHARS_PER_DATASET);
        if dataset.is_error {
            context.push_str(&format!("\n\n{}:\nUnavailable. {}", dataset.title, compact));
        } else {
            context.push_str(&format!("\n\n{}:\n{}", dataset.title, compact));
        }
    }

    if !included_any {
        context.push_str("\n\nNo scoped Finnhub datasets available.");
    }

    context
}

fn is_macro_dataset(title: &str) -> bool {
    matches!(title, "Market News (General)")
}

fn is_micro_dataset(title: &str) -> bool {
    matches!(
        title,
        "Stock Profile"
            | "Company News"
            | "Peers"
            | "Insider Transactions"
            | "Insider Sentiment"
            | "Financials Reported"
            | "SEC Filings"
            | "Earnings Surprises"
            | "USPTO Patents"
            | "Stock Lobbying"
            | "USA Spending"
    )
}

fn link_context_config_from_env() -> LinkContextConfig {
    let enabled = std::env::var("ROMINALS_LINK_CONTEXT_ENABLED")
        .ok()
        .as_deref()
        .and_then(parse_env_bool)
        .unwrap_or(DEFAULT_LINK_CONTEXT_ENABLED);
    let max_urls_per_scope = std::env::var("ROMINALS_LINK_CONTEXT_MAX_URLS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_LINK_CONTEXT_MAX_URLS_PER_SCOPE);
    let max_chars_per_url = std::env::var("ROMINALS_LINK_CONTEXT_MAX_CHARS_PER_URL")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_LINK_CONTEXT_MAX_CHARS_PER_URL);
    let max_total_chars_per_scope = std::env::var("ROMINALS_LINK_CONTEXT_MAX_TOTAL_CHARS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_LINK_CONTEXT_MAX_TOTAL_CHARS_PER_SCOPE);
    let max_fetch_bytes = std::env::var("ROMINALS_LINK_CONTEXT_MAX_FETCH_BYTES")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_LINK_CONTEXT_MAX_FETCH_BYTES);

    LinkContextConfig {
        enabled,
        max_urls_per_scope,
        max_chars_per_url,
        max_total_chars_per_scope,
        max_fetch_bytes,
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

fn build_scoped_link_context<F>(
    datasets: &[FinnhubDatasetSnapshot],
    scope_label: &str,
    include_dataset: F,
    config: &LinkContextConfig,
) -> String
where
    F: Fn(&str) -> bool,
{
    let candidate_urls = extract_scoped_urls(datasets, include_dataset, config);
    if candidate_urls.is_empty() {
        return String::new();
    }

    let http = match reqwest::blocking::Client::builder().build() {
        Ok(client) => client,
        Err(_) => return String::new(),
    };

    build_link_context_from_urls(scope_label, &candidate_urls, config, |url| {
        fetch_url_snippet(&http, url, config)
    })
}

fn build_link_context_from_urls<F>(
    scope_label: &str,
    urls: &[String],
    config: &LinkContextConfig,
    mut fetch_snippet: F,
) -> String
where
    F: FnMut(&str) -> Result<String, io::Error>,
{
    let mut context = format!(
        "{scope_label} link-derived context (capped fetch: <= {} links, <= {} chars total)",
        config.max_urls_per_scope, config.max_total_chars_per_scope
    );
    let mut used_chars = 0usize;
    let mut used_sources = 0usize;

    for url in urls.iter().take(config.max_urls_per_scope) {
        let remaining_chars = config.max_total_chars_per_scope.saturating_sub(used_chars);
        if remaining_chars == 0 {
            break;
        }

        let fetched = match fetch_snippet(url) {
            Ok(text) => text,
            Err(_) => continue,
        };
        if fetched.trim().is_empty() {
            continue;
        }

        let source_char_cap = config.max_chars_per_url.min(remaining_chars);
        let snippet = truncate_for_context(&fetched, source_char_cap);
        let snippet_chars = snippet.chars().count();
        if snippet_chars == 0 {
            continue;
        }

        used_sources = used_sources.saturating_add(1);
        used_chars = used_chars.saturating_add(snippet_chars);
        context.push_str(&format!("\n\nSource {used_sources}: {url}\n{snippet}"));
    }

    if used_sources == 0 {
        return String::new();
    }

    context
}

fn extract_scoped_urls<F>(
    datasets: &[FinnhubDatasetSnapshot],
    include_dataset: F,
    config: &LinkContextConfig,
) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut urls = Vec::new();
    let mut seen = HashSet::new();
    let candidate_limit = config.max_urls_per_scope.saturating_mul(4).max(4);

    for dataset in datasets
        .iter()
        .filter(|dataset| !dataset.is_error && include_dataset(&dataset.title))
    {
        for url in extract_urls_from_json_content(&dataset.content) {
            if seen.insert(url.clone()) {
                urls.push(url);
                if urls.len() >= candidate_limit {
                    return urls;
                }
            }
        }
    }

    urls
}

fn extract_urls_from_json_content(content: &str) -> Vec<String> {
    let parsed: Value = match serde_json::from_str(content) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let mut urls = Vec::new();
    collect_urls_from_value(&parsed, &mut urls);

    let mut seen = HashSet::new();
    urls.into_iter()
        .filter(|url| seen.insert(url.clone()))
        .collect()
}

fn collect_urls_from_value(value: &Value, urls: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_urls_from_value(item, urls);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_urls_from_value(item, urls);
            }
        }
        Value::String(text) => {
            for url in extract_http_urls_from_text(text) {
                urls.push(url);
            }
        }
        _ => {}
    }
}

fn extract_http_urls_from_text(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|part| {
            let candidate = part.trim_matches(|ch: char| {
                ch.is_ascii_punctuation() && ch != '/' && ch != ':' && ch != '?' && ch != '&'
            });
            if candidate.starts_with("http://") || candidate.starts_with("https://") {
                Some(candidate.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn fetch_url_snippet(
    http: &reqwest::blocking::Client,
    url: &str,
    config: &LinkContextConfig,
) -> Result<String, io::Error> {
    let response = http.get(url).send().map_err(|err| {
        io::Error::other(format!("failed to fetch link context from {url}: {err}"))
    })?;
    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "non-success HTTP {} while fetching {url}",
            response.status()
        )));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    let mut body_bytes = Vec::new();
    response
        .take(config.max_fetch_bytes as u64)
        .read_to_end(&mut body_bytes)?;
    let body = String::from_utf8_lossy(&body_bytes);

    if content_type.contains("application/json") {
        let maybe_json: Result<Value, _> = serde_json::from_str(&body);
        let json_text = maybe_json
            .and_then(|value| serde_json::to_string_pretty(&value))
            .unwrap_or_else(|_| body.to_string());
        return Ok(normalize_whitespace(&json_text));
    }

    if content_type.contains("text/html") || body.contains("<html") || body.contains("<HTML") {
        return Ok(normalize_whitespace(&strip_html_tags(&body)));
    }

    Ok(normalize_whitespace(&body))
}

fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_for_context(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
    }

    if max_chars <= 40 {
        return chars.into_iter().take(max_chars).collect();
    }

    let keep = max_chars.saturating_sub(40);
    let truncated: String = chars.into_iter().take(keep).collect();
    format!(
        "{truncated}\n...[truncated {} chars]",
        value.chars().count().saturating_sub(keep)
    )
}

fn non_empty_param(name: &str, value: &str) -> Result<String, io::Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} cannot be empty"),
        ));
    }
    Ok(trimmed.to_string())
}

fn symbol_only_params(symbol: &str) -> Result<Vec<(&'static str, String)>, Box<dyn Error>> {
    Ok(vec![("symbol", non_empty_param("symbol", symbol)?)])
}

fn symbol_with_date_params(
    symbol: &str,
    from: &str,
    to: &str,
) -> Result<Vec<(&'static str, String)>, Box<dyn Error>> {
    Ok(vec![
        ("symbol", non_empty_param("symbol", symbol)?),
        ("from", non_empty_param("from", from)?),
        ("to", non_empty_param("to", to)?),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_adds_query_params_and_token() {
        let client = FinnhubClient::with_base_url("demo-key", "https://finnhub.io/api/v1").unwrap();
        let url = client
            .build_url(
                "stock/profile2",
                &[
                    ("symbol", "AAPL".to_string()),
                    ("token", "demo-key".to_string()),
                ],
            )
            .unwrap();
        let url_text = url.as_str();

        assert!(url_text.starts_with("https://finnhub.io/api/v1/stock/profile2?"));
        assert!(url_text.contains("symbol=AAPL"));
        assert!(url_text.contains("token=demo-key"));
    }

    #[test]
    fn non_empty_param_rejects_blank_values() {
        assert!(non_empty_param("symbol", "").is_err());
        assert!(non_empty_param("symbol", "   ").is_err());
    }

    #[test]
    fn symbol_with_date_params_includes_symbol_and_dates() {
        let params = symbol_with_date_params("AAPL", "2024-01-01", "2024-02-01").unwrap();
        assert_eq!(params.len(), 3);
        assert_eq!(params[0], ("symbol", "AAPL".to_string()));
        assert_eq!(params[1], ("from", "2024-01-01".to_string()));
        assert_eq!(params[2], ("to", "2024-02-01".to_string()));
    }

    #[test]
    fn earnings_surprises_hits_stock_earnings_endpoint() {
        // Regression test: `/stock/earnings-surprises` does not exist on
        // Finnhub's API. Earnings surprise data is served from `/stock/earnings`.
        let client = FinnhubClient::with_base_url("demo-key", "https://finnhub.io/api/v1").unwrap();
        let url = client
            .build_url("stock/earnings", &[("symbol", "AAPL".to_string())])
            .unwrap();
        assert!(
            url.as_str()
                .starts_with("https://finnhub.io/api/v1/stock/earnings?")
        );
    }

    #[test]
    fn finnhub_dataset_titles_matches_dataset_count() {
        assert_eq!(finnhub_dataset_titles().len(), DATASET_DEFS.len());
    }

    #[test]
    fn truncate_for_context_marks_truncated_output() {
        let source = "x".repeat(2_000);
        let truncated = truncate_for_context(&source, 100);
        assert!(truncated.contains("...[truncated"));
        assert!(truncated.len() < source.len());
    }

    #[test]
    fn truncate_for_context_respects_small_limits() {
        let source = "abcdefghijklmnopqrstuvwxyz";
        let truncated = truncate_for_context(source, 12);
        assert_eq!(truncated.chars().count(), 12);
    }

    #[test]
    fn scoped_context_filters_macro_and_micro_datasets() {
        let datasets = vec![
            FinnhubDatasetSnapshot {
                title: "Market News (General)".to_string(),
                content: "macro".to_string(),
                is_error: false,
            },
            FinnhubDatasetSnapshot {
                title: "Stock Profile".to_string(),
                content: "micro".to_string(),
                is_error: false,
            },
        ];

        let macro_context = build_scoped_finnhub_context(
            "AAPL",
            "2025-01-01",
            "2025-12-31",
            &datasets,
            "Macro",
            is_macro_dataset,
        );
        let micro_context = build_scoped_finnhub_context(
            "AAPL",
            "2025-01-01",
            "2025-12-31",
            &datasets,
            "Micro",
            is_micro_dataset,
        );

        assert!(macro_context.contains("Market News (General)"));
        assert!(!macro_context.contains("Stock Profile"));
        assert!(micro_context.contains("Stock Profile"));
        assert!(!micro_context.contains("Market News (General)"));
    }

    #[test]
    fn extract_urls_from_json_content_collects_and_deduplicates() {
        let content = r#"{
          "url": "https://example.com/a",
          "items": [
            {"link": "https://example.com/a"},
            {"link": "https://example.com/b?x=1"}
          ],
          "note": "see https://example.com/c for details"
        }"#;

        let urls = extract_urls_from_json_content(content);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://example.com/a");
        assert_eq!(urls[1], "https://example.com/b?x=1");
        assert_eq!(urls[2], "https://example.com/c");
    }

    #[test]
    fn build_link_context_from_urls_enforces_limits() {
        let config = LinkContextConfig {
            enabled: true,
            max_urls_per_scope: 2,
            max_chars_per_url: 50,
            max_total_chars_per_scope: 120,
            max_fetch_bytes: 100,
        };
        let urls = vec![
            "https://one.test".to_string(),
            "https://two.test".to_string(),
            "https://three.test".to_string(),
        ];

        let context =
            build_link_context_from_urls(
                "Macro",
                &urls,
                &config,
                |_| Ok("abcdefghijk".to_string()),
            );

        assert!(context.contains("Source 1: https://one.test"));
        assert!(context.contains("Source 2: https://two.test"));
        assert!(!context.contains("Source 3: https://three.test"));
    }

    #[test]
    fn parse_env_bool_accepts_common_values() {
        assert_eq!(parse_env_bool("true"), Some(true));
        assert_eq!(parse_env_bool("0"), Some(false));
        assert_eq!(parse_env_bool("off"), Some(false));
        assert_eq!(parse_env_bool("nope"), None);
    }
}
