use super::state::{App, YahooLivePoint};
use crate::api::yahoo::{Candle, CandleRange, Meta};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table, Tabs, Wrap,
};
use serde_json::Value;

fn money(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.2}"),
        None => "n/a".to_string(),
    }
}

fn display_name(meta: &Meta) -> String {
    meta.long_name
        .as_deref()
        .or(meta.short_name.as_deref())
        .unwrap_or(&meta.symbol)
        .to_string()
}

fn change_text(meta: &Meta) -> String {
    match (meta.regular_market_price, meta.chart_previous_close) {
        (Some(price), Some(previous_close)) if previous_close != 0.0 => {
            let delta = price - previous_close;
            let pct = delta / previous_close * 100.0;
            let sign = if delta >= 0.0 { "+" } else { "" };
            format!("{sign}{delta:.2} ({sign}{pct:.2}%)")
        }
        _ => "n/a".to_string(),
    }
}

fn change_style(meta: &Meta) -> Style {
    match (meta.regular_market_price, meta.chart_previous_close) {
        (Some(price), Some(previous_close)) if previous_close != 0.0 => {
            if price - previous_close >= 0.0 {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            }
        }
        _ => Style::default().fg(Color::DarkGray),
    }
}

fn range_text(low: Option<f64>, high: Option<f64>) -> String {
    format!("{} - {}", money(low), money(high))
}

fn label_cell(label: &'static str) -> Cell<'static> {
    Cell::from(Span::styled(
        label,
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

fn labeled_row(label: &'static str, value: String) -> Row<'static> {
    Row::new(vec![label_cell(label), Cell::from(value)])
}

fn quote_rows(meta: &Meta) -> Vec<Row<'static>> {
    let mut rows = vec![
        labeled_row("Symbol", meta.symbol.clone()),
        labeled_row(
            "Exchange",
            meta.full_exchange_name
                .clone()
                .unwrap_or_else(|| "n/a".to_string()),
        ),
        labeled_row(
            "Currency",
            meta.currency.clone().unwrap_or_else(|| "n/a".to_string()),
        ),
        labeled_row("Price", money(meta.regular_market_price)),
        labeled_row("Prev close", money(meta.chart_previous_close)),
        labeled_row(
            "Day range",
            range_text(meta.regular_market_day_low, meta.regular_market_day_high),
        ),
        labeled_row(
            "52-week range",
            range_text(meta.fifty_two_week_low, meta.fifty_two_week_high),
        ),
        labeled_row(
            "Volume",
            meta.regular_market_volume
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
        ),
    ];

    rows.insert(
        4,
        Row::new(vec![
            label_cell("Change"),
            Cell::from(Span::styled(change_text(meta), change_style(meta))),
        ]),
    );

    rows
}

fn wrapped_line_count(text: &str, width: u16) -> u16 {
    if width == 0 {
        return 0;
    }

    text.lines().fold(0u16, |acc, line| {
        let chars = line.chars().count() as u16;
        let wrapped = (chars / width).saturating_add(1);
        acc.saturating_add(wrapped)
    })
}

fn panel_max_scroll(body: &str, panel_width: u16, panel_height: u16) -> u16 {
    let inner_width = panel_width.saturating_sub(2).max(1);
    let visible_lines = panel_height.saturating_sub(2);
    if visible_lines == 0 {
        return 0;
    }

    wrapped_line_count(body, inner_width).saturating_sub(visible_lines)
}

fn format_float(value: Option<f64>, precision: usize) -> String {
    match value {
        Some(v) => format!("{v:.precision$}"),
        None => "n/a".to_string(),
    }
}

fn ascii_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate().take(widths.len()) {
            widths[index] = widths[index].max(cell.len());
        }
    }

    let border = format!(
        "+{}+",
        widths
            .iter()
            .map(|width| "-".repeat(width + 2))
            .collect::<Vec<_>>()
            .join("+")
    );
    let header_row = format!(
        "| {} |",
        headers
            .iter()
            .enumerate()
            .map(|(index, value)| format!("{value:<width$}", width = widths[index]))
            .collect::<Vec<_>>()
            .join(" | ")
    );

    let mut lines = Vec::with_capacity(rows.len() + 4);
    lines.push(border.clone());
    lines.push(header_row);
    lines.push(border.clone());

    for row in rows {
        let row_text = format!(
            "| {} |",
            widths
                .iter()
                .enumerate()
                .map(|(index, width)| {
                    let value = row.get(index).map(String::as_str).unwrap_or("");
                    format!("{value:<width$}", width = *width)
                })
                .collect::<Vec<_>>()
                .join(" | ")
        );
        lines.push(row_text);
    }

    lines.push(border);
    lines.join("\n")
}

fn format_insider_sentiment_table(content: &str) -> Option<String> {
    let payload: Value = serde_json::from_str(content).ok()?;
    let symbol = payload
        .get("symbol")
        .and_then(Value::as_str)
        .unwrap_or("n/a")
        .to_string();
    let data = payload.get("data")?.as_array()?;

    let mut entries: Vec<(i64, i64, Option<f64>, Option<f64>)> = data
        .iter()
        .map(|row| {
            (
                row.get("year").and_then(Value::as_i64).unwrap_or(0),
                row.get("month").and_then(Value::as_i64).unwrap_or(0),
                row.get("mspr").and_then(Value::as_f64),
                row.get("change").and_then(Value::as_f64),
            )
        })
        .collect();

    if entries.is_empty() {
        return Some(format!(
            "Insider Sentiment ({symbol})\nNo records returned."
        ));
    }

    entries.sort_by(|left, right| (right.0, right.1).cmp(&(left.0, left.1)));
    let max_rows = 24;
    let rows: Vec<Vec<String>> = entries
        .iter()
        .take(max_rows)
        .map(|(year, month, mspr, change)| {
            vec![
                year.to_string(),
                format!("{month:02}"),
                format_float(*mspr, 3),
                format_float(*change, 3),
            ]
        })
        .collect();
    let table = ascii_table(&["Year", "Month", "MSPR", "Change"], &rows);

    Some(format!(
        "Insider Sentiment ({symbol})\n{table}\nShowing {} of {} records",
        rows.len(),
        entries.len()
    ))
}

fn format_finnhub_dataset_body(title: &str, content: &str) -> Option<String> {
    match title {
        "Insider Sentiment" => format_insider_sentiment_table(content),
        _ => None,
    }
}

fn downsample_candles(candles: &[Candle], max_points: usize) -> Vec<Candle> {
    if max_points == 0 || candles.is_empty() {
        return Vec::new();
    }
    if candles.len() <= max_points {
        return candles.to_vec();
    }

    let mut sampled = Vec::with_capacity(max_points);
    for bucket in 0..max_points {
        let start = bucket * candles.len() / max_points;
        let mut end = (bucket + 1) * candles.len() / max_points;
        if end <= start {
            end = (start + 1).min(candles.len());
        }
        let window = &candles[start..end];
        let first = window[0];
        let last = window[window.len() - 1];
        let high = window
            .iter()
            .map(|candle| candle.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = window
            .iter()
            .map(|candle| candle.low)
            .fold(f64::INFINITY, f64::min);

        sampled.push(Candle {
            timestamp: last.timestamp,
            open: first.open,
            high,
            low,
            close: last.close,
        });
    }

    sampled
}

fn historical_flow_points(candles: &[Candle], max_points: usize) -> Vec<(f64, f64)> {
    let sampled = downsample_candles(candles, max_points);
    let Some(first_close) = sampled.first().map(|candle| candle.close) else {
        return Vec::new();
    };

    if first_close.abs() <= f64::EPSILON {
        return sampled
            .into_iter()
            .enumerate()
            .map(|(index, candle)| (index as f64, candle.close))
            .collect();
    }

    sampled
        .into_iter()
        .enumerate()
        .map(|(index, candle)| {
            (
                index as f64,
                ((candle.close - first_close) / first_close) * 100.0,
            )
        })
        .collect()
}

fn live_price_points(points: &[YahooLivePoint]) -> Vec<(f64, f64)> {
    let Some(latest_ts) = points.last().map(|point| point.timestamp_ms as f64) else {
        return Vec::new();
    };

    points
        .iter()
        .map(|point| {
            (
                ((point.timestamp_ms as f64 - latest_ts) / 1_000.0).max(-10.0),
                point.price,
            )
        })
        .collect()
}

fn padded_bounds(values: &[f64]) -> Option<(f64, f64)> {
    if values.is_empty() {
        return None;
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !min.is_finite() || !max.is_finite() {
        return None;
    }

    if (max - min).abs() <= f64::EPSILON {
        let pad = if min.abs() <= 1.0 {
            0.5
        } else {
            min.abs() * 0.01
        };
        return Some((min - pad, max + pad));
    }

    let pad = (max - min) * 0.05;
    Some((min - pad, max + pad))
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

fn render_yahoo_tab(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let yahoo_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    if let Some(meta) = &app.quote {
        let quote = Table::new(
            quote_rows(meta),
            [Constraint::Length(18), Constraint::Min(20)],
        )
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Yahoo — {}", display_name(meta))),
        );
        frame.render_widget(quote, yahoo_chunks[0]);
    } else {
        let placeholder =
            Paragraph::new("No Yahoo quote loaded yet. Enter a ticker and press Enter.")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title("Yahoo"))
                .wrap(Wrap { trim: true });
        frame.render_widget(placeholder, yahoo_chunks[0]);
    }

    let chart_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(12)])
        .split(yahoo_chunks[1]);
    let plot_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chart_chunks[1]);

    let range_tabs = Tabs::new(
        CandleRange::ORDERED
            .iter()
            .map(|range| range.tab_label())
            .collect::<Vec<_>>(),
    )
    .select(app.yahoo_range_index())
    .highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Candles [ ] / Ctrl+D/W/M/Y/A"),
    );
    frame.render_widget(range_tabs, chart_chunks[0]);

    let live_points = live_price_points(&app.yahoo_live_prices);
    if live_points.len() < 2 {
        let live_placeholder = Paragraph::new(
            "Waiting for live Yahoo ticks... (updates every ~2 seconds, rolling 10s window)",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Live 10s Stream"),
        )
        .wrap(Wrap { trim: true });
        frame.render_widget(live_placeholder, plot_chunks[0]);
    } else {
        let live_prices: Vec<f64> = live_points.iter().map(|(_, y)| *y).collect();
        let (live_min, live_max) = padded_bounds(&live_prices).unwrap_or((0.0, 1.0));
        let live_first = live_points.first().map(|(_, y)| *y).unwrap_or(live_min);
        let live_last = live_points.last().map(|(_, y)| *y).unwrap_or(live_max);
        let live_style = if live_last >= live_first {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };

        let live_dataset = Dataset::default()
            .name("Live Price")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(live_style)
            .data(&live_points);

        let live_chart = Chart::new(vec![live_dataset])
            .x_axis(
                Axis::default()
                    .title("time")
                    .bounds([-10.0, 0.0])
                    .labels(vec![Span::raw("-10s"), Span::raw("-5s"), Span::raw("now")]),
            )
            .y_axis(
                Axis::default()
                    .title("price")
                    .bounds([live_min, live_max])
                    .labels(vec![
                        Span::raw(format!("{live_min:.2}")),
                        Span::raw(format!("{live_max:.2}")),
                    ]),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Live 10s Stream"),
            );
        frame.render_widget(live_chart, plot_chunks[0]);
    }

    let history_points = historical_flow_points(
        &app.yahoo_candles,
        plot_chunks[1].width.saturating_mul(4).max(20) as usize,
    );
    if history_points.len() < 2 {
        let history_placeholder =
            Paragraph::new("No chart history loaded yet. Fetch a ticker to load Yahoo history.")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Historical Flow"),
                )
                .wrap(Wrap { trim: true });
        frame.render_widget(history_placeholder, plot_chunks[1]);
    } else {
        let flow_values: Vec<f64> = history_points.iter().map(|(_, y)| *y).collect();
        let (history_min, history_max) = padded_bounds(&flow_values).unwrap_or((-1.0, 1.0));
        let history_first = history_points.first().map(|(_, y)| *y).unwrap_or(0.0);
        let history_last = history_points
            .last()
            .map(|(_, y)| *y)
            .unwrap_or(history_max);
        let history_style = if history_last >= history_first {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };

        let history_dataset = Dataset::default()
            .name("Flow %")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(history_style)
            .data(&history_points);

        let history_title = format!(
            "{} flow ({})  now {:+.2}%",
            app.active_ticker.as_deref().unwrap_or("n/a"),
            app.yahoo_range.title_label(),
            history_last
        );
        let history_chart = Chart::new(vec![history_dataset])
            .x_axis(
                Axis::default()
                    .title("samples")
                    .bounds([0.0, (history_points.len() - 1) as f64])
                    .labels(vec![Span::raw("start"), Span::raw("mid"), Span::raw("now")]),
            )
            .y_axis(
                Axis::default()
                    .title("% vs first")
                    .bounds([history_min, history_max])
                    .labels(vec![
                        Span::raw(format!("{history_min:+.2}%")),
                        Span::raw(format!("{history_max:+.2}%")),
                    ]),
            )
            .block(Block::default().borders(Borders::ALL).title(history_title));
        frame.render_widget(history_chart, plot_chunks[1]);
    };
}

#[allow(dead_code)]
fn render_mlx_tab(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let section_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    let section_titles: Vec<String> = app
        .mlx_sections
        .iter()
        .enumerate()
        .map(|(index, section)| format!("{} {}", index + 1, section.title))
        .collect();
    let section_hotkey_hint = if app.mlx_sections.is_empty() {
        "1-0".to_string()
    } else {
        format!("1-{}", app.mlx_sections.len().min(9))
    };
    let section_tabs = Tabs::new(section_titles)
        .select(app.active_mlx_section_index)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("MLX Sections [ and ] / {section_hotkey_hint}")),
        );
    frame.render_widget(section_tabs, section_chunks[0]);

    let panel_title = match &app.comparison_ticker {
        Some(comp) => format!("Worker Output (vs {comp})"),
        None => "Worker Output".to_string(),
    };

    let Some(section) = app.mlx_sections.get(app.active_mlx_section_index) else {
        let empty = Paragraph::new("No worker sections configured.")
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title(panel_title))
            .wrap(Wrap { trim: true });
        frame.render_widget(empty, section_chunks[1]);
        return;
    };

    let body = if let Some(content) = &section.content {
        content.clone()
    } else if let Some(error) = &app.analysis_error {
        format!("Analysis error: {error}")
    } else if app.analysis_loading {
        match &app.mlx_status {
            Some(status) => format!("Running worker: {}\n\n{status}", section.title),
            None => format!("Running worker: {}\n\nAwaiting output...", section.title),
        }
    } else {
        format!(
            "No output yet for {}. Fetch a ticker to run worker analysis.",
            section.title
        )
    };

    let max_scroll = panel_max_scroll(&body, section_chunks[1].width, section_chunks[1].height);
    let scroll = section.scroll.min(max_scroll);
    let title = format!("{panel_title} (scroll {scroll}/{max_scroll})");
    let panel = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((scroll, 0))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, section_chunks[1]);
}

fn render_finnhub_tab(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let dataset_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    let dataset_titles: Vec<String> = app
        .finnhub_datasets
        .iter()
        .enumerate()
        .map(|(index, dataset)| format!("{} {}", index + 1, dataset.title))
        .collect();
    let dataset_hotkey_hint = if app.finnhub_datasets.is_empty() {
        "1-0".to_string()
    } else {
        format!("1-{}", app.finnhub_datasets.len().min(9))
    };
    let dataset_tabs = Tabs::new(dataset_titles)
        .select(app.active_finnhub_dataset_index)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Finnhub Datasets [ and ] / {dataset_hotkey_hint}")),
        );
    frame.render_widget(dataset_tabs, dataset_chunks[0]);

    let Some(dataset) = app.finnhub_datasets.get(app.active_finnhub_dataset_index) else {
        let empty = Paragraph::new("No Finnhub datasets configured.")
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::ALL).title("Finnhub"))
            .wrap(Wrap { trim: true });
        frame.render_widget(empty, dataset_chunks[1]);
        return;
    };

    let raw_body = if let Some(content) = &dataset.content {
        content.clone()
    } else {
        app.finnhub_status.clone().unwrap_or_else(|| {
            "No Finnhub data loaded yet. Set ROMINALS_FINNHUB_API_KEY and fetch a ticker."
                .to_string()
        })
    };
    let body = if dataset.is_error {
        raw_body
    } else if let Some(formatted) = format_finnhub_dataset_body(&dataset.title, &raw_body) {
        formatted
    } else {
        raw_body
    };

    let max_scroll = panel_max_scroll(&body, dataset_chunks[1].width, dataset_chunks[1].height);
    let scroll = dataset.scroll.min(max_scroll);
    let title = format!("{} (scroll {scroll}/{max_scroll})", dataset.title);
    let panel_style = if dataset.is_error {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    let panel = Paragraph::new(body)
        .style(panel_style)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(panel, dataset_chunks[1]);
}

fn render_market_view(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let hub_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    render_yahoo_tab(frame, app, hub_chunks[0]);
    render_finnhub_tab(frame, app, hub_chunks[1]);
}

fn render_report_panel(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let status_text = app
        .report_status
        .as_deref()
        .unwrap_or("Idle. Press Ctrl+E to generate an AI report and send it to Gmail.");
    let recipient = app
        .report_last_sent_to
        .clone()
        .or_else(|| std::env::var("ROMINALS_REPORT_TO").ok())
        .or_else(|| std::env::var("ROMINALS_GMAIL_USER").ok())
        .unwrap_or_else(|| "n/a".to_string());
    let subject = app.report_last_subject.as_deref().unwrap_or("n/a");
    let sent_at = app.report_last_sent_at.as_deref().unwrap_or("n/a");
    let preview = app
        .report_preview
        .as_deref()
        .map(|text| truncate_chars(text, 3_000))
        .unwrap_or_else(|| "No preview generated yet.".to_string());

    let mut lines = vec![
        Line::from("Action: Ctrl+E to summarize and email report"),
        Line::from(format!("Recipient: {recipient}")),
        Line::from(format!("Last subject: {subject}")),
        Line::from(format!("Last sent (UTC): {sent_at}")),
        Line::from(""),
        Line::from(Span::styled(
            "Delivery status:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(status_text),
        Line::from(""),
        Line::from(Span::styled(
            "Latest preview:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    lines.extend(preview.lines().map(Line::from));

    let panel_style = if app.report_loading {
        Style::default().fg(Color::Yellow)
    } else if status_text.contains("failed") || status_text.contains("error") {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let panel = Paragraph::new(lines)
        .style(panel_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Report Mailer"),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(panel, area);
}

pub(super) fn draw_ui(frame: &mut Frame, app: &App) {
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(4),
        ])
        .split(frame.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Rominals TUI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(
            "  |  Enter fetch  Ctrl+R refresh  Ctrl+E email AI report  Yahoo auto-stream ~2s  Tab/Shift+Tab/←/→/[ ] cycle Finnhub dataset  1-9 jump dataset  Ctrl+D/W/M/Y/A Yahoo range  Esc quit",
        ),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Header"));
    frame.render_widget(header, outer_chunks[0]);

    let input = Paragraph::new(vec![
        Line::from(vec![
            Span::raw("Ticker input: "),
            Span::styled(
                app.input.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if app.input_cursor_visible { "│" } else { " " },
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(format!(
            "Current symbol: {}",
            app.active_ticker.as_deref().unwrap_or("n/a")
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("Input"))
    .wrap(Wrap { trim: true });
    frame.render_widget(input, outer_chunks[1]);

    let center_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(outer_chunks[2]);
    render_market_view(frame, app, center_chunks[0]);
    render_report_panel(frame, app, center_chunks[1]);

    let status = if app.analysis_loading {
        Paragraph::new("Refreshing market data...")
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Status / Health"),
            )
    } else {
        match &app.error {
            Some(error) => Paragraph::new(Line::from(vec![
                Span::styled(
                    "Error: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(error),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Status / Health"),
            ),
            None => {
                let yahoo_health = if app.quote.is_some() {
                    "ready"
                } else {
                    "waiting"
                };
                let finnhub_ready = app
                    .finnhub_datasets
                    .iter()
                    .any(|dataset| dataset.content.is_some() && !dataset.is_error);
                let finnhub_health = if finnhub_ready { "ready" } else { "waiting" };
                let finnhub_status = app
                    .finnhub_status
                    .as_deref()
                    .unwrap_or("Finnhub datasets idle.");

                let status_lines = vec![
                    Line::from(format!(
                        "Sources  Yahoo: {yahoo_health}  |  Finnhub: {finnhub_health}"
                    )),
                    Line::from(finnhub_status),
                    Line::from(format!(
                        "Report mailer: {}",
                        app.report_status.as_deref().unwrap_or("idle")
                    )),
                ];
                let style = if finnhub_status.contains("unavailable") {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                Paragraph::new(status_lines).style(style).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Status / Health"),
                )
            }
        }
    };
    frame.render_widget(status, outer_chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> Meta {
        Meta {
            symbol: "AAPL".to_string(),
            long_name: Some("Apple Inc.".to_string()),
            short_name: Some("Apple".to_string()),
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
    fn change_text_formats_change_and_percent() {
        assert_eq!(change_text(&sample_meta()), "+5.00 (+5.26%)");
    }

    #[test]
    fn change_text_hides_change_when_previous_close_is_zero() {
        let mut meta = sample_meta();
        meta.chart_previous_close = Some(0.0);
        assert_eq!(change_text(&meta), "n/a");
    }

    #[test]
    fn display_name_prefers_long_name_then_short_name_then_symbol() {
        let meta = sample_meta();
        assert_eq!(display_name(&meta), "Apple Inc.");

        let mut no_long = sample_meta();
        no_long.long_name = None;
        assert_eq!(display_name(&no_long), "Apple");

        let mut symbol_only = sample_meta();
        symbol_only.long_name = None;
        symbol_only.short_name = None;
        assert_eq!(display_name(&symbol_only), "AAPL");
    }

    #[test]
    fn insider_sentiment_renders_latest_first() {
        let json = r#"{
            "symbol": "AAPL",
            "data": [
                {"year": 2024, "month": 2, "mspr": 1.2, "change": 0.1},
                {"year": 2024, "month": 3, "mspr": 1.1, "change": -0.2}
            ]
        }"#;

        let table = format_insider_sentiment_table(json).unwrap();
        let march_index = table.find("2024 | 03").unwrap();
        let feb_index = table.find("2024 | 02").unwrap();
        assert!(march_index < feb_index);
    }

    #[test]
    fn historical_flow_points_are_percent_change_series() {
        let candles = vec![
            Candle {
                timestamp: 1,
                open: 10.0,
                high: 11.0,
                low: 9.0,
                close: 10.5,
            },
            Candle {
                timestamp: 2,
                open: 10.5,
                high: 11.2,
                low: 9.7,
                close: 10.0,
            },
        ];

        let points = historical_flow_points(&candles, 8);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0], (0.0, 0.0));
        assert!((points[1].1 + 4.7619).abs() < 0.001);
    }

    #[test]
    fn live_price_points_anchor_latest_as_now() {
        let points = vec![
            YahooLivePoint {
                timestamp_ms: 10_000,
                price: 99.5,
            },
            YahooLivePoint {
                timestamp_ms: 12_000,
                price: 100.0,
            },
            YahooLivePoint {
                timestamp_ms: 15_000,
                price: 100.2,
            },
        ];

        let live = live_price_points(&points);
        assert_eq!(live.len(), 3);
        assert_eq!(live.last().unwrap().0, 0.0);
        assert!(live.first().unwrap().0 <= -5.0);
    }

    #[test]
    fn padded_bounds_adds_padding_for_flat_series() {
        let bounds = padded_bounds(&[100.0, 100.0, 100.0]).unwrap();
        assert!(bounds.0 < 100.0);
        assert!(bounds.1 > 100.0);
    }
}
