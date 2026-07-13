use super::state::App;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};

const INVALID_TICKER_ERROR: &str = "Ticker can only include letters, numbers, '.', '-', and '^'.";

fn is_valid_ticker_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '^')
}

pub(crate) fn normalize_ticker(input: &str) -> Option<String> {
    let normalized = input.trim().to_uppercase();

    if normalized.is_empty() || !normalized.chars().all(is_valid_ticker_char) {
        return None;
    }

    Some(normalized)
}

pub(super) fn handle_event<F>(app: &mut App, event: Event, fetch_quote: F) -> bool
where
    F: Fn(&mut App, &str),
{
    if let Event::Key(key_event) = event {
        return handle_key_event(app, key_event, fetch_quote);
    }
    true
}

fn handle_key_event<F>(app: &mut App, key_event: KeyEvent, fetch_quote: F) -> bool
where
    F: Fn(&mut App, &str),
{
    if key_event.kind != KeyEventKind::Press {
        return true;
    }

    match key_event.code {
        KeyCode::Char('q') => false,
        KeyCode::Char('r') => {
            if let Some(ticker) = app.active_ticker.clone() {
                fetch_quote(app, &ticker);
            }
            true
        }
        KeyCode::Enter => {
            if let Some(ticker) = normalize_ticker(&app.input) {
                app.input.clear();
                fetch_quote(app, &ticker);
            } else {
                app.error = Some(INVALID_TICKER_ERROR.to_string());
            }
            true
        }
        KeyCode::Backspace => {
            app.input.pop();
            true
        }
        KeyCode::Char(c) => {
            if is_valid_ticker_char(c) {
                app.input.push(c.to_ascii_uppercase());
            }
            true
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ticker_accepts_supported_symbols() {
        assert_eq!(normalize_ticker(" msft "), Some("MSFT".to_string()));
        assert_eq!(normalize_ticker("^gspc"), Some("^GSPC".to_string()));
        assert_eq!(normalize_ticker("btc-usd"), Some("BTC-USD".to_string()));
    }

    #[test]
    fn normalize_ticker_rejects_empty_or_invalid_values() {
        assert_eq!(normalize_ticker(""), None);
        assert_eq!(normalize_ticker("  "), None);
        assert_eq!(normalize_ticker("AAPL!"), None);
    }
}
