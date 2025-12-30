mod client;
mod events;
mod state;
mod ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use crate::client::{start_client, ClientMessage};
use crate::events::{poll_event, InputEvent};
use crate::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "shredstream-tui")]
#[command(author = "ShredStream TUI")]
#[command(version = "0.1.0")]
#[command(about = "Terminal UI for monitoring Jito ShredStream proxy", long_about = None)]
struct Args {
    /// gRPC endpoint for the ShredStream proxy
    /// Example: http://127.0.0.1:50051
    #[arg(short, long, env = "SHREDSTREAM_PROXY_URL", default_value = "http://127.0.0.1:50051")]
    proxy_url: String,

    /// Tick rate in milliseconds for UI refresh
    #[arg(short, long, default_value = "100")]
    tick_rate: u64,

    /// Metrics window duration in seconds (how often to reset rate calculations)
    #[arg(short, long, default_value = "10")]
    metrics_window: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize tracing for debug logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    // Create application state
    let state = Arc::new(AppState::new(args.proxy_url.clone()));
    state.log_info("ShredStream TUI starting...");
    state.log_info(format!("Connecting to proxy at {}", args.proxy_url));

    // Create channel for client messages
    let (client_tx, mut client_rx) = mpsc::channel::<ClientMessage>(1000);

    // Start the gRPC client in background
    let client_state = Arc::clone(&state);
    let _client_handle = start_client(args.proxy_url.clone(), client_state, client_tx);

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Run the main event loop
    let result = run_app(&mut terminal, state, &mut client_rx, &args).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<AppState>,
    client_rx: &mut mpsc::Receiver<ClientMessage>,
    args: &Args,
) -> Result<()> {
    let tick_duration = Duration::from_millis(args.tick_rate);
    let metrics_window_duration = Duration::from_secs(args.metrics_window);
    let mut last_metrics_reset = std::time::Instant::now();

    loop {
        // Draw the UI
        terminal.draw(|f| ui::draw(f, &state))?;

        // Process any pending client messages (non-blocking)
        while let Ok(msg) = client_rx.try_recv() {
            match msg {
                ClientMessage::EntriesReceived { slot: _, entries: _ } => {
                    // Entries are already processed in the client
                    // We could add additional processing here if needed
                }
                ClientMessage::ConnectionChanged(conn_state) => {
                    state.set_connection_state(conn_state);
                }
                ClientMessage::Error(e) => {
                    state.log_error(format!("Client error: {}", e));
                }
            }
        }

        // Handle input events
        if let Some(event) = poll_event(tick_duration) {
            let show_help = *state.show_help.read();
            
            match event {
                InputEvent::Quit => {
                    state.log_info("Shutting down...");
                    break;
                }
                InputEvent::CloseOverlay if show_help => {
                    state.toggle_help();
                }
                InputEvent::ToggleHelp => {
                    state.toggle_help();
                }
                InputEvent::NextTab if !show_help => {
                    state.next_tab();
                }
                InputEvent::PrevTab if !show_help => {
                    state.prev_tab();
                }
                InputEvent::ScrollUp if !show_help => {
                    state.scroll_up();
                }
                InputEvent::ScrollDown if !show_help => {
                    state.scroll_down();
                }
                InputEvent::ResetMetrics if !show_help => {
                    state.reset_metrics_window();
                    state.log_info("Metrics window reset");
                }
                InputEvent::Tick => {
                    // Regular tick - check if we need to reset metrics window
                    if last_metrics_reset.elapsed() >= metrics_window_duration {
                        // Don't reset cumulative, just the window metrics for rate calc
                        // The state already handles this internally
                        last_metrics_reset = std::time::Instant::now();
                    }
                }
                _ => {
                    // Close help on any key if showing
                    if show_help {
                        state.toggle_help();
                    }
                }
            }
        }
    }

    Ok(())
}
