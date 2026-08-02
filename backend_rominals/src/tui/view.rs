use super::state::{App, AppTab};
use crate::api::yahoo::Meta;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap};
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

fn format_market_sentiment_table(content: &str) -> Option<String> {
    let payload: Value = serde_json::from_str(content).ok()?;
    let symbol = payload
        .get("symbol")
        .and_then(Value::as_str)
        .unwrap_or("n/a")
        .to_string();
    let sentiment = payload.get("sentiment");
    let buzz = payload.get("buzz");

    let rows = vec![
        vec![
            "Company News Score".to_string(),
            format_float(payload.get("companyNewsScore").and_then(Value::as_f64), 3),
        ],
        vec![
            "Sector Avg News Score".to_string(),
            format_float(
                payload
                    .get("sectorAverageNewsScore")
                    .and_then(Value::as_f64),
                3,
            ),
        ],
        vec![
            "Bullish Percent".to_string(),
            format!(
                "{}%",
                format_float(
                    sentiment
                        .and_then(|value| value.get("bullishPercent"))
                        .and_then(Value::as_f64),
                    1
                )
            ),
        ],
        vec![
            "Bearish Percent".to_string(),
            format!(
                "{}%",
                format_float(
                    sentiment
                        .and_then(|value| value.get("bearishPercent"))
                        .and_then(Value::as_f64),
                    1
                )
            ),
        ],
        vec![
            "Sector Avg Bullish".to_string(),
            format!(
                "{}%",
                format_float(
                    payload
                        .get("sectorAverageBullishPercent")
                        .and_then(Value::as_f64),
                    1
                )
            ),
        ],
        vec![
            "Articles Last Week".to_string(),
            buzz.and_then(|value| value.get("articlesInLastWeek"))
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
        ],
        vec![
            "Buzz This Week".to_string(),
            format_float(
                buzz.and_then(|value| value.get("buzz"))
                    .and_then(Value::as_f64),
                3,
            ),
        ],
        vec![
            "Weekly Avg Buzz".to_string(),
            format_float(
                buzz.and_then(|value| value.get("weeklyAverage"))
                    .and_then(Value::as_f64),
                3,
            ),
        ],
    ];

    let table = ascii_table(&["Metric", "Value"], &rows);
    Some(format!("Market Sentiment ({symbol})\n{table}"))
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
        "Market Sentiment" => format_market_sentiment_table(content),
        "Insider Sentiment" => format_insider_sentiment_table(content),
        _ => None,
    }
}

fn render_yahoo_tab(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
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
        frame.render_widget(quote, area);
    } else {
        let placeholder =
            Paragraph::new("No Yahoo quote loaded yet. Enter a ticker and press Enter.")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title("Yahoo"))
                .wrap(Wrap { trim: true });
        frame.render_widget(placeholder, area);
    }
}

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

pub(super) fn draw_ui(frame: &mut Frame, app: &App) {
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
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
            "  |  Enter fetch  Ctrl+R refresh  Tab/←/→/F1-F3 tabs  [ ] or 1-9 sections  Esc quit",
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

    let tabs = Tabs::new(vec!["Yahoo", "MLX", "Finnhub"])
        .select(app.active_tab_index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title("Tabs"));
    frame.render_widget(tabs, outer_chunks[2]);

    match app.active_tab {
        AppTab::Yahoo => render_yahoo_tab(frame, app, outer_chunks[3]),
        AppTab::Mlx => render_mlx_tab(frame, app, outer_chunks[3]),
        AppTab::Finnhub => render_finnhub_tab(frame, app, outer_chunks[3]),
    }

    let status = if app.analysis_loading {
        Paragraph::new("Fetching quote + Finnhub datasets + running MLX workers...")
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Status"))
    } else {
        match &app.error {
            Some(error) => Paragraph::new(Line::from(vec![
                Span::styled(
                    "Error: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(error),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Status")),
            None => {
                let active_status = match app.active_tab {
                    AppTab::Finnhub => app.finnhub_status.as_deref(),
                    AppTab::Yahoo | AppTab::Mlx => app.mlx_status.as_deref(),
                };
                let text = active_status.unwrap_or("Ready.");
                let style = if active_status.is_some() {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Green)
                };
                Paragraph::new(text)
                    .style(style)
                    .block(Block::default().borders(Borders::ALL).title("Status"))
            }
        }
    };
    frame.render_widget(status, outer_chunks[4]);
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
    fn market_sentiment_renders_table() {
        let json = r#"{
            "symbol": "AAPL",
            "companyNewsScore": 0.42,
            "sectorAverageNewsScore": 0.38,
            "sectorAverageBullishPercent": 61.5,
            "sentiment": {
                "bullishPercent": 66.2,
                "bearishPercent": 33.8
            },
            "buzz": {
                "articlesInLastWeek": 123,
                "buzz": 1.4,
                "weeklyAverage": 1.1
            }
        }"#;

        let table = format_market_sentiment_table(json).unwrap();
        assert!(table.contains("Market Sentiment (AAPL)"));
        assert!(table.contains("Company News Score"));
        assert!(table.contains("Bullish Percent"));
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
}
