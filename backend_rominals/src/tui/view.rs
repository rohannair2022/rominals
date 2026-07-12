use super::state::App;
use crate::api::yahoo::Meta;
use crossterm::cursor;
use crossterm::execute;
use crossterm::terminal::{self, ClearType};
use std::io;
use std::io::Write;

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

pub(super) fn draw_ui(stdout: &mut io::Stdout, app: &App) -> io::Result<()> {
    execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    writeln!(stdout, "Rominals TUI")?;
    writeln!(
        stdout,
        "Type a ticker and press Enter. Press r to refresh, q to quit."
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "Ticker input: {}", app.input)?;

    if let Some(ticker) = &app.active_ticker {
        writeln!(stdout, "Current symbol: {ticker}")?;
    }

    if let Some(meta) = &app.quote {
        writeln!(stdout)?;
        writeln!(stdout, "{}", display_meta(meta))?;
    }

    if let Some(error) = &app.error {
        writeln!(stdout)?;
        writeln!(stdout, "Error: {error}")?;
    }

    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> Meta {
        Meta {
            symbol: "AAPL".to_string(),
            long_name: Some("Apple Inc.".to_string()),
            regular_market_price: Some(100.0),
            chart_previous_close: Some(95.0),
            ..Default::default()
        }
    }

    #[test]
    fn money_formats_option_values() {
        assert_eq!(money(Some(123.456)), "123.46");
        assert_eq!(money(None), "n/a");
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
