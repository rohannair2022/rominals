pub(crate) mod controls;
mod state;
mod view;

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
use std::time::Duration;
use view::draw_ui;

fn fetch_and_store_quote(app: &mut App, ticker: &str) {
    match fetch_quote(ticker) {
        Ok(meta) => {
            app.active_ticker = Some(ticker.to_string());
            app.quote = Some(meta);
            app.error = None;
        }
        Err(err) => {
            app.error = Some(err.to_string());
        }
    }
}

pub(crate) fn run_tui(initial_ticker: Option<String>) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let run_result = (|| -> Result<(), Box<dyn Error>> {
        let mut app = App::default();

        if let Some(ticker) = initial_ticker {
            fetch_and_store_quote(&mut app, &ticker);
        }

        loop {
            terminal.draw(|frame| draw_ui(frame, &app))?;

            if !event::poll(Duration::from_millis(250))? {
                continue;
            }

            let event = event::read()?;
            if !controls::handle_event(&mut app, event, fetch_and_store_quote) {
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
