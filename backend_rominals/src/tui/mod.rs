pub(crate) mod controls;
mod state;
mod view;

use crate::api::alpha_vantage::fetch_snapshot_context;
use crate::api::ollama::analyze_company_streaming;
use crate::api::yahoo::fetch_quote;
use crossterm::cursor;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use serde_json::Value;
use state::App;
use std::error::Error;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use view::draw_ui;

enum AnalysisEvent {
    AlphaSnapshot {
        request_id: u64,
        ticker: String,
        fundamentals_summary: String,
        news_summary: String,
    },
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

fn compact_money(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "n/a".to_string();
    };

    let abs = value.abs();
    if abs >= 1_000_000_000_000.0 {
        format!("{:.2}T", value / 1_000_000_000_000.0)
    } else if abs >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else {
        format!("{value:.2}")
    }
}

fn number_field(obj: &Value, key: &str) -> Option<f64> {
    let value = obj.get(key)?;
    if let Some(n) = value.as_f64() {
        return Some(n);
    }

    value.as_str().and_then(|s| s.trim().parse::<f64>().ok())
}

fn string_field(obj: &Value, key: &str) -> Option<String> {
    obj.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn summarize_alpha_snapshots(context_json: &str) -> (String, String) {
    let parsed: Value = match serde_json::from_str(context_json) {
        Ok(value) => value,
        Err(err) => {
            return (
                format!("Alpha Fundamentals Snapshot\nParse error: {err}"),
                format!("Alpha News Snapshot\nParse error: {err}"),
            );
        }
    };

    let overview = parsed.get("overview").unwrap_or(&Value::Null);
    let financials = parsed.get("financials").unwrap_or(&Value::Null);
    let news_feed = parsed
        .get("news")
        .and_then(|news| news.get("articles"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let warnings = parsed
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut fundamentals_lines = Vec::new();
    fundamentals_lines.push("Alpha Fundamentals Snapshot".to_string());

    let name = string_field(overview, "name").unwrap_or_else(|| "n/a".to_string());
    let sector = string_field(overview, "sector").unwrap_or_else(|| "n/a".to_string());
    let industry = string_field(overview, "industry").unwrap_or_else(|| "n/a".to_string());
    fundamentals_lines.push(format!(
        "Name: {name} | Sector: {sector} | Industry: {industry}"
    ));

    let mcap = compact_money(number_field(overview, "market_cap"));
    let pe = number_field(overview, "pe_ratio")
        .map(|v| format!("{v:.2}"))
        .unwrap_or_else(|| "n/a".to_string());
    let ps = number_field(overview, "price_to_sales_ttm")
        .map(|v| format!("{v:.2}"))
        .unwrap_or_else(|| "n/a".to_string());
    fundamentals_lines.push(format!("MCap: {mcap} | P/E: {pe} | P/S: {ps}"));

    let revenue_ttm = compact_money(number_field(overview, "revenue_ttm"));
    let fcf = compact_money(number_field(financials, "latest_free_cash_flow"));
    let cash = compact_money(number_field(financials, "total_cash_and_equivalents"));
    let debt = compact_money(number_field(financials, "total_debt"));
    fundamentals_lines.push(format!(
        "Revenue TTM: {revenue_ttm} | FCF: {fcf} | Cash: {cash} | Debt: {debt}"
    ));

    let mut news_lines = vec!["Alpha News Snapshot".to_string()];
    if news_feed.is_empty() {
        news_lines.push("No news items available.".to_string());
    } else {
        for (index, item) in news_feed.iter().take(3).enumerate() {
            let sentiment =
                string_field(item, "overall_sentiment_label").unwrap_or_else(|| "n/a".to_string());
            let score = number_field(item, "overall_sentiment_score")
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "n/a".to_string());
            let source = string_field(item, "source").unwrap_or_else(|| "n/a".to_string());
            let title = string_field(item, "title").unwrap_or_else(|| "n/a".to_string());
            news_lines.push(format!("{}. [{source}] {sentiment} ({score})", index + 1));
            news_lines.push(format!("   {title}"));
        }
    }

    if !warnings.is_empty() {
        fundamentals_lines.push(format!(
            "Warnings: {} endpoint(s) partial/unavailable",
            warnings.len()
        ));
        news_lines.push(format!(
            "Warnings: {} endpoint(s) partial/unavailable",
            warnings.len()
        ));
    }

    (fundamentals_lines.join("\n"), news_lines.join("\n"))
}

fn queue_analysis_request(app: &mut App, ticker: &str, analysis_tx: &Sender<AnalysisEvent>) {
    app.analysis_request_id = app.analysis_request_id.saturating_add(1);
    app.analysis_loading = true;
    app.alpha_fundamentals_snapshot = None;
    app.alpha_news_snapshot = None;
    app.analysis = None;
    app.analysis_error = None;
    app.alpha_scroll = 0;
    app.ollama_scroll = 0;

    let request_id = app.analysis_request_id;
    let ticker_for_thread = ticker.to_string();
    let comparison_ticker = app.comparison_ticker.clone();
    let tx = analysis_tx.clone();

    thread::spawn(move || {
        let _ = tx.send(AnalysisEvent::Progress {
            request_id,
            ticker: ticker_for_thread.clone(),
            text: "Bootstrapping research pipeline...\n- Loading Alpha Vantage snapshot"
                .to_string(),
        });

        let alpha_context = match fetch_snapshot_context(&ticker_for_thread) {
            Ok(context) => {
                let message = if context.is_some() {
                    "Bootstrapping research pipeline...\n- Alpha Vantage snapshot loaded\n- Starting parallel Ollama workers"
                } else {
                    "Bootstrapping research pipeline...\n- ALPHAVANTAGE_API_KEY not set; continuing with Ollama web search only"
                };
                let _ = tx.send(AnalysisEvent::Progress {
                    request_id,
                    ticker: ticker_for_thread.clone(),
                    text: message.to_string(),
                });
                if let Some(context_json) = &context {
                    let (fundamentals_summary, news_summary) =
                        summarize_alpha_snapshots(context_json);
                    let _ = tx.send(AnalysisEvent::AlphaSnapshot {
                        request_id,
                        ticker: ticker_for_thread.clone(),
                        fundamentals_summary,
                        news_summary,
                    });
                } else {
                    let _ = tx.send(AnalysisEvent::AlphaSnapshot {
                        request_id,
                        ticker: ticker_for_thread.clone(),
                        fundamentals_summary:
                            "Alpha Fundamentals Snapshot\nUnavailable (missing ALPHAVANTAGE_API_KEY)."
                                .to_string(),
                        news_summary:
                            "Alpha News Snapshot\nUnavailable (missing ALPHAVANTAGE_API_KEY)."
                                .to_string(),
                    });
                }
                context
            }
            Err(err) => {
                let _ = tx.send(AnalysisEvent::Progress {
                    request_id,
                    ticker: ticker_for_thread.clone(),
                    text: format!(
                        "Bootstrapping research pipeline...\n- Alpha Vantage unavailable: {err}\n- Continuing with Ollama web search"
                    ),
                });
                let _ = tx.send(AnalysisEvent::AlphaSnapshot {
                    request_id,
                    ticker: ticker_for_thread.clone(),
                    fundamentals_summary: format!(
                        "Alpha Fundamentals Snapshot\nUnavailable: {err}"
                    ),
                    news_summary: format!("Alpha News Snapshot\nUnavailable: {err}"),
                });
                None
            }
        };

        let tx_for_progress = tx.clone();
        let ticker_for_progress = ticker_for_thread.clone();
        let result = analyze_company_streaming(
            &ticker_for_thread,
            comparison_ticker.as_deref(),
            alpha_context.as_deref(),
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
        AnalysisEvent::AlphaSnapshot {
            request_id,
            ticker,
            fundamentals_summary,
            news_summary,
        } => {
            if request_id != app.analysis_request_id
                || app.active_ticker.as_deref() != Some(ticker.as_str())
            {
                return;
            }

            app.alpha_fundamentals_snapshot = Some(fundamentals_summary);
            app.alpha_news_snapshot = Some(news_summary);
        }
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
