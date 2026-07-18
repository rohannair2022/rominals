use super::state::App;
use crate::api::yahoo::Meta;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};

/// Format an Option<f64> as a price string, or "n/a" if absent.
fn money(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.2}", x),
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

fn analysis_max_scroll(analysis: &str, panel_width: u16, panel_height: u16) -> u16 {
    let inner_width = panel_width.saturating_sub(2).max(1);
    let visible_lines = panel_height.saturating_sub(2);
    if visible_lines == 0 {
        return 0;
    }

    wrapped_line_count(analysis, inner_width).saturating_sub(visible_lines)
}

pub(super) fn draw_ui(frame: &mut Frame, app: &App) {
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(frame.area());
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(outer_chunks[2]);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Rominals TUI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  Enter fetch  Ctrl+R refresh  ↑/↓ scroll analysis  Esc quit"),
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

    if let Some(meta) = &app.quote {
        let quote = Table::new(
            quote_rows(meta),
            [Constraint::Length(18), Constraint::Min(20)],
        )
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Quote — {}", display_name(meta))),
        );
        frame.render_widget(quote, content_chunks[0]);
    } else {
        let placeholder = Paragraph::new("No quote loaded yet. Enter a ticker and press Enter.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Quote"))
            .wrap(Wrap { trim: true });
        frame.render_widget(placeholder, content_chunks[0]);
    }

    let analysis_title = match &app.comparison_ticker {
        Some(comp) => format!("Ollama Analysis (vs {comp})"),
        None => "Ollama Analysis".to_string(),
    };

    if app.analysis_loading {
        let loading_panel = Paragraph::new("Running Ollama analysis...")
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title(analysis_title))
            .wrap(Wrap { trim: true });
        frame.render_widget(loading_panel, content_chunks[1]);
    } else if let Some(analysis) = &app.analysis {
        let max_scroll =
            analysis_max_scroll(analysis, content_chunks[1].width, content_chunks[1].height);
        let scroll = app.analysis_scroll.min(max_scroll);
        let title = format!("{analysis_title} (scroll {scroll}/{max_scroll})");
        let analysis_panel = Paragraph::new(analysis.clone())
            .block(Block::default().borders(Borders::ALL).title(title))
            .scroll((scroll, 0))
            .wrap(Wrap { trim: true });
        frame.render_widget(analysis_panel, content_chunks[1]);
    } else if let Some(analysis_error) = &app.analysis_error {
        let analysis_panel = Paragraph::new(Line::from(vec![
            Span::styled(
                "Analysis error: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(analysis_error),
        ]))
        .block(Block::default().borders(Borders::ALL).title(analysis_title))
        .wrap(Wrap { trim: true });
        frame.render_widget(analysis_panel, content_chunks[1]);
    } else {
        let analysis_placeholder =
            Paragraph::new("No analysis loaded yet. Fetch a ticker to generate one.")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL).title(analysis_title))
                .wrap(Wrap { trim: true });
        frame.render_widget(analysis_placeholder, content_chunks[1]);
    }

    let status = if app.analysis_loading {
        Paragraph::new("Fetching quote + analysis...")
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
            None => Paragraph::new("Ready.")
                .style(Style::default().fg(Color::Green))
                .block(Block::default().borders(Borders::ALL).title("Status")),
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
}
