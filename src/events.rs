use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::App, commands};

/// Handle one key event. Returns `true` if the application should quit.
pub async fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Ctrl+C → quit.
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
        return Ok(true);
    }
    // Esc → quit.
    if key.code == KeyCode::Esc {
        return Ok(true);
    }

    // Scrolling in content area.
    match key.code {
        KeyCode::PageUp => {
            app.content_scroll = app.content_scroll.saturating_sub(10);
            return Ok(false);
        }
        KeyCode::PageDown => {
            app.content_scroll = app.content_scroll.saturating_add(10);
            return Ok(false);
        }
        _ => {}
    }

    // Prompt editing and history.
    match key.code {
        KeyCode::Enter => {
            let input = app.prompt_input.trim().to_string();
            if !input.is_empty() {
                // Save to history (avoid consecutive duplicates).
                if app.prompt_history.last().map(|s| s.as_str()) != Some(&input) {
                    app.prompt_history.push(input.clone());
                }
                app.prompt_history_idx = None;
                app.prompt_input.clear();

                if let Err(e) = commands::execute(app, &input).await {
                    let msg = format!("Error: {e}");
                    app.push_event(format!("[ERR] {}", e));
                    app.push_output(msg.clone());
                    app.content_lines.push(msg);
                }
            }
        }

        KeyCode::Backspace => {
            app.prompt_input.pop();
            app.prompt_history_idx = None;
        }

        KeyCode::Up => scroll_history_up(app),

        KeyCode::Down => scroll_history_down(app),

        KeyCode::Char(c) => {
            // Auto-insert '/' for the first character if nothing typed yet.
            if app.prompt_input.is_empty() && c != '/' {
                app.prompt_input.push('/');
            }
            app.prompt_input.push(c);
            app.prompt_history_idx = None;
        }

        _ => {}
    }

    Ok(app.should_quit)
}

fn scroll_history_up(app: &mut App) {
    if app.prompt_history.is_empty() {
        return;
    }
    let new_idx = match app.prompt_history_idx {
        None => app.prompt_history.len() - 1,
        Some(i) => i.saturating_sub(1),
    };
    app.prompt_history_idx = Some(new_idx);
    app.prompt_input = app.prompt_history[new_idx].clone();
}

fn scroll_history_down(app: &mut App) {
    match app.prompt_history_idx {
        None => {}
        Some(i) => {
            if i + 1 < app.prompt_history.len() {
                let new_idx = i + 1;
                app.prompt_history_idx = Some(new_idx);
                app.prompt_input = app.prompt_history[new_idx].clone();
            } else {
                app.prompt_history_idx = None;
                app.prompt_input.clear();
            }
        }
    }
}
