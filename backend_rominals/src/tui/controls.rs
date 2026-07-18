use super::state::App;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

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

    app.input_cursor_visible = true;

    match (key_event.code, key_event.modifiers) {
        (KeyCode::Esc, _) => false,
        (KeyCode::Char('q' | 'Q'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => false,
        (KeyCode::Char('c' | 'C'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => false,
        (KeyCode::Char('r' | 'R'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(ticker) = app.active_ticker.clone() {
                fetch_quote(app, &ticker);
            }
            true
        }
        (KeyCode::Enter, _) => {
            if let Some(ticker) = normalize_ticker(&app.input) {
                app.input.clear();
                app.analysis_scroll = 0;
                fetch_quote(app, &ticker);
            } else {
                app.error = Some(INVALID_TICKER_ERROR.to_string());
            }
            true
        }
        (KeyCode::Up, _) => {
            app.analysis_scroll = app.analysis_scroll.saturating_sub(1);
            true
        }
        (KeyCode::Down, _) => {
            app.analysis_scroll = app.analysis_scroll.saturating_add(1);
            true
        }
        (KeyCode::PageUp, _) => {
            app.analysis_scroll = app.analysis_scroll.saturating_sub(8);
            true
        }
        (KeyCode::PageDown, _) => {
            app.analysis_scroll = app.analysis_scroll.saturating_add(8);
            true
        }
        (KeyCode::Home, _) => {
            app.analysis_scroll = 0;
            true
        }
        (KeyCode::Backspace, _) => {
            app.input.pop();
            true
        }
        (KeyCode::Char(c), _) => {
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
    use std::cell::Cell;

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

    #[test]
    fn handle_event_accepts_q_character_for_input() {
        let mut app = App::default();
        let event = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

        let keep_running = handle_event(&mut app, event, |_app, _ticker| {});

        assert!(keep_running);
        assert_eq!(app.input, "Q");
    }

    #[test]
    fn handle_event_accepts_r_character_for_input() {
        let mut app = App::default();
        let event = Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

        let keep_running = handle_event(&mut app, event, |_app, _ticker| {});

        assert!(keep_running);
        assert_eq!(app.input, "R");
    }

    #[test]
    fn handle_event_quits_on_escape() {
        let mut app = App::default();
        let event = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        let keep_running = handle_event(&mut app, event, |_app, _ticker| {});

        assert!(!keep_running);
    }

    #[test]
    fn handle_event_refreshes_on_ctrl_r() {
        let mut app = App::default();
        app.active_ticker = Some("MSFT".to_string());
        let event = Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        let called = Cell::new(false);

        let keep_running = handle_event(&mut app, event, |_app, ticker| {
            called.set(true);
            assert_eq!(ticker, "MSFT");
        });

        assert!(keep_running);
        assert!(called.get());
    }

    #[test]
    fn handle_event_scrolls_analysis_panel_with_arrow_keys() {
        let mut app = App::default();
        app.analysis_scroll = 2;

        let down = Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let up = Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        assert!(handle_event(&mut app, down, |_app, _ticker| {}));
        assert_eq!(app.analysis_scroll, 3);
        assert!(handle_event(&mut app, up, |_app, _ticker| {}));
        assert_eq!(app.analysis_scroll, 2);
    }
}
