use super::state::App;
use crate::api::yahoo::CandleRange;
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
    F: Fn(&mut App, &str, bool),
{
    if let Event::Key(key_event) = event {
        return handle_key_event(app, key_event, fetch_quote);
    }
    true
}

fn handle_key_event<F>(app: &mut App, key_event: KeyEvent, fetch_quote: F) -> bool
where
    F: Fn(&mut App, &str, bool),
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
                fetch_quote(app, &ticker, true);
            }
            true
        }
        (KeyCode::Enter, _) => {
            if let Some(ticker) = normalize_ticker(&app.input) {
                app.input.clear();
                fetch_quote(app, &ticker, true);
            } else {
                app.error = Some(INVALID_TICKER_ERROR.to_string());
            }
            true
        }
        (KeyCode::Tab, modifiers) if modifiers.contains(KeyModifiers::SHIFT) => {
            app.prev_finnhub_dataset();
            true
        }
        (KeyCode::BackTab, _) => {
            app.prev_finnhub_dataset();
            true
        }
        (KeyCode::Tab, _) => {
            app.next_finnhub_dataset();
            true
        }
        (KeyCode::Left, _) => {
            app.prev_finnhub_dataset();
            true
        }
        (KeyCode::Right, _) => {
            app.next_finnhub_dataset();
            true
        }
        (KeyCode::F(1), _) => {
            app.set_active_finnhub_dataset_index(0);
            true
        }
        (KeyCode::F(2), _) => {
            app.set_active_finnhub_dataset_index(1);
            true
        }
        (KeyCode::F(3), _) => {
            app.set_active_finnhub_dataset_index(2);
            true
        }
        (KeyCode::Char('['), _) => {
            app.prev_finnhub_dataset();
            true
        }
        (KeyCode::Char(']'), _) => {
            app.next_finnhub_dataset();
            true
        }
        (KeyCode::Char('d' | 'D'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.set_yahoo_range(CandleRange::Day) {
                if let Some(ticker) = app.active_ticker.clone() {
                    fetch_quote(app, &ticker, false);
                }
            }
            true
        }
        (KeyCode::Char('w' | 'W'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.set_yahoo_range(CandleRange::Week) {
                if let Some(ticker) = app.active_ticker.clone() {
                    fetch_quote(app, &ticker, false);
                }
            }
            true
        }
        (KeyCode::Char('m' | 'M'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.set_yahoo_range(CandleRange::Month) {
                if let Some(ticker) = app.active_ticker.clone() {
                    fetch_quote(app, &ticker, false);
                }
            }
            true
        }
        (KeyCode::Char('y' | 'Y'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.set_yahoo_range(CandleRange::Year) {
                if let Some(ticker) = app.active_ticker.clone() {
                    fetch_quote(app, &ticker, false);
                }
            }
            true
        }
        (KeyCode::Char('a' | 'A'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.set_yahoo_range(CandleRange::All) {
                if let Some(ticker) = app.active_ticker.clone() {
                    fetch_quote(app, &ticker, false);
                }
            }
            true
        }
        (KeyCode::Char('e' | 'E'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
            if app.active_ticker.is_some() {
                app.report_requested = true;
                app.error = None;
            } else {
                app.error = Some("Load a ticker before sending a report email.".to_string());
            }
            true
        }
        (KeyCode::Char(c @ '1'..='9'), _) => {
            let index = (c as usize) - ('1' as usize);
            if index < app.finnhub_datasets.len() {
                app.set_active_finnhub_dataset_index(index);
            }
            true
        }
        (KeyCode::Up, _) => {
            if let Some(dataset) = app.active_finnhub_dataset_mut() {
                dataset.scroll = dataset.scroll.saturating_sub(1);
            }
            true
        }
        (KeyCode::Down, _) => {
            if let Some(dataset) = app.active_finnhub_dataset_mut() {
                dataset.scroll = dataset.scroll.saturating_add(1);
            }
            true
        }
        (KeyCode::PageUp, _) => {
            if let Some(dataset) = app.active_finnhub_dataset_mut() {
                dataset.scroll = dataset.scroll.saturating_sub(8);
            }
            true
        }
        (KeyCode::PageDown, _) => {
            if let Some(dataset) = app.active_finnhub_dataset_mut() {
                dataset.scroll = dataset.scroll.saturating_add(8);
            }
            true
        }
        (KeyCode::Home, _) => {
            if let Some(dataset) = app.active_finnhub_dataset_mut() {
                dataset.scroll = 0;
            }
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

        let keep_running = handle_event(&mut app, event, |_app, _ticker, _run_analysis| {});

        assert!(keep_running);
        assert_eq!(app.input, "Q");
    }

    #[test]
    fn handle_event_accepts_r_character_for_input() {
        let mut app = App::default();
        let event = Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));

        let keep_running = handle_event(&mut app, event, |_app, _ticker, _run_analysis| {});

        assert!(keep_running);
        assert_eq!(app.input, "R");
    }

    #[test]
    fn handle_event_quits_on_escape() {
        let mut app = App::default();
        let event = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        let keep_running = handle_event(&mut app, event, |_app, _ticker, _run_analysis| {});

        assert!(!keep_running);
    }

    #[test]
    fn handle_event_refreshes_on_ctrl_r() {
        let mut app = App::default();
        app.active_ticker = Some("MSFT".to_string());
        let event = Event::Key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        let called = Cell::new(false);

        let keep_running = handle_event(&mut app, event, |_app, ticker, run_analysis| {
            called.set(true);
            assert_eq!(ticker, "MSFT");
            assert!(run_analysis);
        });

        assert!(keep_running);
        assert!(called.get());
    }

    #[test]
    fn handle_event_scrolls_finnhub_dataset_with_arrow_keys() {
        let mut app = App::default();
        if let Some(dataset) = app.active_finnhub_dataset_mut() {
            dataset.scroll = 2;
        }

        let down = Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let up = Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        assert!(handle_event(
            &mut app,
            down,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.finnhub_datasets[0].scroll, 3);
        assert!(handle_event(
            &mut app,
            up,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.finnhub_datasets[0].scroll, 2);
    }

    #[test]
    fn handle_event_cycles_finnhub_datasets_with_navigation_keys() {
        let mut app = App::default();

        let right = Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let f2 = Event::Key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE));
        let f3 = Event::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE));
        let tab = Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let backtab = Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
        let left = Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));

        assert!(handle_event(
            &mut app,
            right,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 1);
        assert!(handle_event(
            &mut app,
            f2,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 1);
        assert!(handle_event(
            &mut app,
            f3,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 2);
        assert!(handle_event(
            &mut app,
            tab,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 3);
        assert!(handle_event(
            &mut app,
            backtab,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 2);
        assert!(handle_event(
            &mut app,
            left,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 1);
    }

    #[test]
    fn handle_event_switches_finnhub_datasets() {
        let mut app = App::default();
        let next = Event::Key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        let jump = Event::Key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        assert!(handle_event(
            &mut app,
            next,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 1);
        assert!(handle_event(
            &mut app,
            jump,
            |_app, _ticker, _run_analysis| {}
        ));
        assert_eq!(app.active_finnhub_dataset_index, 1);
    }

    #[test]
    fn handle_event_brackets_switch_finnhub_without_refreshing_yahoo() {
        let mut app = App::default();
        app.active_ticker = Some("AAPL".to_string());
        let next = Event::Key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        let called = Cell::new(false);

        assert!(handle_event(
            &mut app,
            next,
            |_app, ticker, run_analysis| {
                called.set(true);
                assert_eq!(ticker, "AAPL");
                assert!(!run_analysis);
            }
        ));
        assert!(!called.get());
        assert_eq!(app.active_finnhub_dataset_index, 1);
        assert_eq!(app.yahoo_range, CandleRange::Day);
    }

    #[test]
    fn handle_event_keeps_typing_letters_for_ticker_input() {
        let mut app = App::default();
        app.active_ticker = Some("MSFT".to_string());
        let event = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        let called = Cell::new(false);

        assert!(handle_event(
            &mut app,
            event,
            |_app, _ticker, _run_analysis| {
                called.set(true);
            }
        ));
        assert_eq!(app.input, "A");
        assert!(!called.get());
    }

    #[test]
    fn handle_event_switches_yahoo_range_with_ctrl_hotkey() {
        let mut app = App::default();
        app.active_ticker = Some("AAPL".to_string());
        let event = Event::Key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        let called = Cell::new(false);

        assert!(handle_event(
            &mut app,
            event,
            |_app, ticker, run_analysis| {
                called.set(true);
                assert_eq!(ticker, "AAPL");
                assert!(!run_analysis);
            }
        ));
        assert!(called.get());
        assert_eq!(app.yahoo_range, CandleRange::Week);
    }

    #[test]
    fn handle_event_marks_report_request_on_ctrl_e() {
        let mut app = App::default();
        app.active_ticker = Some("AAPL".to_string());
        let event = Event::Key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));

        assert!(handle_event(
            &mut app,
            event,
            |_app, _ticker, _run_analysis| {}
        ));
        assert!(app.report_requested);
    }
}
