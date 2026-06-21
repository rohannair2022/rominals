//! Tiny CLI: `cargo run -- AAPL` -> prints a quote snapshot from Yahoo Finance.
//!
//! Uses the public v8 chart endpoint, which returns a `meta` object with the
//! fields we care about and does NOT require the cookie/crumb auth that the
//! newer v7 quote endpoint now demands.

use serde::Deserialize;
use std::env;
use std::error::Error;

// ----- Response shape -------------------------------------------------------
// We only declare the fields we want. serde silently ignores everything else
// in the JSON, so we don't have to model Yahoo's entire (huge) payload.

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: Chart,
}

#[derive(Debug, Deserialize)]
struct Chart {
    // `result` is null when the ticker is invalid, hence Option<...>.
    result: Option<Vec<ChartResult>>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: Meta,
}

// Yahoo uses camelCase keys; this attribute maps them to snake_case fields.
// Every quote field is Option<T> because Yahoo omits some depending on the
// asset class / market state, and a missing field should not crash us.
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
    v.map(|x| format!("{:.2}", x)).unwrap_or_else(|| "n/a".into())
}

// ----- Entry point ----------------------------------------------------------

fn main() -> Result<(), Box<dyn Error>> {
    // 1. Read the ticker from argv. nth(1) skips the program name.
    let ticker = match env::args().nth(1) {
        Some(t) => t.to_uppercase(),
        None => {
            eprintln!("Usage: cargo run -- <TICKER>   (e.g. AAPL, MSFT, KRKNF)");
            std::process::exit(2);
        }
    };

    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{ticker}"
    );

    // 2. Build a reusable client. A User-Agent is mandatory — Yahoo returns
    //    429/403 to clients that don't send one.
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (rust-yfinance/0.1)")
        .build()?;

    // 3. Fire the request. `?` propagates network errors up to main, which
    //    prints them and returns a non-zero exit code automatically.
    let resp = client.get(&url).send()?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} from Yahoo", resp.status()).into());
    }

    // 4. Deserialize JSON straight into our typed structs.
    let body: ChartResponse = resp.json()?;

    // 5. Unwrap the single result, or bail with a friendly message.
    let meta = body
        .chart
        .result
        .and_then(|mut v| v.pop()) // take the first (only) result
        .map(|r| r.meta)
        .ok_or("No data — is that a valid ticker?")?;

    // 6. Derived numbers.
    let (change, pct) = match (meta.regular_market_price, meta.chart_previous_close) {
        (Some(p), Some(prev)) if prev != 0.0 => (Some(p - prev), Some((p - prev) / prev * 100.0)),
        _ => (None, None),
    };

    // 7. Print.
    let name = meta
        .long_name
        .or(meta.short_name)
        .unwrap_or_else(|| meta.symbol.clone());

    println!("{} ({})", name, meta.symbol);
    println!("  Exchange:       {}", meta.full_exchange_name.unwrap_or_else(|| "n/a".into()));
    println!("  Currency:       {}", meta.currency.unwrap_or_else(|| "n/a".into()));
    println!("  Price:          {}", money(meta.regular_market_price));
    match (change, pct) {
        (Some(c), Some(p)) => {
            let sign = if c >= 0.0 { "+" } else { "" };
            println!("  Change:         {sign}{:.2} ({sign}{:.2}%)", c, p);
        }
        _ => println!("  Change:         n/a"),
    }
    println!("  Prev close:     {}", money(meta.chart_previous_close));
    println!("  Day range:      {} - {}", money(meta.regular_market_day_low), money(meta.regular_market_day_high));
    println!("  52-week range:  {} - {}", money(meta.fifty_two_week_low), money(meta.fifty_two_week_high));
    println!("  Volume:         {}", meta.regular_market_volume.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()));

    Ok(())
}