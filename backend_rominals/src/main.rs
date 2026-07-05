mod api;

use api::yahoo::{Meta, fetch_quote};
use std::env;
use std::error::Error;
use std::io;

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
        (Some(price), Some(previous_close)) if previous_close != 0.0 => {
            let delta = price - previous_close;
            (Some(delta), Some(delta / previous_close * 100.0))
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

fn usage_message(binary: &str) -> String {
    format!("Usage: {binary} <TICKER>   (e.g. AAPL, MSFT, KRKNF)")
}

fn parse_ticker_from_args<I>(args: I) -> Result<String, io::Error>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let binary = args.next().unwrap_or_else(|| "cargo run --".to_string());
    let ticker = args.next();
    let has_extra_args = args.next().is_some();

    match (ticker, has_extra_args) {
        (Some(raw_ticker), false) if !raw_ticker.trim().is_empty() => Ok(raw_ticker.to_uppercase()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            usage_message(&binary),
        )),
    }
}

// ----- Entry point ----------------------------------------------------------

fn main() -> Result<(), Box<dyn Error>> {
    let ticker = parse_ticker_from_args(env::args())?;
    let meta = fetch_quote(&ticker)?;

    println!("Fetching quote for {}...", ticker);
    println!("{}", display_meta(&meta));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> Meta {
        Meta {
            symbol: "AAPL".to_string(),
            currency: Some("USD".to_string()),
            full_exchange_name: Some("NasdaqGS".to_string()),
            long_name: Some("Apple Inc.".to_string()),
            short_name: Some("Apple".to_string()),
            regular_market_price: Some(100.0),
            chart_previous_close: Some(95.0),
            regular_market_day_high: Some(101.0),
            regular_market_day_low: Some(94.5),
            regular_market_volume: Some(123456789),
            fifty_two_week_high: Some(200.0),
            fifty_two_week_low: Some(80.0),
        }
    }

    #[test]
    fn money_formats_option_values() {
        assert_eq!(money(Some(123.456)), "123.46");
        assert_eq!(money(None), "n/a");
    }

    #[test]
    fn parse_ticker_requires_exactly_one_non_empty_arg() {
        let args = vec!["backend_rominals".to_string(), "msft".to_string()];
        assert_eq!(parse_ticker_from_args(args).unwrap(), "MSFT");

        let missing_ticker = vec!["backend_rominals".to_string()];
        assert!(parse_ticker_from_args(missing_ticker).is_err());

        let too_many_args = vec![
            "backend_rominals".to_string(),
            "AAPL".to_string(),
            "EXTRA".to_string(),
        ];
        assert!(parse_ticker_from_args(too_many_args).is_err());
    }

    #[test]
    fn display_meta_formats_change_and_name() {
        let output = display_meta(&sample_meta());
        assert!(output.contains("Apple Inc. (AAPL)"));
        assert!(output.contains("Change:         +5.00 (+5.26%)"));
    }

    #[test]
    fn display_meta_hides_change_when_previous_close_is_zero() {
        let mut meta = sample_meta();
        meta.chart_previous_close = Some(0.0);

        let output = display_meta(&meta);
        assert!(output.contains("Change:         n/a"));
    }
}
