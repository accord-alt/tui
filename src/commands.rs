use anyhow::{anyhow, Result};
use multiaddr::Multiaddr;
use accord_network::{
    storage::fs::{
        list_connections, list_known_users, load_connection, load_known_user, load_local_user,
        load_peers, save_local_user,
    },
    Connection, FullNode, FullNodeCommand, User, UserMeta,
};
use tokio::sync::oneshot;

use crate::app::{App, NodeStatus};

fn listen_addr(port: u16) -> String {
    format!("/ip4/0.0.0.0/tcp/{}", port)
}

pub async fn execute(app: &mut App, raw: &str) -> Result<()> {
    let input = raw.trim();
    if input.is_empty() {
        return Ok(());
    }

    let (cmd, rest) = split_command(input);

    match cmd {
        "/help" => cmd_help(app),
        "/quit" => cmd_quit(app),
        "/events" => cmd_events(app),
        "/console" => cmd_console(app),
        "/messages" => cmd_messages(app),
        "/startNode" => cmd_start_node(app).await?,
        "/stopNode" => cmd_stop_node(app).await?,
        "/restartNode" => cmd_restart_node(app).await?,
        "/port" => cmd_port(app, rest).await?,
        "/sync" => cmd_sync(app),
        "/peers" => cmd_peers(app)?,
        "/nick" => cmd_nick(app, rest)?,
        "/user" => cmd_user(app, rest).await?,
        "/users" => cmd_users(app).await?,
        "/connection" => cmd_connection(app, rest).await?,
        "/connections" => cmd_connections(app)?,
        "/connectionsPending" => cmd_connections_pending(app)?,
        "/acceptConnection" => cmd_accept_connection(app, rest).await?,
        "/declineConnection" => cmd_decline_connection(app, rest),
        "/message" => cmd_message(app, rest).await?,
        "/messagePlugin" => cmd_message_plugin(app, rest).await?,
        _ => {
            let msg = format!("Unknown command: {}. Type /help for a list.", cmd);
            app.push_event(format!("[CMD] Unknown: {}", cmd));
            show_lines(app, "Error", vec![msg]);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Help
// ---------------------------------------------------------------------------

fn cmd_help(app: &mut App) {
    let lines: Vec<String> = [
        "Available commands:",
        "  /startNode                                   Start the P2P node",
        "  /stopNode                                    Stop the P2P node",
        "  /restartNode                                 Restart the P2P node",
        "  /port <port>                                 Change listen port and restart node",
        "  /sync                                        Note: sync is automatic",
        "  /peers                                       Show all known peers in content",
        "  /user                                        Show local user (or create one) in content",
        "  /nick <new_name>                             Change your display name",
        "  /users                                       Show all known users in content",
        "  /user <nick>                                 Show a user by display name in content",
        "  /connection <nick>                           Initiate a connection with a user",
        "  /connections                                 View all connections in content",
        "  /connectionsPending                          View pending connections in content",
        "  /acceptConnection <from_id> <their_pubkey>   Accept an incoming connection",
        "  /declineConnection <connection_id>           Decline a connection",
        "  /message <nick> <body>                       Send a text message",
        "  /messagePlugin <nick> <type> <body>          Send a plugin message",
        "  /messages                                    Show all messages in content",
        "  /events                                      Show all node events in content",
        "  /console                                     Show all output in content",
        "  /help                                        Show all commands in content",
        "  /quit                                        Quit the TUI",
        "",
        "Navigation:  PgUp/PgDn scroll content  |  ↑↓ prompt history  |  Esc quit",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    app.push_event("[CMD] /help");
    app.set_content("Help", lines);
}

// ---------------------------------------------------------------------------
// Quit
// ---------------------------------------------------------------------------

fn cmd_quit(app: &mut App) {
    app.push_event("[APP] Quit requested.");
    app.should_quit = true;
}

// ---------------------------------------------------------------------------
// Events / Console views
// ---------------------------------------------------------------------------

fn cmd_events(app: &mut App) {
    let lines = app.events.clone();
    app.push_event("[CMD] /events — showing events.");
    let lines_with_fresh = {
        let mut v = app.events.clone();
        v.push("[CMD] /events — showing events.".to_string());
        v
    };
    app.set_content("Events", lines_with_fresh);
    // auto-scroll to bottom
    app.content_scroll = lines.len() as u16;
}

fn cmd_console(app: &mut App) {
    app.push_output("[CMD] /console — showing output log.");
    let lines = app.output.clone();
    app.set_content("Console", lines);
    app.content_scroll = app.output.len() as u16;
}

fn cmd_messages(app: &mut App) {
    app.push_event("[CMD] /messages — showing messages.");
    let mut lines = vec![format!("Messages  ({})", app.messages.len()), String::new()];
    if app.messages.is_empty() {
        lines.push("  No messages yet. Use /message <nick> <body> to send one.".to_string());
    } else {
        lines.extend(app.messages.clone());
    }
    app.set_content("Messages", lines);
}

// ---------------------------------------------------------------------------
// Node lifecycle
// ---------------------------------------------------------------------------

async fn cmd_start_node(app: &mut App) -> Result<()> {
    if app.node_tx.is_some() {
        show_lines(app, "Node", vec!["Node is already running.".to_string()]);
        return Ok(());
    }

    let addr_str = listen_addr(app.listen_port);
    let msg = format!("Starting node on {} …", addr_str);
    app.push_event(format!("[NODE] {}", msg));
    app.push_output(msg.clone());

    let addr: Multiaddr = addr_str
        .parse()
        .map_err(|e: multiaddr::Error| anyhow!("Invalid listen address: {e}"))?;

    let node = FullNode::new(addr);
    match node.run().await {
        Ok(tx) => {
            app.node_tx = Some(tx);
            app.node_status = NodeStatus::Running { addr: addr_str.clone() };
            let ok = format!("Node started on {}.", addr_str);
            app.push_event(format!("[NODE] {}", ok));
            app.push_output(ok.clone());
            show_lines(app, "Node", vec![ok]);
        }
        Err(e) => {
            let err = format!("Failed to start node: {e}");
            app.push_event(format!("[NODE] Start failed: {e}"));
            app.push_output(err.clone());
            show_lines(app, "Node", vec![err]);
        }
    }

    Ok(())
}

async fn cmd_stop_node(app: &mut App) -> Result<()> {
    match app.node_tx.take() {
        Some(tx) => {
            let _ = tx.send(FullNodeCommand::Shutdown).await;
            app.node_status = NodeStatus::Stopped;
            app.push_event("[NODE] Stopped.");
            app.push_output("Node stopped.".to_string());
            show_lines(app, "Node", vec!["Node stopped.".to_string()]);
        }
        None => {
            show_lines(app, "Node", vec!["Node is not running.".to_string()]);
        }
    }
    Ok(())
}

async fn cmd_restart_node(app: &mut App) -> Result<()> {
    app.push_event("[NODE] Restarting…");
    cmd_stop_node(app).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    cmd_start_node(app).await?;
    Ok(())
}

async fn cmd_port(app: &mut App, rest: &str) -> Result<()> {
    let arg = rest.trim();
    if arg.is_empty() {
        show_lines(app, "Port", vec![format!(
            "Current port: {}  |  Usage: /port <port>",
            app.listen_port
        )]);
        return Ok(());
    }

    let new_port: u16 = arg
        .parse()
        .map_err(|_| anyhow!("'{}' is not a valid port number (1–65535).", arg))?;

    if new_port == 0 {
        show_lines(app, "Port", vec!["Port must be between 1 and 65535.".to_string()]);
        return Ok(());
    }

    let old_port = app.listen_port;
    app.listen_port = new_port;
    app.push_event(format!("[NODE] Port changed: {} → {}", old_port, new_port));
    app.push_output(format!("Port changed to {}. Restarting node…", new_port));
    cmd_restart_node(app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------

fn cmd_sync(app: &mut App) {
    if app.node_tx.is_none() {
        show_lines(app, "Sync", vec!["Node is not running. Use /startNode first.".to_string()]);
        return;
    }
    let msg = "Sync is continuous — the node syncs automatically with peers via gossipsub.";
    app.push_event("[SYNC] Manual sync requested.");
    app.push_output(msg.to_string());
    show_lines(app, "Sync", vec![msg.to_string()]);
}

// ---------------------------------------------------------------------------
// Peers
// ---------------------------------------------------------------------------

fn cmd_peers(app: &mut App) -> Result<()> {
    let peers = load_peers(None).unwrap_or_default();
    app.peers = peers.clone();
    app.push_event(format!("[PEERS] Refreshed ({} known).", peers.len()));
    app.push_output(format!("Peers: {} known.", peers.len()));

    let mut lines = vec![format!("Known peers  ({})", peers.len()), String::new()];
    if peers.is_empty() {
        lines.push("  No peers discovered yet. Start the node and wait for mDNS/Kademlia.".to_string());
    } else {
        for (i, p) in peers.iter().enumerate() {
            lines.push(format!("  {:>3}.  {}", i + 1, p));
        }
    }
    app.set_content("Peers", lines);
    Ok(())
}

// ---------------------------------------------------------------------------
// Nick
// ---------------------------------------------------------------------------

fn cmd_nick(app: &mut App, rest: &str) -> Result<()> {
    let new_name = rest.trim();
    if new_name.is_empty() {
        show_lines(app, "Nick", vec!["Usage: /nick <new_name>".to_string()]);
        return Ok(());
    }

    let mut user = match load_local_user(None) {
        Ok(u) => u,
        Err(_) => {
            show_lines(app, "Nick", vec!["No local user found. Use /user to create one first.".to_string()]);
            return Ok(());
        }
    };

    let old_name = user.meta.display_name.clone().unwrap_or_else(|| "(unnamed)".to_string());
    user.meta.display_name = Some(new_name.to_string());
    save_local_user(&user, None)?;

    if let Some(local) = app.users.iter_mut().find(|u| u.is_local()) {
        local.meta.display_name = Some(new_name.to_string());
    }

    let msg = format!("Display name changed: {} → {}", old_name, new_name);
    app.push_event(format!("[NICK] {} → {}", old_name, new_name));
    app.push_output(msg.clone());
    show_lines(app, "Nick", vec![msg]);
    Ok(())
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

async fn cmd_user(app: &mut App, rest: &str) -> Result<()> {
    let arg = rest.trim();

    // /user <nick>  → look up by display name
    if !arg.is_empty() {
        if let Some(id) = resolve_nick(arg) {
            return cmd_show_user_by_id(app, &id).await;
        }
        // Nick not found — treat as display name for a new user.
    }

    let tx = match &app.node_tx {
        Some(tx) => tx.clone(),
        None => {
            show_lines(app, "User", vec!["Node is not running. Use /startNode first.".to_string()]);
            return Ok(());
        }
    };

    // Show existing local user if no arg.
    if arg.is_empty() {
        match load_local_user(None) {
            Ok(user) => {
                let lines = user_lines(&user);
                app.set_content("User", lines);
                return Ok(());
            }
            Err(_) => {
                app.push_output("No local user found — creating one…".to_string());
            }
        }
    }

    // Create user.
    let meta = UserMeta {
        display_name: if arg.is_empty() { None } else { Some(arg.to_string()) },
        ..Default::default()
    };

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(FullNodeCommand::CreateUser { meta, reply: reply_tx })
        .await
        .map_err(|_| anyhow!("Node channel closed"))?;

    match reply_rx.await? {
        Ok(user) => {
            let name = user.meta.display_name.as_deref().unwrap_or("(unnamed)");
            app.push_event(format!("[USER] Created: {} ({})", name, truncate_id(&user.id, 16)));
            app.push_output(format!("User created: {}", name));
            let lines = user_lines(&user);
            if !app.users.iter().any(|u| u.id == user.id) {
                app.users.push(user);
            }
            app.set_content("User", lines);
        }
        Err(e) => {
            app.push_event(format!("[USER] Create failed: {e}"));
            show_lines(app, "User", vec![format!("Error creating user: {e}")]);
        }
    }

    Ok(())
}

async fn cmd_show_user_by_id(app: &mut App, id: &str) -> Result<()> {
    let tx = match &app.node_tx {
        Some(tx) => tx.clone(),
        None => {
            show_lines(app, "User", vec!["Node is not running. Use /startNode first.".to_string()]);
            return Ok(());
        }
    };

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(FullNodeCommand::GetUser { id: id.to_string(), reply: reply_tx })
        .await
        .map_err(|_| anyhow!("Node channel closed"))?;

    match reply_rx.await? {
        Ok(user) => {
            let lines = user_lines(&user);
            app.set_content("User", lines);
        }
        Err(e) => {
            show_lines(app, "User", vec![format!("User not found: {e}")]);
        }
    }

    Ok(())
}

async fn cmd_users(app: &mut App) -> Result<()> {
    let tx = match &app.node_tx {
        Some(tx) => tx.clone(),
        None => {
            // Fallback: read from filesystem.
            let ids = list_known_users(None).unwrap_or_default();
            let mut lines = vec![format!("Known users  ({})", ids.len()), String::new()];
            if ids.is_empty() {
                lines.push("  No remote users on record.".to_string());
            } else {
                for id in &ids {
                    let name = load_known_user(id, None)
                        .ok()
                        .and_then(|m| m.display_name)
                        .unwrap_or_else(|| "(unnamed)".to_string());
                    lines.push(format!("  {}  {}", name, id));
                }
            }
            app.set_content("Users", lines);
            return Ok(());
        }
    };

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(FullNodeCommand::GetUsers { reply: reply_tx })
        .await
        .map_err(|_| anyhow!("Node channel closed"))?;

    match reply_rx.await? {
        Ok(users) => {
            app.users = users.clone();
            app.push_event(format!("[USERS] Refreshed ({} found).", users.len()));
            app.push_output(format!("Users: {} found.", users.len()));
            let mut lines = vec![format!("Known users  ({})", users.len()), String::new()];
            if users.is_empty() {
                lines.push("  No remote users discovered yet.".to_string());
            } else {
                for u in &users {
                    let label = if u.is_local() { "LOCAL " } else { "REMOTE" };
                    let name = u.meta.display_name.as_deref().unwrap_or("(unnamed)");
                    lines.push(format!("  [{}]  {}  —  {}", label, name, truncate_id(&u.id, 24)));
                }
            }
            app.set_content("Users", lines);
        }
        Err(e) => {
            app.push_event(format!("[USERS] Fetch failed: {e}"));
            show_lines(app, "Users", vec![format!("Error fetching users: {e}")]);
        }
    }

    Ok(())
}

fn user_lines(user: &User) -> Vec<String> {
    let role = if user.is_local() { "LOCAL" } else { "REMOTE" };
    let name = user.meta.display_name.as_deref().unwrap_or("(unnamed)");
    vec![
        format!("[{}]  {}", role, name),
        format!("  id         : {}", user.id),
        format!("  public_key : {}", user.public_key),
    ]
}

// ---------------------------------------------------------------------------
// Connections
// ---------------------------------------------------------------------------

async fn cmd_connection(app: &mut App, rest: &str) -> Result<()> {
    let arg = rest.trim();
    if arg.is_empty() {
        show_lines(app, "Connection", vec!["Usage: /connection <nick>".to_string()]);
        return Ok(());
    }

    let to_id = match resolve_nick(arg) {
        Some(id) => id,
        None => {
            show_lines(app, "Connection", vec![format!(
                "No user found with nick '{}'. Use /users to see known users.", arg
            )]);
            return Ok(());
        }
    };

    let tx = match &app.node_tx {
        Some(tx) => tx.clone(),
        None => {
            show_lines(app, "Connection", vec!["Node is not running. Use /startNode first.".to_string()]);
            return Ok(());
        }
    };

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(FullNodeCommand::CreateConnection { to_id: to_id.clone(), reply: reply_tx })
        .await
        .map_err(|_| anyhow!("Node channel closed"))?;

    match reply_rx.await? {
        Ok(conn) => {
            let state = if conn.is_established() { "established" } else { "pending" };
            app.push_event(format!("[CONN] → {} [{}]", truncate_id(&conn.to_id, 16), state));
            app.push_output(format!("Connection initiated with {} [{}].", arg, state));
            let lines = vec![
                format!("Connection initiated  [{}]", state),
                String::new(),
                format!("  from  : {}", conn.from_id),
                format!("  to    : {}", conn.to_id),
                format!("  state : {}", state),
            ];
            if !app.connections.iter().any(|c| c.to_id == conn.to_id) {
                app.connections.push(conn);
            }
            app.set_content("Connection", lines);
        }
        Err(e) => {
            app.push_event(format!("[CONN] Create failed: {e}"));
            show_lines(app, "Connection", vec![format!("Error creating connection: {e}")]);
        }
    }

    Ok(())
}

fn cmd_connections(app: &mut App) -> Result<()> {
    let local_user = load_local_user(None);
    let from_id = local_user.as_ref().map(|u| u.id.clone()).unwrap_or_default();

    let to_ids = list_connections(None).unwrap_or_default();
    let mut conns: Vec<Connection> = Vec::new();
    for to_id in &to_ids {
        if let Ok(c) = load_connection(&from_id, to_id, None) {
            conns.push(c);
        }
    }
    app.connections = conns.clone();

    let mut lines = vec![format!("Connections  ({})", conns.len()), String::new()];
    if conns.is_empty() {
        lines.push("  No connections on record.".to_string());
    } else {
        for c in &conns {
            let state = if c.is_established() { "established" } else { "pending   " };
            lines.push(format!("  [{}]  {} → {}", state, truncate_id(&c.from_id, 16), truncate_id(&c.to_id, 16)));
        }
    }
    app.push_output(format!("Connections: {}.", conns.len()));
    app.set_content("Connections", lines);
    Ok(())
}

fn cmd_connections_pending(app: &mut App) -> Result<()> {
    let local_user = load_local_user(None);
    let from_id = local_user.as_ref().map(|u| u.id.clone()).unwrap_or_default();

    let to_ids = list_connections(None).unwrap_or_default();
    let pending: Vec<Connection> = to_ids
        .iter()
        .filter_map(|to_id| load_connection(&from_id, to_id, None).ok())
        .filter(|c| !c.is_established())
        .collect();

    let mut lines = vec![format!("Pending connections  ({})", pending.len()), String::new()];
    if pending.is_empty() {
        lines.push("  No pending connections.".to_string());
    } else {
        for c in &pending {
            lines.push(format!("  {} → {}", truncate_id(&c.from_id, 16), truncate_id(&c.to_id, 16)));
            if let Some(pub_k) = &c.public_key {
                lines.push(format!("    our_public_key: {}", pub_k));
            }
        }
    }
    app.set_content("Connections (Pending)", lines);
    Ok(())
}

async fn cmd_accept_connection(app: &mut App, rest: &str) -> Result<()> {
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    if parts.len() < 2 {
        show_lines(app, "Accept Connection", vec!["Usage: /acceptConnection <from_id> <their_public_key>".to_string()]);
        return Ok(());
    }
    let from_id = parts[0].trim();
    let their_pub_key = parts[1].trim();

    let tx = match &app.node_tx {
        Some(tx) => tx.clone(),
        None => {
            show_lines(app, "Accept Connection", vec!["Node is not running. Use /startNode first.".to_string()]);
            return Ok(());
        }
    };

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(FullNodeCommand::AcceptConnection {
        from_id: from_id.to_string(),
        their_public_key: their_pub_key.to_string(),
        reply: reply_tx,
    })
    .await
    .map_err(|_| anyhow!("Node channel closed"))?;

    match reply_rx.await? {
        Ok(conn) => {
            app.push_event(format!("[CONN] Accepted from {} — DH key established.", truncate_id(&conn.from_id, 16)));
            app.push_output(format!("Connection with {} accepted.", conn.from_id));
            let lines = vec![
                format!("Connection accepted  [established]"),
                String::new(),
                format!("  from  : {}", conn.from_id),
                format!("  to    : {}", conn.to_id),
            ];
            let idx = app.connections.iter().position(|c| c.from_id == conn.from_id);
            match idx {
                Some(i) => app.connections[i] = conn,
                None => app.connections.push(conn),
            }
            app.set_content("Accept Connection", lines);
        }
        Err(e) => {
            app.push_event(format!("[CONN] Accept failed: {e}"));
            show_lines(app, "Accept Connection", vec![format!("Error accepting connection: {e}")]);
        }
    }

    Ok(())
}

fn cmd_decline_connection(app: &mut App, rest: &str) {
    let user_id = rest.trim();
    if user_id.is_empty() {
        show_lines(app, "Decline Connection", vec!["Usage: /declineConnection <connection_id>".to_string()]);
        return;
    }
    app.connections.retain(|c| c.to_id != user_id && c.from_id != user_id);
    app.push_event(format!("[CONN] Declined connection with {}.", truncate_id(user_id, 16)));
    show_lines(app, "Decline Connection", vec![
        format!("Connection with {} removed locally.", user_id),
        "(Network-level decline not yet implemented in the library.)".to_string(),
    ]);
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

async fn cmd_message(app: &mut App, rest: &str) -> Result<()> {
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    if parts.len() < 2 {
        show_lines(app, "Message", vec!["Usage: /message <nick> <body>".to_string()]);
        return Ok(());
    }
    let nick = parts[0].trim();
    let body = parts[1].trim();

    let to_id = match resolve_nick(nick) {
        Some(id) => id,
        None => {
            show_lines(app, "Message", vec![format!(
                "No user found with nick '{}'. Use /users to see known users.", nick
            )]);
            return Ok(());
        }
    };

    send_message(app, nick, &to_id, "text", serde_json::json!({ "text": body })).await
}

async fn cmd_message_plugin(app: &mut App, rest: &str) -> Result<()> {
    let parts: Vec<&str> = rest.splitn(3, ' ').collect();
    if parts.len() < 3 {
        show_lines(app, "Message", vec!["Usage: /messagePlugin <nick> <plugin_type> <plugin_body>".to_string()]);
        return Ok(());
    }
    let nick = parts[0].trim();
    let plugin_type = parts[1].trim();
    let plugin_body_str = parts[2].trim();

    let to_id = match resolve_nick(nick) {
        Some(id) => id,
        None => {
            show_lines(app, "Message", vec![format!(
                "No user found with nick '{}'. Use /users to see known users.", nick
            )]);
            return Ok(());
        }
    };

    let plugin_body = serde_json::from_str(plugin_body_str)
        .unwrap_or_else(|_| serde_json::json!({ "raw": plugin_body_str }));

    send_message(app, nick, &to_id, plugin_type, plugin_body).await
}

async fn send_message(
    app: &mut App,
    nick: &str,
    to_id: &str,
    plugin_type: &str,
    plugin_body: serde_json::Value,
) -> Result<()> {
    let tx = match &app.node_tx {
        Some(tx) => tx.clone(),
        None => {
            show_lines(app, "Message", vec!["Node is not running. Use /startNode first.".to_string()]);
            return Ok(());
        }
    };

    let local_user = load_local_user(None)
        .map_err(|_| anyhow!("No local user — run /user first"))?;

    let msg = accord_network::Message::new(
        local_user.id.clone(),
        to_id,
        plugin_type,
        plugin_body.clone(),
    );
    let data = serde_json::to_vec(&msg)?;

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(FullNodeCommand::StoreMessage { data, reply: reply_tx })
        .await
        .map_err(|_| anyhow!("Node channel closed"))?;

    match reply_rx.await? {
        Ok(hash) => {
            let line = format!(
                "[{}→{}]  [{}]  {}",
                truncate_id(&local_user.id, 8),
                truncate_id(to_id, 8),
                plugin_type,
                plugin_body
            );
            app.messages.push(line.clone());
            app.push_event(format!("[MSG] → {} [{}] (hash: {})", nick, plugin_type, truncate_id(&hash, 12)));
            app.push_output(format!("Message sent to {} (hash: {}).", nick, hash));
            app.set_content("Message", vec![
                format!("Message sent  [{}]", plugin_type),
                String::new(),
                format!("  to   : {} ({})", nick, truncate_id(to_id, 16)),
                format!("  body : {}", plugin_body),
                format!("  hash : {}", hash),
            ]);
        }
        Err(e) => {
            app.push_event(format!("[MSG] Send failed: {e}"));
            show_lines(app, "Message", vec![format!("Error storing message: {e}")]);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a display-name (nick) to a user ID (case-insensitive).
fn resolve_nick(nick: &str) -> Option<String> {
    if let Ok(local) = load_local_user(None) {
        if local.meta.display_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(nick)) {
            return Some(local.id);
        }
    }
    let ids = list_known_users(None).unwrap_or_default();
    for id in ids {
        if let Ok(meta) = load_known_user(&id, None) {
            if meta.display_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(nick)) {
                return Some(id);
            }
        }
    }
    None
}

/// Set the content area to a small list of lines with the given title.
fn show_lines(app: &mut App, title: &str, lines: Vec<String>) {
    app.set_content(title, lines);
}

fn split_command(input: &str) -> (&str, &str) {
    match input.find(' ') {
        Some(idx) => (&input[..idx], input[idx + 1..].trim_start()),
        None => (input, ""),
    }
}

fn truncate_id(id: &str, max: usize) -> String {
    if id.len() <= max {
        id.to_owned()
    } else {
        format!("{}…", &id[..max])
    }
}
