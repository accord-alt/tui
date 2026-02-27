use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::time::{sleep, Duration};

mod app;
mod commands;
mod events;
mod ui;

use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    // Auto-start the node on launch as required by the plan.
    if let Err(e) = commands::execute(&mut app, "/startNode").await {
        app.push_event(format!("[NODE] Auto-start failed: {e}"));
    }

    let result = run(&mut terminal, &mut app).await;

    // Always restore the terminal, even on error.
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;

    result
}

async fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut reader = EventStream::new();

    loop {
        terminal.draw(|f| ui::render(f, app))?;

        let tick = sleep(Duration::from_millis(250));

        tokio::select! {
            _ = tick => {
                // Periodic refresh — re-draw even without input so the UI stays alive.
            }
            maybe_event = reader.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        let quit = events::handle_key(app, key).await?;
                        if quit || app.should_quit {
                            break;
                        }
                    }
                    Some(Ok(_)) => {} // mouse events, resize, etc.
                    Some(Err(e)) => return Err(e.into()),
                    None => break,
                }
            }
        }
    }

    Ok(())
}
