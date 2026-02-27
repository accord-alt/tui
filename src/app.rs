use accord_network::{Connection, FullNodeCommand, User};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeStatus {
    Stopped,
    Running { addr: String },
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Stopped => write!(f, "Stopped"),
            NodeStatus::Running { addr } => write!(f, "Running  ({})", addr),
        }
    }
}

pub struct App {
    pub content_scroll: u16,
    /// Lines currently displayed in the content area.
    pub content_lines: Vec<String>,
    /// Title shown on the content block border.
    pub content_title: String,

    pub prompt_input: String,
    pub prompt_history: Vec<String>,
    /// Index into prompt_history while scrolling; None = live input.
    pub prompt_history_idx: Option<usize>,

    pub node_tx: Option<mpsc::Sender<FullNodeCommand>>,
    pub node_status: NodeStatus,
    /// TCP port the node listens on (default 51030).
    pub listen_port: u16,

    pub peers: Vec<String>,
    pub users: Vec<User>,
    pub connections: Vec<Connection>,
    pub messages: Vec<String>,

    /// All node events in chronological order (shown by /events).
    pub events: Vec<String>,
    /// Command output log (shown by /console).
    pub output: Vec<String>,

    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let welcome = vec![
            "Welcome to Accord!".to_string(),
            "Starting the P2P node…".to_string(),
            "Type /help to see all available commands.".to_string(),
        ];
        Self {
            content_scroll: 0,
            content_lines: welcome.clone(),
            content_title: " Accord ".to_string(),
            prompt_input: String::new(),
            prompt_history: Vec::new(),
            prompt_history_idx: None,
            node_tx: None,
            node_status: NodeStatus::Stopped,
            listen_port: 51030,
            peers: Vec::new(),
            users: Vec::new(),
            connections: Vec::new(),
            messages: Vec::new(),
            events: welcome,
            output: Vec::new(),
            should_quit: false,
        }
    }

    /// Replace the content area with new lines and a title.
    pub fn set_content(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.content_title = format!(" {} ", title.into());
        self.content_lines = lines;
        self.content_scroll = 0;
    }

    /// Append a line to the events log.
    pub fn push_event(&mut self, line: impl Into<String>) {
        self.events.push(line.into());
    }

    /// Append a line to the console output log.
    pub fn push_output(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

}
