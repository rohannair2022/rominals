pub(crate) mod controls;
mod state;
mod view;

use crate::api::finnhub::{FinnhubSnapshot, fetch_finnhub_snapshot};
use crate::api::mlx::{
    WorkerSectionChunk, WorkerSectionOutput, analyze_company_workers, preload_mlx_model,
};
use crate::api::yahoo::{build_analysis_context, fetch_quote};
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
    PreloadStatus {
        text: String,
    },
    PreloadComplete {
        result: Result<(), String>,
    },
    Status {
        request_id: u64,
        ticker: String,
        text: String,
    },
    FinnhubStatus {
        request_id: u64,
        ticker: String,
        text: String,
    },
    FinnhubComplete {
        request_id: u64,
        ticker: String,
        result: Result<FinnhubSnapshot, String>,
    },
    SectionStream {
        request_id: u64,
        ticker: String,
        section: WorkerSectionChunk,
    },
    SectionComplete {
        request_id: u64,
        ticker: String,
        section: WorkerSectionOutput,
    },
    Complete {
        request_id: u64,
        ticker: String,
        result: Result<(), String>,
    },
}

fn queue_model_preload(analysis_tx: &Sender<AnalysisEvent>) {
    let tx = analysis_tx.clone();

    thread::spawn(move || {
        let tx_for_status = tx.clone();
        let result = preload_mlx_model(|status_text| {
            let _ = tx_for_status.send(AnalysisEvent::PreloadStatus {
                text: status_text.to_string(),
            });
        })
        .map_err(|err| err.to_string());

        let _ = tx.send(AnalysisEvent::PreloadComplete { result });
    });
}

fn queue_analysis_request(
    app: &mut App,
    ticker: &str,
    analysis_tx: &Sender<AnalysisEvent>,
    snapshot_context: Option<String>,
) {
    app.analysis_request_id = app.analysis_request_id.saturating_add(1);
    app.analysis_loading = true;
    app.analysis_error = None;
    app.mlx_status = Some("Bootstrapping worker pipeline...".to_string());
    app.reset_mlx_sections();
    app.finnhub_status = Some("Preparing Finnhub dataset fetch...".to_string());
    app.reset_finnhub_datasets();

    let request_id = app.analysis_request_id;
    let ticker_for_thread = ticker.to_string();
    let comparison_ticker = app.comparison_ticker.clone();
    let tx = analysis_tx.clone();

    thread::spawn(move || {
        let tx_for_status = tx.clone();
        let ticker_for_status = ticker_for_thread.clone();
        let tx_for_stream = tx.clone();
        let ticker_for_stream = ticker_for_thread.clone();
        let tx_for_section = tx.clone();
        let ticker_for_section = ticker_for_thread.clone();
        let tx_for_finnhub = tx.clone();
        let ticker_for_finnhub = ticker_for_thread.clone();
        let mut combined_context = snapshot_context.unwrap_or_default();

        let _ = tx_for_finnhub.send(AnalysisEvent::FinnhubStatus {
            request_id,
            ticker: ticker_for_finnhub.clone(),
            text: "Fetching Finnhub datasets...".to_string(),
        });

        match fetch_finnhub_snapshot(&ticker_for_thread) {
            Ok(snapshot) => {
                if !combined_context.trim().is_empty() {
                    combined_context.push_str("\n\n");
                }
                combined_context.push_str(&snapshot.context);
                let _ = tx_for_finnhub.send(AnalysisEvent::FinnhubComplete {
                    request_id,
                    ticker: ticker_for_finnhub.clone(),
                    result: Ok(snapshot),
                });
            }
            Err(err) => {
                let _ = tx_for_finnhub.send(AnalysisEvent::FinnhubComplete {
                    request_id,
                    ticker: ticker_for_finnhub.clone(),
                    result: Err(err.to_string()),
                });
            }
        }

        let merged_context = if combined_context.trim().is_empty() {
            None
        } else {
            Some(combined_context)
        };

        let result = analyze_company_workers(
            &ticker_for_thread,
            comparison_ticker.as_deref(),
            merged_context.as_deref(),
            |status_text| {
                let _ = tx_for_status.send(AnalysisEvent::Status {
                    request_id,
                    ticker: ticker_for_status.clone(),
                    text: status_text.to_string(),
                });
            },
            |section_chunk| {
                let _ = tx_for_stream.send(AnalysisEvent::SectionStream {
                    request_id,
                    ticker: ticker_for_stream.clone(),
                    section: section_chunk.clone(),
                });
            },
            |section| {
                let _ = tx_for_section.send(AnalysisEvent::SectionComplete {
                    request_id,
                    ticker: ticker_for_section.clone(),
                    section: section.clone(),
                });
            },
        )
        .map(|_| ())
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
    let mut snapshot_context: Option<String> = None;

    match fetch_quote(ticker) {
        Ok(meta) => {
            snapshot_context = Some(build_analysis_context(&meta));
            app.quote = Some(meta);
        }
        Err(err) => {
            app.error = Some(format!("Quote error: {err}"));
            app.quote = None;
        }
    }

    queue_analysis_request(app, ticker, analysis_tx, snapshot_context);
}

fn apply_analysis_event(app: &mut App, event: AnalysisEvent) {
    match event {
        AnalysisEvent::PreloadStatus { text } => {
            if !app.analysis_loading {
                app.mlx_status = Some(text);
            }
        }
        AnalysisEvent::PreloadComplete { result } => {
            if app.analysis_loading {
                return;
            }

            match result {
                Ok(()) => {
                    app.mlx_status = Some("MLX model preloaded and ready.".to_string());
                }
                Err(err) => {
                    app.error = Some(format!("MLX preload warning: {err}"));
                }
            }
        }
        AnalysisEvent::Status {
            request_id,
            ticker,
            text,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            app.mlx_status = Some(text);
            app.analysis_error = None;
        }
        AnalysisEvent::FinnhubStatus {
            request_id,
            ticker,
            text,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            app.finnhub_status = Some(text);
        }
        AnalysisEvent::FinnhubComplete {
            request_id,
            ticker,
            result,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            match result {
                Ok(snapshot) => {
                    app.finnhub_status = Some(
                        "Finnhub datasets loaded. Use the Finnhub tab to inspect each endpoint."
                            .to_string(),
                    );
                    app.apply_finnhub_snapshot(snapshot);
                }
                Err(err) => {
                    app.finnhub_status = Some(format!("Finnhub unavailable: {err}"));
                    app.reset_finnhub_datasets();
                }
            }
        }
        AnalysisEvent::SectionStream {
            request_id,
            ticker,
            section,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            if let Some(slot) = app.mlx_sections.get_mut(section.index) {
                slot.title = section.title;
                let content = slot.content.get_or_insert_with(String::new);
                content.push_str(&section.chunk);
            }
            app.analysis_error = None;
        }
        AnalysisEvent::SectionComplete {
            request_id,
            ticker,
            section,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            if let Some(slot) = app.mlx_sections.get_mut(section.index) {
                slot.title = section.title;
                slot.content = Some(section.content);
                slot.scroll = 0;
            }
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
                Ok(()) => {
                    app.mlx_status = Some("All worker sections complete.".to_string());
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
        } else {
            queue_model_preload(&analysis_tx);
        }

        loop {
            drain_analysis_events(&mut app, &analysis_rx);
            terminal.draw(|frame| draw_ui(frame, &app))?;

            if !event::poll(Duration::from_millis(50))? {
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
