use anyhow::Result;
use crossterm::event::{self, Event, KeyEvent};
use std::time::{Duration, Instant};

/// Events that can occur in the application
#[derive(Debug)]
pub enum InputEvent {
    /// User input event (keyboard)
    Input(KeyEvent),
    /// Periodic tick for updating UI
    Tick,
}

/// Handles terminal events and provides a unified event stream
pub struct EventHandler {
    /// Duration between tick events
    tick_rate: Duration,
    /// Last time a tick event was sent
    last_tick: Instant,
}

impl EventHandler {
    /// Create a new event handler with the specified tick rate in milliseconds
    pub fn new(tick_rate: u64) -> Self {
        Self {
            tick_rate: Duration::from_millis(tick_rate),
            last_tick: Instant::now(),
        }
    }

    /// Get the next event, blocking until one is available
    pub fn next(&mut self) -> Result<InputEvent> {
        let timeout = self.tick_rate
            .checked_sub(self.last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    return Ok(InputEvent::Input(key));
                }
                // Ignore other events (mouse, resize, etc.) for now
                _ => {}
            }
        }

        // Check if we should send a tick
        if self.last_tick.elapsed() >= self.tick_rate {
            self.last_tick = Instant::now();
            return Ok(InputEvent::Tick);
        }

        // If we get here, poll returned false and it's not time for a tick yet
        // This should rarely happen, but we'll just wait a bit and try again
        std::thread::sleep(Duration::from_millis(10));
        self.next()
    }
}