use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

/// Input events for the application
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Quit the application
    Quit,
    /// Navigate to next tab
    NextTab,
    /// Navigate to previous tab
    PrevTab,
    /// Scroll up
    ScrollUp,
    /// Scroll down
    ScrollDown,
    /// Reset metrics window
    ResetMetrics,
    /// Toggle help display
    ToggleHelp,
    /// Close help/overlay
    CloseOverlay,
    /// No input (tick)
    Tick,
}

/// Poll for input events with a timeout
pub fn poll_event(timeout: Duration) -> Option<InputEvent> {
    if event::poll(timeout).ok()? {
        if let Event::Key(key) = event::read().ok()? {
            // Only handle key press events (not release)
            if key.kind != KeyEventKind::Press {
                return None;
            }

            return Some(match key.code {
                // Quit
                KeyCode::Char('q') => InputEvent::Quit,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    InputEvent::Quit
                }
                KeyCode::Esc => InputEvent::CloseOverlay,

                // Tab navigation
                KeyCode::Tab => InputEvent::NextTab,
                KeyCode::BackTab => InputEvent::PrevTab,
                KeyCode::Right | KeyCode::Char('l') => InputEvent::NextTab,
                KeyCode::Left | KeyCode::Char('h') => InputEvent::PrevTab,

                // Scrolling
                KeyCode::Up | KeyCode::Char('k') => InputEvent::ScrollUp,
                KeyCode::Down | KeyCode::Char('j') => InputEvent::ScrollDown,
                KeyCode::PageUp => InputEvent::ScrollUp,
                KeyCode::PageDown => InputEvent::ScrollDown,

                // Actions
                KeyCode::Char('r') => InputEvent::ResetMetrics,
                KeyCode::Char('?') => InputEvent::ToggleHelp,

                _ => return None,
            });
        }
    }
    
    Some(InputEvent::Tick)
}
