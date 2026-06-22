use serde::Deserialize;
use std::env;
use std::error::Error;


#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: Chart,
}

#[derive(Debug, Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: Meta,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    symbol: String,
    currency: Option<String>,
    full_exchange_name: Option<String>,
    long_name: Option<String>,
    short_name: Option<String>,
    regular_market_price: Option<f64>,
    chart_previous_close: Option<f64>,
    regular_market_day_high: Option<f64>,
    regular_market_day_low: Option<f64>,
    regular_market_volume: Option<i64>,
    fifty_two_week_high: Option<f64>,
    fifty_two_week_low: Option<f64>,
}

// ----- Helpers --------------------------------------------------------------

/// Format an Option<f64> as a price string, or "n/a" if absent.
fn money(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.2}", x),
        None => "n/a".to_string(),
    }
}

fn display_meta(meta: &Meta) -> String {
    let (change, pct) = match (meta.regular_market_price, meta.chart_previous_close) {
        (Some(price), Some(prev_close)) if prev_close != 0.0 => {
            let delta = price - prev_close;
            (Some(delta), Some(delta / prev_close * 100.0))
        }
        _ => (None, None),
    };

    let name = meta
        .long_name
        .as_deref()
        .or(meta.short_name.as_deref())
        .unwrap_or(&meta.symbol);

    let change_text = match (change, pct) {
        (Some(delta), Some(percent)) => {
            let sign = if delta >= 0.0 { "+" } else { "" };
            format!("{sign}{delta:.2} ({sign}{percent:.2}%)")
        }
        _ => "n/a".to_string(),
    };

    let volume = meta
        .regular_market_volume
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());

    format!(
        "{} ({})\n  Exchange:       {}\n  Currency:       {}\n  Price:          {}\n  Change:         {}\n  Prev close:     {}\n  Day range:      {} - {}\n  52-week range:  {} - {}\n  Volume:         {}",
        name,
        meta.symbol,
        meta.full_exchange_name.as_deref().unwrap_or("n/a"),
        meta.currency.as_deref().unwrap_or("n/a"),
        money(meta.regular_market_price),
        change_text,
        money(meta.chart_previous_close),
        money(meta.regular_market_day_low),
        money(meta.regular_market_day_high),
        money(meta.fifty_two_week_low),
        money(meta.fifty_two_week_high),
        volume,
    )
}

// ----- Entry point ----------------------------------------------------------

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: cargo run -- <TICKER>   (e.g. AAPL, MSFT, KRKNF)");
        std::process::exit(2);
    }

    let ticker = args[1].to_uppercase();

    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{ticker}"
    );

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (rust-yfinance/0.1)")
        .build()?;

    let resp = client.get(&url).send()?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} from Yahoo", resp.status()).into());
    }

    println!("Fetching quote for {}...", ticker);

    let body: ChartResponse = resp.json()?;

    let meta = match body.chart.result {
        Some(mut results_vec) => match results_vec.pop() {
            Some(results_item) => results_item.meta,
            None => return Err("No data — is that a valid ticker?".into()),
        },
        None => return Err("No data — is that a valid ticker?".into()),
    };

    println!("{}", display_meta(&meta));

    Ok(())
}