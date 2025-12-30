use std::sync::Arc;
use std::time::Duration;

use num_format::{Locale, ToFormattedString};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, List, ListItem, Paragraph, Row,
        Sparkline, Table, Tabs,
    },
    Frame,
};

use crate::state::{AppState, ConnectionState, LogLevel};

/// Tab titles
const TAB_TITLES: [&str; 4] = ["📊 Overview", "📦 Slots", "💰 Transactions", "📜 Logs"];

/// Format a large number with commas
fn format_number(n: u64) -> String {
    n.to_formatted_string(&Locale::en)
}

/// Format duration as human-readable string
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

/// Main UI rendering function
pub fn draw(f: &mut Frame, state: &Arc<AppState>) {
    let size = f.area();

    // Main layout: header, tabs, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(3),  // Tabs
            Constraint::Min(10),    // Content
            Constraint::Length(3),  // Footer
        ])
        .split(size);

    draw_header(f, state, chunks[0]);
    draw_tabs(f, state, chunks[1]);
    draw_content(f, state, chunks[2]);
    draw_footer(f, state, chunks[3]);

    // Draw help overlay if enabled
    if *state.show_help.read() {
        draw_help_overlay(f, state);
    }
}

/// Draw the header with connection info and live stats
fn draw_header(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let conn_state = state.connection_state.read().clone();
    let (status_color, status_icon) = match &conn_state {
        ConnectionState::Connected => (Color::Green, "●"),
        ConnectionState::Connecting | ConnectionState::Reconnecting => (Color::Yellow, "◐"),
        ConnectionState::Disconnected => (Color::Gray, "○"),
        ConnectionState::Error(_) => (Color::Red, "✖"),
    };

    let uptime = format_duration(state.uptime());
    let current_slot = state.current_slot.load(std::sync::atomic::Ordering::Relaxed);
    
    let window_secs = state.metrics_window_secs();
    let entries_per_sec = state.metrics.get_entries_per_sec(window_secs);
    let txns_per_sec = state.metrics.get_txns_per_sec(window_secs);

    let header_text = vec![
        Span::styled("🔗 ShredStream TUI ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(format!("{}", conn_state), Style::default().fg(status_color)),
        Span::raw(" │ "),
        Span::styled("Slot: ", Style::default().fg(Color::Gray)),
        Span::styled(format_number(current_slot), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(format!("{:.1} entries/s", entries_per_sec), Style::default().fg(Color::Cyan)),
        Span::raw(" │ "),
        Span::styled(format!("{:.1} txns/s", txns_per_sec), Style::default().fg(Color::Magenta)),
        Span::raw(" │ "),
        Span::styled("Uptime: ", Style::default().fg(Color::Gray)),
        Span::styled(uptime, Style::default().fg(Color::White)),
    ];

    let header = Paragraph::new(Line::from(header_text))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(header, area);
}

/// Draw the tab bar
fn draw_tabs(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let selected = *state.selected_tab.read();
    
    let titles: Vec<Line> = TAB_TITLES
        .iter()
        .map(|t| Line::from(*t))
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .select(selected)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .divider(symbols::line::VERTICAL);

    f.render_widget(tabs, area);
}

/// Draw the main content based on selected tab
fn draw_content(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let selected = *state.selected_tab.read();
    
    match selected {
        0 => draw_overview_tab(f, state, area),
        1 => draw_slots_tab(f, state, area),
        2 => draw_transactions_tab(f, state, area),
        3 => draw_logs_tab(f, state, area),
        _ => {}
    }
}

/// Draw the overview tab with metrics and stats
fn draw_overview_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),  // Connection info
            Constraint::Length(10),  // Current metrics
            Constraint::Min(5),      // Sparkline
        ])
        .split(chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // Cumulative stats
            Constraint::Min(5),     // Recent slots
        ])
        .split(chunks[1]);

    // Connection info
    draw_connection_info(f, state, left_chunks[0]);
    
    // Current metrics
    draw_current_metrics(f, state, left_chunks[1]);
    
    // Sparkline
    draw_rate_sparkline(f, state, left_chunks[2]);
    
    // Cumulative stats
    draw_cumulative_stats(f, state, right_chunks[0]);
    
    // Recent slots preview
    draw_recent_slots_preview(f, state, right_chunks[1]);
}

/// Draw connection information panel
fn draw_connection_info(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let conn_state = state.connection_state.read().clone();
    let conn_duration = state.connection_duration()
        .map(format_duration)
        .unwrap_or_else(|| "N/A".to_string());
    let reconnects = state.reconnect_count.load(std::sync::atomic::Ordering::Relaxed);

    let info_text = vec![
        Line::from(vec![
            Span::styled("Proxy URL: ", Style::default().fg(Color::Gray)),
            Span::styled(&state.proxy_url, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", conn_state), Style::default().fg(
                match conn_state {
                    ConnectionState::Connected => Color::Green,
                    ConnectionState::Connecting | ConnectionState::Reconnecting => Color::Yellow,
                    _ => Color::Red,
                }
            )),
        ]),
        Line::from(vec![
            Span::styled("Connected for: ", Style::default().fg(Color::Gray)),
            Span::styled(conn_duration, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Reconnects: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(reconnects), Style::default().fg(
                if reconnects > 0 { Color::Yellow } else { Color::White }
            )),
        ]),
    ];

    let block = Block::default()
        .title(" Connection ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(info_text).block(block);
    f.render_widget(paragraph, area);
}

/// Draw current window metrics
fn draw_current_metrics(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let window_secs = state.metrics_window_secs();
    let metrics = &state.metrics;

    let entries = metrics.entry_count.load(std::sync::atomic::Ordering::Relaxed);
    let txns = metrics.txn_count.load(std::sync::atomic::Ordering::Relaxed);
    let recovered = metrics.recovered_count.load(std::sync::atomic::Ordering::Relaxed);

    let metrics_text = vec![
        Line::from(vec![
            Span::styled("Window: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}s", window_secs), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Entries: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(entries), Style::default().fg(Color::Cyan)),
            Span::styled(format!(" ({:.1}/s)", metrics.get_entries_per_sec(window_secs)), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Transactions: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(txns), Style::default().fg(Color::Magenta)),
            Span::styled(format!(" ({:.1}/s)", metrics.get_txns_per_sec(window_secs)), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Recovered: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(recovered), Style::default().fg(Color::Yellow)),
        ]),
    ];

    let block = Block::default()
        .title(" Current Window ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(metrics_text).block(block);
    f.render_widget(paragraph, area);
}

/// Draw rate sparkline
fn draw_rate_sparkline(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    // Get transaction counts from recent slots for sparkline
    let slot_history = state.slot_history.read();
    let data: Vec<u64> = slot_history.iter()
        .map(|s| s.txn_count)
        .collect();

    let block = Block::default()
        .title(" Transaction Rate ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let sparkline = Sparkline::default()
        .block(block)
        .data(&data)
        .style(Style::default().fg(Color::Magenta));

    f.render_widget(sparkline, area);
}

/// Draw cumulative statistics
fn draw_cumulative_stats(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let metrics = &state.metrics;
    
    let total_entries = metrics.total_entries.load(std::sync::atomic::Ordering::Relaxed);
    let total_txns = metrics.total_txns.load(std::sync::atomic::Ordering::Relaxed);
    let total_received = metrics.total_received.load(std::sync::atomic::Ordering::Relaxed);
    let total_success = metrics.total_success_forward.load(std::sync::atomic::Ordering::Relaxed);
    let total_fail = metrics.total_fail_forward.load(std::sync::atomic::Ordering::Relaxed);
    let total_dup = metrics.total_duplicate.load(std::sync::atomic::Ordering::Relaxed);

    let stats_text = vec![
        Line::from(vec![
            Span::styled("Total Entries: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(total_entries), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Total Transactions: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(total_txns), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled("── Forwarding Stats ──", Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled("Received: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(total_received), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Forwarded: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(total_success), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Failed: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(total_fail), Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled("Duplicates: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(total_dup), Style::default().fg(Color::Yellow)),
        ]),
    ];

    let block = Block::default()
        .title(" Cumulative Statistics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(stats_text).block(block);
    f.render_widget(paragraph, area);
}

/// Draw recent slots preview
fn draw_recent_slots_preview(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let slot_history = state.slot_history.read();
    
    let items: Vec<ListItem> = slot_history.iter()
        .rev()
        .take(10)
        .map(|slot| {
            let content = Line::from(vec![
                Span::styled(format!("{}", slot.slot), Style::default().fg(Color::White)),
                Span::raw(" │ "),
                Span::styled(format!("{} entries", slot.entry_count), Style::default().fg(Color::Cyan)),
                Span::raw(", "),
                Span::styled(format!("{} txns", slot.txn_count), Style::default().fg(Color::Magenta)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let block = Block::default()
        .title(" Recent Slots ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

/// Draw the slots tab with detailed slot history
fn draw_slots_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let slot_history = state.slot_history.read();
    
    let header = Row::new(vec![
        Cell::from("Slot").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Entries").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Transactions").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Time").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = slot_history.iter()
        .rev()
        .map(|slot| {
            Row::new(vec![
                Cell::from(format_number(slot.slot)).style(Style::default().fg(Color::White)),
                Cell::from(format_number(slot.entry_count)).style(Style::default().fg(Color::Cyan)),
                Cell::from(format_number(slot.txn_count)).style(Style::default().fg(Color::Magenta)),
                Cell::from(slot.timestamp.format("%H:%M:%S%.3f").to_string()).style(Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    let table = Table::new(rows, [
        Constraint::Length(15),
        Constraint::Length(12),
        Constraint::Length(15),
        Constraint::Length(15),
    ])
    .header(header)
    .block(
        Block::default()
            .title(" Slot History ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(table, area);
}

/// Draw the transactions tab
fn draw_transactions_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let txn_samples = state.txn_samples.read();
    
    let header = Row::new(vec![
        Cell::from("Slot").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Signature").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Time").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = txn_samples.iter()
        .rev()
        .map(|txn| {
            // Truncate signature for display
            let sig_display = if txn.signature.len() > 44 {
                format!("{}...{}", &txn.signature[..20], &txn.signature[txn.signature.len()-20..])
            } else {
                txn.signature.clone()
            };
            
            Row::new(vec![
                Cell::from(format_number(txn.slot)).style(Style::default().fg(Color::White)),
                Cell::from(sig_display).style(Style::default().fg(Color::Yellow)),
                Cell::from(txn.received_at.format("%H:%M:%S%.3f").to_string()).style(Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    let table = Table::new(rows, [
        Constraint::Length(15),
        Constraint::Min(50),
        Constraint::Length(15),
    ])
    .header(header)
    .block(
        Block::default()
            .title(" Recent Transactions (Sample) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(table, area);
}

/// Draw the logs tab
fn draw_logs_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let logs = state.logs.read();
    
    let items: Vec<ListItem> = logs.iter()
        .rev()
        .map(|log| {
            let level_style = match log.level {
                LogLevel::Info => Style::default().fg(Color::Cyan),
                LogLevel::Warn => Style::default().fg(Color::Yellow),
                LogLevel::Error => Style::default().fg(Color::Red),
                LogLevel::Debug => Style::default().fg(Color::Gray),
            };
            
            let content = Line::from(vec![
                Span::styled(
                    log.timestamp.format("%H:%M:%S").to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(" "),
                Span::styled(format!("[{}]", log.level), level_style),
                Span::raw(" "),
                Span::styled(&log.message, Style::default().fg(Color::White)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let block = Block::default()
        .title(" Logs ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

/// Draw the footer with keyboard shortcuts
fn draw_footer(f: &mut Frame, _state: &Arc<AppState>, area: Rect) {
    let shortcuts = vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::styled(" Quit ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" ←/→", Style::default().fg(Color::Yellow)),
        Span::styled(" Switch Tab ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" ↑/↓", Style::default().fg(Color::Yellow)),
        Span::styled(" Scroll ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" r", Style::default().fg(Color::Yellow)),
        Span::styled(" Reset Metrics ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" ?", Style::default().fg(Color::Yellow)),
        Span::styled(" Help ", Style::default().fg(Color::Gray)),
    ];

    let footer = Paragraph::new(Line::from(shortcuts))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(footer, area);
}

/// Draw help overlay
fn draw_help_overlay(f: &mut Frame, _state: &Arc<AppState>) {
    let area = f.area();
    
    // Create centered popup
    let popup_width = 50;
    let popup_height = 15;
    let popup_area = Rect::new(
        (area.width.saturating_sub(popup_width)) / 2,
        (area.height.saturating_sub(popup_height)) / 2,
        popup_width.min(area.width),
        popup_height.min(area.height),
    );

    // Clear the area
    f.render_widget(ratatui::widgets::Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled("Keyboard Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  q, Ctrl+C  ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit the application"),
        ]),
        Line::from(vec![
            Span::styled("  ←, →       ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch between tabs"),
        ]),
        Line::from(vec![
            Span::styled("  ↑, ↓       ", Style::default().fg(Color::Yellow)),
            Span::raw("Scroll up/down"),
        ]),
        Line::from(vec![
            Span::styled("  Tab        ", Style::default().fg(Color::Yellow)),
            Span::raw("Next tab"),
        ]),
        Line::from(vec![
            Span::styled("  r          ", Style::default().fg(Color::Yellow)),
            Span::raw("Reset current metrics window"),
        ]),
        Line::from(vec![
            Span::styled("  ?          ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);

    f.render_widget(paragraph, popup_area);
}
