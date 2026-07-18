pub(crate) mod controls;
mod state;
mod view;

use crate::api::ollama::analyze_company_streaming;
use crate::api::yahoo::fetch_quote;
use crossterm::cursor;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use state::App;
use std::error::Error;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use view::draw_ui;

enum AnalysisEvent {
    Progress {
        request_id: u64,
        ticker: String,
        text: String,
    },
    Complete {
        request_id: u64,
        ticker: String,
        result: Result<String, String>,
    },
}

fn queue_analysis_request(app: &mut App, ticker: &str, analysis_tx: &Sender<AnalysisEvent>) {
    app.analysis_request_id = app.analysis_request_id.saturating_add(1);
    app.analysis_loading = true;
    app.analysis = None;
    app.analysis_error = None;
    app.analysis_scroll = 0;

    let request_id = app.analysis_request_id;
    let ticker_for_thread = ticker.to_string();
    let comparison_ticker = app.comparison_ticker.clone();
    let tx = analysis_tx.clone();

    thread::spawn(move || {
        let tx_for_progress = tx.clone();
        let ticker_for_progress = ticker_for_thread.clone();
        let result = analyze_company_streaming(
            &ticker_for_thread,
            comparison_ticker.as_deref(),
            |partial_text| {
                let _ = tx_for_progress.send(AnalysisEvent::Progress {
                    request_id,
                    ticker: ticker_for_progress.clone(),
                    text: partial_text.to_string(),
                });
            },
        )
        .map_err(|err| err.to_string());
        let _ = tx.send(AnalysisEvent::Complete {
            request_id,
            ticker: ticker_for_thread,
            result,
        });
    });
}

fn fetch_and_store_quote(app: &mut App, ticker: &str, analysis_tx: &Sender<AnalysisEvent>) {
    app.active_ticker = Some(ticker.to_string());
    app.error = None;

    match fetch_quote(ticker) {
        Ok(meta) => {
            app.quote = Some(meta);
        }
        Err(err) => {
            app.error = Some(format!("Quote error: {err}"));
            app.quote = None;
        }
    }

    queue_analysis_request(app, ticker, analysis_tx);
}

fn apply_analysis_event(app: &mut App, event: AnalysisEvent) {
    match event {
        AnalysisEvent::Progress {
            request_id,
            ticker,
            text,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            app.analysis = Some(text);
            app.analysis_error = None;
        }
        AnalysisEvent::Complete {
            request_id,
            ticker,
            result,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            app.analysis_loading = false;
            match result {
                Ok(analysis) => {
                    app.analysis = Some(analysis);
                    app.analysis_error = None;
                }
                Err(err) => {
                    app.analysis_error = Some(err.clone());
                    app.error = Some(format!("Analysis error: {err}"));
                }
            }
        }
    }
}

fn drain_analysis_events(app: &mut App, analysis_rx: &Receiver<AnalysisEvent>) {
    while let Ok(event) = analysis_rx.try_recv() {
        apply_analysis_event(app, event);
    }
}

pub(crate) fn run_tui(initial_ticker: Option<String>) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = (|| -> Result<(), Box<dyn Error>> {
        let (analysis_tx, analysis_rx) = mpsc::channel::<AnalysisEvent>();
        let mut app = App::default();

        if let Some(ticker) = initial_ticker {
            fetch_and_store_quote(&mut app, &ticker, &analysis_tx);
        }

        loop {
            drain_analysis_events(&mut app, &analysis_rx);
            terminal.draw(|frame| draw_ui(frame, &app))?;

            if !event::poll(Duration::from_millis(250))? {
                app.input_cursor_visible = !app.input_cursor_visible;
                continue;
            }

            let event = event::read()?;
            if !controls::handle_event(&mut app, event, |app, ticker| {
                fetch_and_store_quote(app, ticker, &analysis_tx)
            }) {
                break;
            }
        }

        Ok(())
    })();

    let cleanup_result = (|| -> io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            cursor::Show,
            terminal::LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;
        Ok(())
    })();

    cleanup_result?;
    run_result
}
