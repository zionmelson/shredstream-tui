use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use num_format::{Locale, ToFormattedString};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, List, ListItem, Paragraph, Row,
        Sparkline, Table, Tabs, Clear,
    },
    Frame,
};

use crate::state::{AppState, ConnectionState, LogLevel};
use crate::programs::ProgramCategory;

/// Tab titles - 8 tabs total
const TAB_TITLES: [&str; 8] = [
    "📊 Overview",
    "⏱️ Latency",
    "🌳 Turbine",
    "📦 Programs",
    "👑 Leaders",
    "🏆 Competition",
    "📜 Logs",
    "💰 Wallet",
];

fn format_number(n: u64) -> String {
    n.to_formatted_string(&Locale::en)
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn truncate_pubkey(s: &str) -> String {
    if s.len() > 12 {
        format!("{}..{}", &s[..6], &s[s.len()-4..])
    } else {
        s.to_string()
    }
}

/// Main UI rendering function
pub fn draw(f: &mut Frame, state: &Arc<AppState>) {
    let size = f.area();

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

    if *state.show_help.read() {
        draw_help_overlay(f, state);
    }
}

fn draw_header(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let conn_state = state.connection_state.read().clone();
    let (status_color, status_icon) = match &conn_state {
        ConnectionState::Connected => (Color::Green, "●"),
        ConnectionState::Connecting | ConnectionState::Reconnecting => (Color::Yellow, "◐"),
        ConnectionState::Disconnected => (Color::Gray, "○"),
        ConnectionState::Error(_) => (Color::Red, "✖"),
    };

    let uptime = format_duration(state.uptime());
    let current_slot = state.current_slot.load(Ordering::Relaxed);
    
    let window_secs = state.metrics_window_secs();
    let entries_per_sec = state.metrics.get_entries_per_sec(window_secs);
    let txns_per_sec = state.metrics.get_txns_per_sec(window_secs);
    
    // MEV metrics
    let dex_count = state.program_stats.dex_txn_count.load(Ordering::Relaxed);
    let bundles = state.competition_stats.bundle_count.load(Ordering::Relaxed);
    let avg_latency = state.latency_stats.avg_latency_ms();
    let turbine_avg = state.turbine_stats.avg_index();

    let header_text = vec![
        Span::styled("🔗 ShredStream MEV ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(format!("{}", conn_state), Style::default().fg(status_color)),
        Span::raw(" │ "),
        Span::styled("Slot: ", Style::default().fg(Color::Gray)),
        Span::styled(format_number(current_slot), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(format!("{:.0} txn/s", txns_per_sec), Style::default().fg(Color::Magenta)),
        Span::raw(" │ "),
        Span::styled(format!("{:.1}ms", avg_latency), Style::default().fg(Color::Yellow)),
        Span::raw(" │ "),
        Span::styled(format!("T:{:.0}", turbine_avg), Style::default().fg(Color::Cyan)),
        Span::raw(" │ "),
        Span::styled(format!("DEX:{}", format_number(dex_count)), Style::default().fg(Color::Green)),
        Span::raw(" │ "),
        Span::styled(uptime, Style::default().fg(Color::DarkGray)),
    ];

    let header = Paragraph::new(Line::from(header_text))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(header, area);
}

fn draw_tabs(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let selected = *state.selected_tab.read();
    
    let titles: Vec<Line> = TAB_TITLES.iter().map(|t| Line::from(*t)).collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .select(selected)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .divider(symbols::line::VERTICAL);

    f.render_widget(tabs, area);
}

fn draw_content(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let selected = *state.selected_tab.read();
    
    match selected {
        0 => draw_overview_tab(f, state, area),
        1 => draw_latency_tab(f, state, area),
        2 => draw_turbine_tab(f, state, area),
        3 => draw_programs_tab(f, state, area),
        4 => draw_leaders_tab(f, state, area),
        5 => draw_competition_tab(f, state, area),
        6 => draw_logs_tab(f, state, area),
        7 => draw_wallet_tab(f, state, area),
        _ => {}
    }
}

// ============================================================================
// Tab 0: Overview
// ============================================================================

fn draw_overview_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),   // Connection + Core metrics
            Constraint::Length(10),  // MEV metrics
            Constraint::Min(5),      // Sparkline
        ])
        .split(chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Network health
            Constraint::Min(5),     // Recent slots
        ])
        .split(chunks[1]);

    draw_connection_metrics(f, state, left_chunks[0]);
    draw_mev_summary(f, state, left_chunks[1]);
    draw_rate_sparkline(f, state, left_chunks[2]);
    draw_network_health(f, state, right_chunks[0]);
    draw_recent_slots(f, state, right_chunks[1]);
}

fn draw_connection_metrics(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let window_secs = state.metrics_window_secs();
    let metrics = &state.metrics;

    let conn_duration = state.connection_duration()
        .map(format_duration)
        .unwrap_or_else(|| "N/A".to_string());

    let text = vec![
        Line::from(vec![
            Span::styled("Entries: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(metrics.total_entries.load(Ordering::Relaxed)), Style::default().fg(Color::Cyan)),
            Span::styled(format!(" ({:.1}/s)", metrics.get_entries_per_sec(window_secs)), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Transactions: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(metrics.total_txns.load(Ordering::Relaxed)), Style::default().fg(Color::Magenta)),
            Span::styled(format!(" ({:.1}/s)", metrics.get_txns_per_sec(window_secs)), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Connected: ", Style::default().fg(Color::Gray)),
            Span::styled(conn_duration, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Reconnects: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(state.reconnect_count.load(Ordering::Relaxed)), Style::default().fg(Color::Yellow)),
        ]),
    ];

    let block = Block::default()
        .title(" Core Metrics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_mev_summary(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let program_stats = &state.program_stats;
    let competition = &state.competition_stats;
    let latency = &state.latency_stats;
    let turbine = &state.turbine_stats;

    let text = vec![
        Line::from(Span::styled("── DEX Activity ──", Style::default().fg(Color::Green))),
        Line::from(vec![
            Span::styled("DEX Txns: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(program_stats.dex_txn_count.load(Ordering::Relaxed)), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Lending: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(program_stats.lending_txn_count.load(Ordering::Relaxed)), Style::default().fg(Color::Blue)),
        ]),
        Line::from(Span::styled("── Competition ──", Style::default().fg(Color::Yellow))),
        Line::from(vec![
            Span::styled("Bundles: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(competition.bundle_count.load(Ordering::Relaxed)), Style::default().fg(Color::Yellow)),
            Span::styled(format!(" ({:.4} SOL tips)", competition.total_tips_sol()), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Duplicates: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(competition.duplicate_count.load(Ordering::Relaxed)), Style::default().fg(Color::Red)),
        ]),
    ];

    let block = Block::default()
        .title(" MEV Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_rate_sparkline(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let slot_history = state.slot_history.read();
    let data: Vec<u64> = slot_history.iter().map(|s| s.txn_count).collect();

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

fn draw_network_health(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let health = &state.network_health;
    let latency = &state.latency_stats;
    let turbine = &state.turbine_stats;

    let fec_rate = health.fec_recovery_rate();
    let hb_rate = health.heartbeat_success_rate();

    let text = vec![
        Line::from(vec![
            Span::styled("Avg Latency: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.2} ms", latency.avg_latency_ms()), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Min/Max: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.2}/{:.2} ms", latency.min_latency_ms(), latency.max_latency_ms()), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Turbine Idx: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1} avg", turbine.avg_index()), Style::default().fg(Color::Cyan)),
            Span::styled(format!(" ({}–{})", turbine.min_index(), turbine.max_index()), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("FEC Recovery: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}%", fec_rate), Style::default().fg(if fec_rate < 10.0 { Color::Green } else { Color::Yellow })),
        ]),
        Line::from(vec![
            Span::styled("Heartbeat: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}%", hb_rate), Style::default().fg(if hb_rate > 95.0 { Color::Green } else { Color::Red })),
        ]),
    ];

    let block = Block::default()
        .title(" Network Health ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_recent_slots(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let slot_history = state.slot_history.read();
    
    let items: Vec<ListItem> = slot_history.iter()
        .rev()
        .take(15)
        .map(|slot| {
            let mut spans = vec![
                Span::styled(format!("{}", slot.slot), Style::default().fg(Color::White)),
                Span::raw(" │ "),
                Span::styled(format!("{} ent", slot.entry_count), Style::default().fg(Color::Cyan)),
                Span::raw(", "),
                Span::styled(format!("{} txn", slot.txn_count), Style::default().fg(Color::Magenta)),
            ];
            if slot.dex_txn_count > 0 {
                spans.push(Span::raw(" │ "));
                spans.push(Span::styled(format!("{} dex", slot.dex_txn_count), Style::default().fg(Color::Green)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let block = Block::default()
        .title(" Recent Slots ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

// ============================================================================
// Tab 1: Latency
// ============================================================================

fn draw_latency_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(5)])
        .split(chunks[0]);

    // Global latency stats
    let latency = &state.latency_stats;
    let stats_text = vec![
        Line::from(Span::styled("── Global Latency ──", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Average: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.2} ms", latency.avg_latency_ms()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Minimum: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.2} ms", latency.min_latency_ms()), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Maximum: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.2} ms", latency.max_latency_ms()), Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled("Samples: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(latency.sample_count.load(Ordering::Relaxed)), Style::default().fg(Color::White)),
        ]),
    ];

    let stats_block = Block::default()
        .title(" Latency Statistics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(Paragraph::new(stats_text).block(stats_block), left_chunks[0]);

    // Region latencies
    let region_stats = latency.region_latencies.read();
    let mut regions: Vec<_> = region_stats.values().collect();
    regions.sort_by(|a, b| a.avg_latency_ms().partial_cmp(&b.avg_latency_ms()).unwrap());

    let region_items: Vec<ListItem> = regions.iter().map(|r| {
        ListItem::new(Line::from(vec![
            Span::styled(&r.region, Style::default().fg(Color::Cyan)),
            Span::raw(": "),
            Span::styled(format!("{:.2} ms avg", r.avg_latency_ms()), Style::default().fg(Color::Yellow)),
            Span::styled(format!(" ({} samples)", r.sample_count), Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let region_block = Block::default()
        .title(" By Region ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(List::new(region_items).block(region_block), left_chunks[1]);

    // Leader latencies
    let leader_stats = latency.leader_latencies.read();
    let mut leaders: Vec<_> = leader_stats.values().collect();
    leaders.sort_by(|a, b| a.avg_latency_ms().partial_cmp(&b.avg_latency_ms()).unwrap());

    let header = Row::new(vec![
        Cell::from("Leader").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Avg").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Min").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Max").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Count").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = leaders.iter().take(20).map(|l| {
        Row::new(vec![
            Cell::from(truncate_pubkey(&l.leader.to_string())).style(Style::default().fg(Color::White)),
            Cell::from(format!("{:.2}ms", l.avg_latency_ms())).style(Style::default().fg(Color::Yellow)),
            Cell::from(format!("{:.2}ms", l.min_latency_us as f64 / 1000.0)).style(Style::default().fg(Color::Green)),
            Cell::from(format!("{:.2}ms", l.max_latency_us as f64 / 1000.0)).style(Style::default().fg(Color::Red)),
            Cell::from(format!("{}", l.sample_count)).style(Style::default().fg(Color::Gray)),
        ])
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(8),
    ])
    .header(header)
    .block(Block::default().title(" By Leader ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(table, chunks[1]);
}

// ============================================================================
// Tab 2: Turbine
// ============================================================================

fn draw_turbine_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(14), Constraint::Min(5)])
        .split(area);

    let turbine = &state.turbine_stats;
    let total = turbine.total_samples.load(Ordering::Relaxed) as f64;
    
    let layer0 = turbine.layer_0_count.load(Ordering::Relaxed);
    let layer1 = turbine.layer_1_count.load(Ordering::Relaxed);
    let layer2 = turbine.layer_2_count.load(Ordering::Relaxed);
    let layer3 = turbine.layer_3_plus_count.load(Ordering::Relaxed);

    let layer0_pct = if total > 0.0 { (layer0 as f64 / total) * 100.0 } else { 0.0 };
    let layer1_pct = if total > 0.0 { (layer1 as f64 / total) * 100.0 } else { 0.0 };
    let layer2_pct = if total > 0.0 { (layer2 as f64 / total) * 100.0 } else { 0.0 };
    let layer3_pct = if total > 0.0 { (layer3 as f64 / total) * 100.0 } else { 0.0 };

    let text = vec![
        Line::from(Span::styled("── Turbine Tree Position ──", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Average Index: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}", turbine.avg_index()), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" (lower = earlier in propagation)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("Range: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{} – {}", turbine.min_index(), turbine.max_index()), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Samples: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(turbine.total_samples.load(Ordering::Relaxed)), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled("── Layer Distribution ──", Style::default().fg(Color::Yellow))),
        Line::from(vec![
            Span::styled("Layer 0 (Root): ", Style::default().fg(Color::Green)),
            Span::styled(format!("{} ({:.1}%)", format_number(layer0), layer0_pct), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Layer 1: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{} ({:.1}%)", format_number(layer1), layer1_pct), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Layer 2: ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{} ({:.1}%)", format_number(layer2), layer2_pct), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Layer 3+: ", Style::default().fg(Color::Red)),
            Span::styled(format!("{} ({:.1}%)", format_number(layer3), layer3_pct), Style::default().fg(Color::White)),
        ]),
    ];

    let block = Block::default()
        .title(" Turbine Tree Analysis ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(Paragraph::new(text).block(block), chunks[0]);

    // Recent samples
    let samples = turbine.samples.read();
    let items: Vec<ListItem> = samples.iter().rev().take(20).map(|s| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("Slot {}", s.slot), Style::default().fg(Color::White)),
            Span::raw(" │ "),
            Span::styled(format!("idx:{}", s.turbine_index), Style::default().fg(Color::Cyan)),
            Span::raw(" │ "),
            Span::styled(format!("layer:{}", s.layer), Style::default().fg(match s.layer {
                0 => Color::Green,
                1 => Color::Cyan,
                2 => Color::Yellow,
                _ => Color::Red,
            })),
            Span::raw(" │ "),
            Span::styled(s.timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let samples_block = Block::default()
        .title(" Recent Samples ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(List::new(items).block(samples_block), chunks[1]);
}

// ============================================================================
// Tab 3: Programs
// ============================================================================

fn draw_programs_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Top programs table
    let programs = state.program_stats.get_top_programs(30);
    
    let header = Row::new(vec![
        Cell::from("Program").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Category").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Txns").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Last Seen").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = programs.iter().map(|p| {
        let cat_color = match p.category {
            ProgramCategory::Dex => Color::Green,
            ProgramCategory::Lending => Color::Blue,
            ProgramCategory::Mev => Color::Yellow,
            ProgramCategory::Staking => Color::Magenta,
            _ => Color::Gray,
        };
        Row::new(vec![
            Cell::from(p.name.clone()).style(Style::default().fg(Color::White)),
            Cell::from(format!("{}", p.category)).style(Style::default().fg(cat_color)),
            Cell::from(format_number(p.txn_count)).style(Style::default().fg(Color::Cyan)),
            Cell::from(p.last_seen.format("%H:%M:%S").to_string()).style(Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let table = Table::new(rows, [
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(10),
    ])
    .header(header)
    .block(Block::default().title(" Top Programs ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(table, chunks[0]);

    // Category summary
    let ps = &state.program_stats;
    let text = vec![
        Line::from(Span::styled("── By Category ──", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("🔄 DEX: ", Style::default().fg(Color::Green)),
            Span::styled(format_number(ps.dex_txn_count.load(Ordering::Relaxed)), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("🏦 Lending: ", Style::default().fg(Color::Blue)),
            Span::styled(format_number(ps.lending_txn_count.load(Ordering::Relaxed)), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("⚡ MEV: ", Style::default().fg(Color::Yellow)),
            Span::styled(format_number(ps.mev_txn_count.load(Ordering::Relaxed)), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("🥩 Staking: ", Style::default().fg(Color::Magenta)),
            Span::styled(format_number(ps.staking_txn_count.load(Ordering::Relaxed)), Style::default().fg(Color::White)),
        ]),
    ];

    let block = Block::default()
        .title(" Category Breakdown ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(Paragraph::new(text).block(block), chunks[1]);
}

// ============================================================================
// Tab 4: Leaders
// ============================================================================

fn draw_leaders_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let leaders = state.leader_tracker.get_top_leaders(30);
    
    let header = Row::new(vec![
        Cell::from("Leader").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Slots").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Skip %").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Total Txns").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Avg Latency").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = leaders.iter().map(|l| {
        let skip_color = if l.skip_rate() < 5.0 { Color::Green } 
            else if l.skip_rate() < 15.0 { Color::Yellow } 
            else { Color::Red };
        
        Row::new(vec![
            Cell::from(truncate_pubkey(&l.leader.to_string())).style(Style::default().fg(Color::White)),
            Cell::from(format_number(l.slots_seen)).style(Style::default().fg(Color::Cyan)),
            Cell::from(format!("{:.1}%", l.skip_rate())).style(Style::default().fg(skip_color)),
            Cell::from(format_number(l.total_txns)).style(Style::default().fg(Color::Magenta)),
            Cell::from(format!("{:.2}ms", l.avg_latency_ms)).style(Style::default().fg(Color::Yellow)),
        ])
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(12),
    ])
    .header(header)
    .block(Block::default().title(" Leader Performance ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(table, area);
}

// ============================================================================
// Tab 5: Competition
// ============================================================================

fn draw_competition_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(5)])
        .split(area);

    let competition = &state.competition_stats;

    let text = vec![
        Line::from(Span::styled("── Bundle Activity ──", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Total Bundles: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(competition.bundle_count.load(Ordering::Relaxed)), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Total Tips: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.6} SOL", competition.total_tips_sol()), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Duplicates: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(competition.duplicate_count.load(Ordering::Relaxed)), Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled("Sandwiches: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(competition.sandwich_count.load(Ordering::Relaxed)), Style::default().fg(Color::Magenta)),
        ]),
    ];

    let block = Block::default()
        .title(" Competition Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(Paragraph::new(text).block(block), chunks[0]);

    // Recent bundles
    let bundles = competition.bundles.read();
    let items: Vec<ListItem> = bundles.iter().rev().take(15).map(|b| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("Slot {}", b.slot), Style::default().fg(Color::White)),
            Span::raw(" │ "),
            Span::styled(format!("{} txns", b.txn_count), Style::default().fg(Color::Cyan)),
            Span::raw(" │ "),
            Span::styled(format!("{:.6} SOL tip", b.tip_amount as f64 / 1e9), Style::default().fg(Color::Green)),
            Span::raw(" │ "),
            Span::styled(b.timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let bundles_block = Block::default()
        .title(" Recent Bundles ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(List::new(items).block(bundles_block), chunks[1]);
}

// ============================================================================
// Tab 6: Logs
// ============================================================================

fn draw_logs_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let logs = state.logs.read();
    
    let items: Vec<ListItem> = logs.iter().rev().map(|log| {
        let level_style = match log.level {
            LogLevel::Info => Style::default().fg(Color::Cyan),
            LogLevel::Warn => Style::default().fg(Color::Yellow),
            LogLevel::Error => Style::default().fg(Color::Red),
            LogLevel::Debug => Style::default().fg(Color::Gray),
        };
        
        ListItem::new(Line::from(vec![
            Span::styled(log.timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(format!("[{}]", log.level), level_style),
            Span::raw(" "),
            Span::styled(&log.message, Style::default().fg(Color::White)),
        ]))
    }).collect();

    let block = Block::default()
        .title(" Logs ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(List::new(items).block(block), area);
}

// ============================================================================
// Tab 7: Wallet
// ============================================================================

fn draw_wallet_tab(f: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let wallet = &state.wallet_monitor;
    let wallet_addr = wallet.wallet.read();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(5)])
        .split(area);

    let wallet_str = wallet_addr.map(|w| w.to_string()).unwrap_or_else(|| "Not configured".to_string());
    let txn_count = wallet.txn_count.load(Ordering::Relaxed);
    let success = wallet.success_count.load(Ordering::Relaxed);
    let fail = wallet.fail_count.load(Ordering::Relaxed);

    let text = vec![
        Line::from(vec![
            Span::styled("Wallet: ", Style::default().fg(Color::Gray)),
            Span::styled(&wallet_str, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Transactions: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(txn_count), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Success: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(success), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Failed: ", Style::default().fg(Color::Gray)),
            Span::styled(format_number(fail), Style::default().fg(Color::Red)),
        ]),
    ];

    let block = Block::default()
        .title(" Wallet Monitor ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(Paragraph::new(text).block(block), chunks[0]);

    // Recent wallet transactions
    let txns = wallet.transactions.read();
    let items: Vec<ListItem> = txns.iter().rev().take(15).map(|t| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("Slot {}", t.slot), Style::default().fg(Color::White)),
            Span::raw(" │ "),
            Span::styled(truncate_pubkey(&t.signature), Style::default().fg(Color::Yellow)),
            Span::raw(" │ "),
            Span::styled(if t.success { "✓" } else { "✗" }, Style::default().fg(if t.success { Color::Green } else { Color::Red })),
            Span::raw(" │ "),
            Span::styled(t.timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let txns_block = Block::default()
        .title(" Recent Transactions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(List::new(items).block(txns_block), chunks[1]);
}

// ============================================================================
// Footer & Help
// ============================================================================

fn draw_footer(f: &mut Frame, _state: &Arc<AppState>, area: Rect) {
    let shortcuts = vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::styled(" Quit ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" ←/→", Style::default().fg(Color::Yellow)),
        Span::styled(" Tab ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" ↑/↓", Style::default().fg(Color::Yellow)),
        Span::styled(" Scroll ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" r", Style::default().fg(Color::Yellow)),
        Span::styled(" Reset ", Style::default().fg(Color::Gray)),
        Span::raw("│"),
        Span::styled(" ?", Style::default().fg(Color::Yellow)),
        Span::styled(" Help ", Style::default().fg(Color::Gray)),
    ];

    let footer = Paragraph::new(Line::from(shortcuts))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    f.render_widget(footer, area);
}

fn draw_help_overlay(f: &mut Frame, _state: &Arc<AppState>) {
    let area = f.area();
    
    let popup_width = 60;
    let popup_height = 18;
    let popup_area = Rect::new(
        (area.width.saturating_sub(popup_width)) / 2,
        (area.height.saturating_sub(popup_height)) / 2,
        popup_width.min(area.width),
        popup_height.min(area.height),
    );

    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled("Keyboard Shortcuts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![Span::styled("  q, Ctrl+C  ", Style::default().fg(Color::Yellow)), Span::raw("Quit")]),
        Line::from(vec![Span::styled("  ←, →, Tab  ", Style::default().fg(Color::Yellow)), Span::raw("Switch tabs")]),
        Line::from(vec![Span::styled("  ↑, ↓       ", Style::default().fg(Color::Yellow)), Span::raw("Scroll")]),
        Line::from(vec![Span::styled("  r          ", Style::default().fg(Color::Yellow)), Span::raw("Reset metrics window")]),
        Line::from(vec![Span::styled("  ?          ", Style::default().fg(Color::Yellow)), Span::raw("Toggle help")]),
        Line::from(""),
        Line::from(Span::styled("Tabs", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from("  0: Overview   1: Latency   2: Turbine"),
        Line::from("  3: Programs   4: Leaders   5: Competition"),
        Line::from("  6: Logs       7: Wallet"),
        Line::from(""),
        Line::from(Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    f.render_widget(Paragraph::new(help_text).block(block), popup_area);
}
