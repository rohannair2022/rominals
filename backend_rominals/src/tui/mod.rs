pub(crate) mod controls;
mod state;
mod view;

use crate::api::finnhub::{FinnhubSnapshot, fetch_finnhub_snapshot};
use crate::api::mlx::{
    WorkerSectionChunk, WorkerSectionOutput, analyze_company_workers, preload_mlx_model,
    summarize_terminal_report,
};
use crate::api::report::{ReportEmailConfig, send_report_email};
use crate::api::yahoo::{CandleRange, QuoteSnapshot, build_analysis_context, fetch_quote_snapshot};
use chrono::Utc;
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
use std::time::{Duration, Instant};
use view::draw_ui;

const YAHOO_STREAM_POLL_INTERVAL: Duration = Duration::from_secs(2);
const REPORT_EMAIL_SUBJECT_PREFIX: &str = "[Rominals AI Report]";
const REPORT_SECTION_CHAR_LIMIT: usize = 1_400;
const REPORT_DATASET_CHAR_LIMIT: usize = 700;
const REPORT_EMAIL_MIN_WORDS: usize = 300;
const REPORT_EMAIL_MAX_WORDS: usize = 400;
const REPORT_PREVIEW_CHAR_LIMIT: usize = 4_000;

#[derive(Clone, Debug)]
struct ReportDeliveryReceipt {
    recipient: String,
    subject: String,
    sent_at_utc: String,
}

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
    YahooComplete {
        request_id: u64,
        ticker: String,
        result: Result<QuoteSnapshot, String>,
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
    ReportStatus {
        request_id: u64,
        text: String,
    },
    ReportPreview {
        request_id: u64,
        recipient: String,
        subject: String,
        preview: String,
    },
    ReportComplete {
        request_id: u64,
        result: Result<ReportDeliveryReceipt, String>,
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
    yahoo_range: CandleRange,
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
        let tx_for_yahoo = tx.clone();
        let ticker_for_yahoo_event = ticker_for_thread.clone();
        let ticker_for_yahoo_fetch = ticker_for_thread.clone();
        let mut macro_context = String::new();
        let mut finnhub_micro_context = String::new();

        let _ = tx_for_status.send(AnalysisEvent::Status {
            request_id,
            ticker: ticker_for_status.clone(),
            text: "Fetching Yahoo snapshot + Finnhub datasets...".to_string(),
        });
        let yahoo_handle = thread::spawn(move || {
            fetch_quote_snapshot(&ticker_for_yahoo_fetch, yahoo_range)
                .map_err(|err| err.to_string())
        });

        let _ = tx_for_finnhub.send(AnalysisEvent::FinnhubStatus {
            request_id,
            ticker: ticker_for_finnhub.clone(),
            text: "Fetching Finnhub datasets...".to_string(),
        });

        match fetch_finnhub_snapshot(&ticker_for_thread) {
            Ok(snapshot) => {
                macro_context = snapshot.macro_context.clone();
                if !snapshot.micro_context.trim().is_empty() {
                    finnhub_micro_context = snapshot.micro_context.clone();
                }
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

        let mut micro_context = String::new();
        let yahoo_result = match yahoo_handle.join() {
            Ok(result) => result,
            Err(_) => Err("Yahoo fetch thread panicked.".to_string()),
        };

        match yahoo_result {
            Ok(snapshot) => {
                micro_context = build_analysis_context(&snapshot.meta);
                let _ = tx_for_yahoo.send(AnalysisEvent::YahooComplete {
                    request_id,
                    ticker: ticker_for_yahoo_event.clone(),
                    result: Ok(snapshot),
                });
            }
            Err(err) => {
                let _ = tx_for_yahoo.send(AnalysisEvent::YahooComplete {
                    request_id,
                    ticker: ticker_for_yahoo_event.clone(),
                    result: Err(err),
                });
            }
        }

        if !finnhub_micro_context.trim().is_empty() {
            if !micro_context.trim().is_empty() {
                micro_context.push_str("\n\n");
            }
            micro_context.push_str(&finnhub_micro_context);
        }

        let macro_prompt_context = if macro_context.trim().is_empty() {
            None
        } else {
            Some(macro_context)
        };
        let micro_prompt_context = if micro_context.trim().is_empty() {
            None
        } else {
            Some(micro_context)
        };

        let result = analyze_company_workers(
            &ticker_for_thread,
            comparison_ticker.as_deref(),
            macro_prompt_context.as_deref(),
            micro_prompt_context.as_deref(),
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

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max_chars).collect();
    format!(
        "{clipped}\n...[truncated {} chars]",
        char_count.saturating_sub(max_chars)
    )
}

fn sanitize_worker_output(text: &str) -> String {
    let no_tags = text.replace("<think>", "").replace("</think>", "");
    let filtered = no_tags
        .lines()
        .filter(|line| !(line.contains("tokens generated in") && line.contains("tokens/sec")))
        .collect::<Vec<_>>()
        .join("\n");
    filtered.trim().to_string()
}

fn safe_ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if denominator.abs() <= f64::EPSILON {
        None
    } else {
        Some(numerator / denominator)
    }
}

fn format_opt_price(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_opt_percent(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:+.2} percent"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn take_first_words(text: &str, max_words: usize) -> String {
    if max_words == 0 {
        return String::new();
    }
    text.split_whitespace()
        .take(max_words)
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_report_text(text: &str) -> String {
    let mut lines = Vec::new();

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if lines.last().is_some_and(String::is_empty) {
                continue;
            }
            lines.push(String::new());
            continue;
        }

        let line = trimmed
            .trim_start_matches(|ch: char| matches!(ch, '*' | '#' | '•' | '>' | '`'))
            .trim_start_matches("- ")
            .replace("TL;DR", "TLDR")
            .replace('%', " percent")
            .replace('|', " ")
            .replace("  ", " ");
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            lines.push(collapsed);
        }
    }

    lines.join("\n").trim().to_string()
}

fn enforce_email_word_range(base_text: &str, supplemental_text: &str) -> String {
    let mut output = normalize_report_text(base_text);
    if output.is_empty() {
        return output;
    }

    if word_count(&output) > REPORT_EMAIL_MAX_WORDS {
        output = take_first_words(&output, REPORT_EMAIL_MAX_WORDS);
    }

    let current_words = word_count(&output);
    if current_words < REPORT_EMAIL_MIN_WORDS {
        let extra_needed = REPORT_EMAIL_MIN_WORDS.saturating_sub(current_words);
        let supplemental = normalize_report_text(supplemental_text);
        if !supplemental.is_empty() {
            let supplemental_clip = take_first_words(&supplemental, extra_needed + 20);
            if !supplemental_clip.is_empty() {
                output.push_str("\n\nAdditional data context\n");
                output.push_str(&supplemental_clip);
            }
        }
    }

    if word_count(&output) > REPORT_EMAIL_MAX_WORDS {
        output = take_first_words(&output, REPORT_EMAIL_MAX_WORDS);
    }

    output
}

fn format_key_value_table(rows: &[(&str, String)]) -> String {
    let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    rows.iter()
        .map(|(label, value)| format!("{label:<width$} : {value}", width = width))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_report_data_snapshot(app: &App, ticker: &str) -> String {
    let mut rows: Vec<(&str, String)> = vec![("Ticker", ticker.to_string())];
    let ready_count = app
        .finnhub_datasets
        .iter()
        .filter(|dataset| dataset.content.is_some() && !dataset.is_error)
        .count();
    let error_count = app
        .finnhub_datasets
        .iter()
        .filter(|dataset| dataset.is_error)
        .count();

    if let Some(meta) = &app.quote {
        let day_change_pct = match (meta.regular_market_price, meta.chart_previous_close) {
            (Some(price), Some(prev)) => safe_ratio(price - prev, prev).map(|value| value * 100.0),
            _ => None,
        };
        let yearly_position_pct = match (
            meta.regular_market_price,
            meta.fifty_two_week_low,
            meta.fifty_two_week_high,
        ) {
            (Some(price), Some(low), Some(high)) => safe_ratio(price - low, high - low)
                .map(|value| value * 100.0)
                .filter(|value| value.is_finite()),
            _ => None,
        };

        rows.extend([
            ("Current price", format_opt_price(meta.regular_market_price)),
            (
                "Previous close",
                format_opt_price(meta.chart_previous_close),
            ),
            ("Day move", format_opt_percent(day_change_pct)),
            (
                "Intraday range",
                format!(
                    "{} to {}",
                    format_opt_price(meta.regular_market_day_low),
                    format_opt_price(meta.regular_market_day_high)
                ),
            ),
            (
                "52 week range",
                format!(
                    "{} to {}",
                    format_opt_price(meta.fifty_two_week_low),
                    format_opt_price(meta.fifty_two_week_high)
                ),
            ),
            (
                "52 week position",
                format_opt_percent(yearly_position_pct.map(|value| value.abs())),
            ),
            (
                "Volume",
                meta.regular_market_volume
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "n/a".to_string()),
            ),
        ]);
    } else {
        rows.push(("Yahoo quote", "not loaded yet".to_string()));
    }

    rows.push((
        "Finnhub datasets ready",
        format!("{ready_count} of {}", app.finnhub_datasets.len()),
    ));
    rows.push(("Finnhub datasets error", error_count.to_string()));

    if let Some(dataset) = app.finnhub_datasets.get(app.active_finnhub_dataset_index) {
        let focus_status = if dataset.content.is_some() && !dataset.is_error {
            "ready"
        } else if dataset.is_error {
            "error"
        } else {
            "waiting"
        };
        rows.push((
            "Primary dataset focus",
            format!("{} ({focus_status})", dataset.title),
        ));
    }

    format_key_value_table(&rows)
}

fn build_report_prompt_context(app: &App, ticker: &str, data_snapshot: &str) -> String {
    let mut sections = vec![format!(
        "Session timestamp UTC: {}\nTicker: {ticker}\n\nFormatted data snapshot\n{data_snapshot}",
        Utc::now().to_rfc3339()
    )];

    if let Some(dataset) = app.finnhub_datasets.get(app.active_finnhub_dataset_index) {
        let excerpt = dataset
            .content
            .as_deref()
            .map(|value| truncate_chars(value, REPORT_DATASET_CHAR_LIMIT))
            .unwrap_or_else(|| "No dataset content available.".to_string());
        sections.push(format!(
            "Active Finnhub dataset\nTitle: {}\nContent excerpt:\n{}",
            dataset.title, excerpt
        ));
    }

    if let Some(section) = app
        .mlx_sections
        .first()
        .and_then(|value| value.content.as_deref())
    {
        sections.push(format!(
            "Macro worker output\n{}",
            truncate_chars(&sanitize_worker_output(section), REPORT_SECTION_CHAR_LIMIT)
        ));
    }
    if let Some(section) = app
        .mlx_sections
        .get(1)
        .and_then(|value| value.content.as_deref())
    {
        sections.push(format!(
            "Micro worker output\n{}",
            truncate_chars(&sanitize_worker_output(section), REPORT_SECTION_CHAR_LIMIT)
        ));
    }

    sections.join("\n\n")
}

fn queue_report_email_request(app: &mut App, analysis_tx: &Sender<AnalysisEvent>) {
    app.report_requested = false;

    if app.report_loading {
        app.report_status = Some("Report delivery is already in progress.".to_string());
        return;
    }
    if app.analysis_loading {
        app.report_status =
            Some("Wait for the analysis run to complete, then press Ctrl+E again.".to_string());
        return;
    }

    let Some(ticker) = app.active_ticker.clone() else {
        app.error = Some("Load a ticker before sending a report email.".to_string());
        return;
    };

    app.report_request_id = app.report_request_id.saturating_add(1);
    app.report_loading = true;
    app.report_status = Some("Preparing report context...".to_string());
    app.report_preview = None;

    let request_id = app.report_request_id;
    let data_snapshot = build_report_data_snapshot(app, &ticker);
    let report_context = build_report_prompt_context(app, &ticker, &data_snapshot);
    let supplemental_context = format!(
        "Data snapshot for reference. {}",
        data_snapshot.replace('\n', ". ")
    );
    let tx = analysis_tx.clone();

    thread::spawn(move || {
        let _ = tx.send(AnalysisEvent::ReportStatus {
            request_id,
            text: "Generating AI summary with MLX...".to_string(),
        });

        let config = match ReportEmailConfig::from_env() {
            Ok(config) => config,
            Err(err) => {
                let _ = tx.send(AnalysisEvent::ReportComplete {
                    request_id,
                    result: Err(format!(
                        "{err}. Required setup: ROMINALS_GMAIL_USER and ROMINALS_GMAIL_APP_PASSWORD (plus optional ROMINALS_REPORT_TO)."
                    )),
                });
                return;
            }
        };

        let summary = match summarize_terminal_report(&report_context) {
            Ok(summary) => summary,
            Err(err) => {
                let _ = tx.send(AnalysisEvent::ReportComplete {
                    request_id,
                    result: Err(format!("LLM summary generation failed: {err}")),
                });
                return;
            }
        };

        let generated_at = Utc::now();
        let subject = format!(
            "{REPORT_EMAIL_SUBJECT_PREFIX} {ticker} {}",
            generated_at.format("%Y-%m-%d %H:%M UTC")
        );
        let clean_summary = normalize_report_text(&summary);
        let raw_body = format!(
            "Rominals AI market report\n\
Generated at UTC: {}\n\
\n\
Market data snapshot\n\
{data_snapshot}\n\
\n\
AI TLDR and insights\n\
{clean_summary}",
            generated_at.to_rfc3339()
        );
        let body = enforce_email_word_range(&raw_body, &supplemental_context);
        let preview = truncate_chars(
            &format!("To: {}\nSubject: {subject}\n\n{body}", config.recipient),
            REPORT_PREVIEW_CHAR_LIMIT,
        );

        let _ = tx.send(AnalysisEvent::ReportPreview {
            request_id,
            recipient: config.recipient.clone(),
            subject: subject.clone(),
            preview,
        });
        let _ = tx.send(AnalysisEvent::ReportStatus {
            request_id,
            text: "Sending email via Gmail SMTP...".to_string(),
        });

        match send_report_email(&config, &subject, &body) {
            Ok(()) => {
                let _ = tx.send(AnalysisEvent::ReportComplete {
                    request_id,
                    result: Ok(ReportDeliveryReceipt {
                        recipient: config.recipient,
                        subject,
                        sent_at_utc: generated_at.to_rfc3339(),
                    }),
                });
            }
            Err(err) => {
                let _ = tx.send(AnalysisEvent::ReportComplete {
                    request_id,
                    result: Err(format!("Email delivery failed: {err}")),
                });
            }
        }
    });
}

fn fetch_and_store_quote(
    app: &mut App,
    ticker: &str,
    analysis_tx: &Sender<AnalysisEvent>,
    run_analysis: bool,
) {
    app.active_ticker = Some(ticker.to_string());
    app.error = None;
    app.prune_yahoo_live_prices(Utc::now().timestamp_millis());

    if run_analysis {
        app.quote = None;
        app.yahoo_candles.clear();
        app.yahoo_live_prices.clear();
        queue_analysis_request(app, ticker, analysis_tx, app.yahoo_range);
        return;
    }

    match fetch_quote_snapshot(ticker, app.yahoo_range) {
        Ok(snapshot) => {
            if let Some(price) = snapshot.meta.regular_market_price {
                app.push_yahoo_live_price(Utc::now().timestamp_millis(), price);
            }
            app.quote = Some(snapshot.meta);
            app.yahoo_candles = snapshot.candles;
        }
        Err(err) => {
            app.error = Some(format!("Quote error: {err}"));
        }
    }
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
        AnalysisEvent::YahooComplete {
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
                    if let Some(price) = snapshot.meta.regular_market_price {
                        app.push_yahoo_live_price(Utc::now().timestamp_millis(), price);
                    }
                    app.quote = Some(snapshot.meta);
                    app.yahoo_candles = snapshot.candles;
                }
                Err(err) => {
                    app.error = Some(format!("Quote error: {err}"));
                    app.quote = None;
                    app.yahoo_candles.clear();
                    app.yahoo_live_prices.clear();
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
        AnalysisEvent::ReportStatus { request_id, text } => {
            if request_id != app.report_request_id {
                return;
            }
            app.report_status = Some(text);
        }
        AnalysisEvent::ReportPreview {
            request_id,
            recipient,
            subject,
            preview,
        } => {
            if request_id != app.report_request_id {
                return;
            }
            app.report_last_sent_to = Some(recipient);
            app.report_last_subject = Some(subject);
            app.report_preview = Some(preview);
        }
        AnalysisEvent::ReportComplete { request_id, result } => {
            if request_id != app.report_request_id {
                return;
            }
            app.report_loading = false;
            match result {
                Ok(receipt) => {
                    app.report_status = Some("Report email sent successfully.".to_string());
                    app.report_last_sent_to = Some(receipt.recipient);
                    app.report_last_subject = Some(receipt.subject);
                    app.report_last_sent_at = Some(receipt.sent_at_utc);
                }
                Err(err) => {
                    app.report_status = Some(format!("Report email failed: {err}"));
                    app.error = Some(format!("Report error: {err}"));
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
        let mut last_stream_poll = Instant::now();

        if let Some(ticker) = initial_ticker {
            fetch_and_store_quote(&mut app, &ticker, &analysis_tx, true);
            last_stream_poll = Instant::now();
        } else {
            queue_model_preload(&analysis_tx);
        }

        loop {
            drain_analysis_events(&mut app, &analysis_rx);
            terminal.draw(|frame| draw_ui(frame, &app))?;

            if let Some(ticker) = app.active_ticker.clone() {
                if last_stream_poll.elapsed() >= YAHOO_STREAM_POLL_INTERVAL {
                    fetch_and_store_quote(&mut app, &ticker, &analysis_tx, false);
                    last_stream_poll = Instant::now();
                }
            }

            if !event::poll(Duration::from_millis(50))? {
                app.input_cursor_visible = !app.input_cursor_visible;
                continue;
            }

            let event = event::read()?;
            let keep_running =
                controls::handle_event(&mut app, event, |app, ticker, run_analysis| {
                    fetch_and_store_quote(app, ticker, &analysis_tx, run_analysis)
                });

            if app.report_requested {
                queue_report_email_request(&mut app, &analysis_tx);
            }

            if !keep_running {
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
