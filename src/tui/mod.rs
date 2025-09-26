pub mod app;
pub mod ui;
pub mod events;

pub use app::App;
pub use events::{EventHandler, InputEvent};

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;

/// Initialize the terminal for TUI mode
pub fn init() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

/// Restore the terminal to normal mode
pub fn restore() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

/// Run the TUI application
pub fn run() -> Result<()> {
    let mut terminal = init()?;

    // Create the application state
    let mut app = App::new()?;
    let event_handler = EventHandler::new(250); // 250ms tick rate

    // Main application loop
    let result = run_app(&mut terminal, &mut app, event_handler);

    // Restore terminal
    restore()?;

    result
}

/// Main application loop
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut event_handler: EventHandler,
) -> Result<()> {
    loop {
        // Draw the UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Handle events
        match event_handler.next()? {
            InputEvent::Input(event) => {
                if app.handle_input(event)? {
                    break; // Exit requested
                }
            }
            InputEvent::Tick => {
                app.tick();
            }
        }
    }

    Ok(())
}