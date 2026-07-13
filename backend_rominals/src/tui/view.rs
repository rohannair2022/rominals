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

pub(super) fn draw_ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(8),
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
        Span::raw("  |  Enter fetch  r refresh  q quit"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Header"));
    frame.render_widget(header, chunks[0]);

    let input = Paragraph::new(vec![
        Line::from(format!("Ticker input: {}", app.input)),
        Line::from(format!(
            "Current symbol: {}",
            app.active_ticker.as_deref().unwrap_or("n/a")
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("Input"))
    .wrap(Wrap { trim: true });
    frame.render_widget(input, chunks[1]);

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
        frame.render_widget(quote, chunks[2]);
    } else {
        let placeholder = Paragraph::new("No quote loaded yet. Enter a ticker and press Enter.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Quote"))
            .wrap(Wrap { trim: true });
        frame.render_widget(placeholder, chunks[2]);
    }

    let status = match &app.error {
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
    };
    frame.render_widget(status, chunks[3]);
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
