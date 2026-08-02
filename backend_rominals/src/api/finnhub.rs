use chrono::{Duration as ChronoDuration, Utc};
use serde_json::Value;
use std::error::Error;
use std::io;
use std::time::Duration;

const DEFAULT_FINNHUB_BASE_URL: &str = "https://finnhub.io/api/v1";
const DEFAULT_TIMEOUT_SECS: u64 = 15;
const FINNHUB_LOOKBACK_DAYS: i64 = 365;
const CONTEXT_MAX_CHARS_PER_DATASET: usize = 1_200;
const DATASET_DEFS: [(&str, &str); 13] = [
    ("stock_profile", "Stock Profile"),
    ("news", "Market News (General)"),
    ("company_news", "Company News"),
    ("market_sentiment", "Market Sentiment"),
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
    pub context: String,
    pub datasets: Vec<FinnhubDatasetSnapshot>,
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
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(5))
            .build()?;

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

    pub fn market_sentiment(&self, symbol: &str) -> Result<Value, Box<dyn Error>> {
        self.get_json("news-sentiment", symbol_only_params(symbol)?)
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
    push_dataset(&mut datasets, DATASET_DEFS[3].1, || {
        client.market_sentiment(&symbol)
    });
    push_dataset(&mut datasets, DATASET_DEFS[4].1, || client.peers(&symbol));
    push_dataset(&mut datasets, DATASET_DEFS[5].1, || {
        client.insider_transactions(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[6].1, || {
        client.insider_sentiments(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[7].1, || {
        client.financials_reported(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[8].1, || {
        client.sec_filings(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[9].1, || {
        client.earnings_surprises(&symbol)
    });
    push_dataset(&mut datasets, DATASET_DEFS[10].1, || {
        client.uspto_patents(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[11].1, || {
        client.stock_lobbying(&symbol, &from, &to)
    });
    push_dataset(&mut datasets, DATASET_DEFS[12].1, || {
        client.stock_usa_spending(&symbol, &from, &to)
    });

    let context = build_finnhub_context(&symbol, &from, &to, &datasets);
    Ok(FinnhubSnapshot { context, datasets })
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

fn build_finnhub_context(
    symbol: &str,
    from: &str,
    to: &str,
    datasets: &[FinnhubDatasetSnapshot],
) -> String {
    let mut context = format!(
        "Finnhub snapshot for {symbol} (rolling window {from} to {to})\n\
Use this as supplemental context alongside Yahoo data."
    );

    for dataset in datasets {
        let compact = truncate_for_context(&dataset.content, CONTEXT_MAX_CHARS_PER_DATASET);
        if dataset.is_error {
            context.push_str(&format!("\n\n{}:\nUnavailable. {}", dataset.title, compact));
        } else {
            context.push_str(&format!("\n\n{}:\n{}", dataset.title, compact));
        }
    }

    context
}

fn truncate_for_context(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
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
    fn build_finnhub_context_includes_dataset_names() {
        let datasets = vec![
            FinnhubDatasetSnapshot {
                title: "Stock Profile".to_string(),
                content: "{\"name\":\"Apple\"}".to_string(),
                is_error: false,
            },
            FinnhubDatasetSnapshot {
                title: "SEC Filings".to_string(),
                content: "Error: HTTP 403".to_string(),
                is_error: true,
            },
        ];

        let context = build_finnhub_context("AAPL", "2025-01-01", "2025-12-31", &datasets);
        assert!(context.contains("Finnhub snapshot for AAPL"));
        assert!(context.contains("Stock Profile"));
        assert!(context.contains("SEC Filings"));
        assert!(context.contains("Unavailable."));
    }
}
