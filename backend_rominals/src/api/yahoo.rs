use serde::Deserialize;
use std::error::Error;
use std::io;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: Chart,
}

#[derive(Debug, Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
    error: Option<ChartApiError>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: Meta,
}

#[derive(Debug, Deserialize)]
struct ChartApiError {
    code: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub symbol: String,
    pub currency: Option<String>,
    pub full_exchange_name: Option<String>,
    pub long_name: Option<String>,
    pub short_name: Option<String>,
    pub regular_market_price: Option<f64>,
    pub chart_previous_close: Option<f64>,
    pub regular_market_day_high: Option<f64>,
    pub regular_market_day_low: Option<f64>,
    pub regular_market_volume: Option<i64>,
    pub fifty_two_week_high: Option<f64>,
    pub fifty_two_week_low: Option<f64>,
}

pub fn fetch_quote(ticker: &str) -> Result<Meta, Box<dyn Error>> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{ticker}");

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (rust-yfinance/0.1)")
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let resp = client.get(&url).send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_body = resp
            .text()
            .unwrap_or_else(|err| format!("<failed to read error body: {err}>"));
        return Err(io::Error::other(format!(
            "HTTP {status} from Yahoo for {ticker}: {error_body}"
        ))
        .into());
    }

    let body: ChartResponse = resp.json()?;
    let meta = extract_meta(body, ticker)?;
    Ok(meta)
}

fn extract_meta(body: ChartResponse, ticker: &str) -> Result<Meta, io::Error> {
    let Chart { result, error } = body.chart;

    if let Some(api_error) = error {
        let code = api_error.code.unwrap_or_else(|| "unknown".to_string());
        let description = api_error
            .description
            .unwrap_or_else(|| "No description provided".to_string());
        return Err(io::Error::other(format!(
            "Yahoo API error for {ticker} [{code}]: {description}"
        )));
    }

    let first_result = result
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Yahoo response did not include quote results for {ticker}"),
            )
        })?
        .into_iter()
        .next()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Yahoo returned an empty result list for {ticker}"),
            )
        })?;

    Ok(first_result.meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta(symbol: &str) -> Meta {
        Meta {
            symbol: symbol.to_string(),
            currency: None,
            full_exchange_name: None,
            long_name: None,
            short_name: None,
            regular_market_price: None,
            chart_previous_close: None,
            regular_market_day_high: None,
            regular_market_day_low: None,
            regular_market_volume: None,
            fifty_two_week_high: None,
            fifty_two_week_low: None,
        }
    }

    #[test]
    fn extract_meta_returns_first_result_meta() {
        let body = ChartResponse {
            chart: Chart {
                result: Some(vec![ChartResult {
                    meta: sample_meta("AAPL"),
                }]),
                error: None,
            },
        };

        let meta = extract_meta(body, "AAPL").unwrap();
        assert_eq!(meta.symbol, "AAPL");
    }

    #[test]
    fn extract_meta_returns_api_error_details() {
        let body = ChartResponse {
            chart: Chart {
                result: None,
                error: Some(ChartApiError {
                    code: Some("Not Found".to_string()),
                    description: Some("No data found".to_string()),
                }),
            },
        };

        let err = extract_meta(body, "INVALID").unwrap_err();
        assert_eq!(
            err.to_string(),
            "Yahoo API error for INVALID [Not Found]: No data found"
        );
    }
}
