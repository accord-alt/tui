use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(0),    // content
            Constraint::Length(3), // prompt
        ])
        .split(f.area());

    render_header(f, chunks[0], app);
    render_content(f, chunks[1], app);
    render_prompt(f, chunks[2], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let status = match &app.node_status {
        crate::app::NodeStatus::Stopped => "●  Stopped".to_string(),
        crate::app::NodeStatus::Running { .. } => {
            format!("●  Running  (port {})", app.listen_port)
        }
    };

    let title = Paragraph::new(format!(" Accord  v{}   │   {}", VERSION, status))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn render_content(f: &mut Frame, area: Rect, app: &App) {
    let lines = &app.content_lines;
    let visible_height = area.height.saturating_sub(2) as usize;
    let total = lines.len();

    let scroll_offset = if total <= visible_height {
        0
    } else {
        let max_scroll = total - visible_height;
        (app.content_scroll as usize).min(max_scroll)
    };

    let visible: Vec<ListItem> = lines
        .iter()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|l| ListItem::new(l.as_str()))
        .collect();

    let title = if total > visible_height {
        let pct = (scroll_offset * 100) / total.max(1);
        format!("{}({}%  PgUp/PgDn) ", app.content_title, pct)
    } else {
        app.content_title.clone()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let list = List::new(visible).block(block);
    f.render_widget(list, area);
}

fn render_prompt(f: &mut Frame, area: Rect, app: &App) {
    let display = format!("> {}", app.prompt_input);
    let prompt = Paragraph::new(display)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Prompt  (Enter=run  ↑↓=history  Esc=quit) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(prompt, area);

    // Position the cursor after the "> " prefix.
    let cursor_x = area.x + 2 + app.prompt_input.len() as u16 + 1;
    let cursor_y = area.y + 1;
    if cursor_x < area.x + area.width - 1 {
        f.set_cursor_position((cursor_x, cursor_y));
    }
}
