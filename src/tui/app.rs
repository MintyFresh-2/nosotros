use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

use crate::accounts::AccountManager;

/// Current view/screen in the application
#[derive(Debug, Clone, PartialEq)]
pub enum CurrentView {
    Feed,
    AccountModal,
    ComposeModal,
    HelpModal,
}

/// Application state and logic
pub struct App {
    /// Current view being displayed
    pub current_view: CurrentView,

    /// Whether the application should exit
    pub should_quit: bool,

    /// Account manager for handling user accounts and secure storage
    pub account_manager: AccountManager,

    /// Whether the keystore is currently unlocked
    pub keystore_unlocked: bool,

    /// Current password input (when prompting for unlock)
    pub password_input: String,

    /// Whether we're currently showing a password input prompt
    pub password_prompt_active: bool,

    /// Status message to display to user
    pub status_message: Option<String>,

    /// Feed content (placeholder for now)
    pub feed_items: Vec<String>,

    /// Selected item index in current view
    pub selected_index: usize,

    /// Compose modal state
    pub compose_text: String,
    pub compose_relay_selection: Vec<(String, bool)>, // (relay_url, selected)
    pub compose_focus: ComposeFocus,
}

/// Focus state within the compose modal
#[derive(Debug, Clone, PartialEq)]
pub enum ComposeFocus {
    Text,
    RelayList,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Result<Self> {
        // Get config directory (create if doesn't exist)
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosotros");

        let account_manager = AccountManager::new(config_dir)?;

        Ok(Self {
            current_view: CurrentView::Feed,
            should_quit: false,
            account_manager,
            keystore_unlocked: false,
            password_input: String::new(),
            password_prompt_active: false,
            status_message: Some("Welcome to Nosotros! Press 'a' to manage accounts, '?' for help".to_string()),
            feed_items: vec![
                "No posts yet. Follow accounts or check relays.".to_string(),
            ],
            selected_index: 0,
            compose_text: String::new(),
            compose_relay_selection: vec![
                ("wss://relay.damus.io".to_string(), true),
                ("wss://nos.lol".to_string(), true),
                ("wss://relay.snort.social".to_string(), false),
            ],
            compose_focus: ComposeFocus::Text,
        })
    }

    /// Handle keyboard input events
    pub fn handle_input(&mut self, key: KeyEvent) -> Result<bool> {
        // Handle global shortcuts first
        if self.handle_global_shortcuts(key)? {
            return Ok(true); // Exit requested
        }

        // Handle view-specific input
        match self.current_view {
            CurrentView::Feed => self.handle_feed_input(key)?,
            CurrentView::AccountModal => self.handle_account_modal_input(key)?,
            CurrentView::ComposeModal => self.handle_compose_modal_input(key)?,
            CurrentView::HelpModal => self.handle_help_modal_input(key)?,
        }

        Ok(false)
    }

    /// Handle global keyboard shortcuts available from any view
    fn handle_global_shortcuts(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return Ok(true);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return Ok(true);
            }
            KeyCode::Char('?') => {
                self.current_view = CurrentView::HelpModal;
            }
            KeyCode::Char('a') => {
                self.current_view = CurrentView::AccountModal;
            }
            KeyCode::Char('n') => {
                if self.keystore_unlocked {
                    self.current_view = CurrentView::ComposeModal;
                    self.compose_text.clear();
                    self.compose_focus = ComposeFocus::Text;
                } else {
                    self.status_message = Some("Please unlock accounts first (press 'a')".to_string());
                }
            }
            KeyCode::Char('r') => {
                self.refresh_view();
            }
            KeyCode::Esc => {
                // Return to feed from any modal
                if self.current_view != CurrentView::Feed {
                    self.current_view = CurrentView::Feed;
                    self.password_prompt_active = false;
                    self.password_input.clear();
                }
            }
            _ => return Ok(false), // Not a global shortcut
        }

        Ok(false)
    }

    /// Handle input when in feed view
    fn handle_feed_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index < self.feed_items.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected_index = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.selected_index = self.feed_items.len().saturating_sub(1);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle input when in account modal
    fn handle_account_modal_input(&mut self, key: KeyEvent) -> Result<()> {
        if self.password_prompt_active {
            self.handle_password_input(key)?;
        } else {
            match key.code {
                KeyCode::Char('u') => {
                    // Unlock keystore
                    self.password_prompt_active = true;
                    self.password_input.clear();
                    self.status_message = Some("Enter password to unlock keystore:".to_string());
                }
                KeyCode::Char('l') => {
                    // Lock keystore
                    self.account_manager.lock_keystore();
                    self.keystore_unlocked = false;
                    self.status_message = Some("Keystore locked".to_string());
                }
                KeyCode::Char('c') => {
                    if self.keystore_unlocked {
                        self.status_message = Some("Create account feature coming soon!".to_string());
                    } else {
                        self.status_message = Some("Please unlock keystore first".to_string());
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle password input
    fn handle_password_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                // Try to unlock with the entered password
                let password = secrecy::SecretString::new(self.password_input.clone().into_boxed_str());
                match self.account_manager.unlock_keystore(&password) {
                    Ok(()) => {
                        self.keystore_unlocked = true;
                        self.password_prompt_active = false;
                        self.password_input.clear();
                        self.status_message = Some("Keystore unlocked successfully!".to_string());
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to unlock: {}", e));
                        self.password_input.clear();
                    }
                }
            }
            KeyCode::Char(c) => {
                self.password_input.push(c);
            }
            KeyCode::Backspace => {
                self.password_input.pop();
            }
            KeyCode::Esc => {
                self.password_prompt_active = false;
                self.password_input.clear();
                self.status_message = Some("Password entry cancelled".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle input when in compose modal
    fn handle_compose_modal_input(&mut self, key: KeyEvent) -> Result<()> {
        match self.compose_focus {
            ComposeFocus::Text => {
                match key.code {
                    KeyCode::Tab => {
                        self.compose_focus = ComposeFocus::RelayList;
                    }
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.publish_post()?;
                    }
                    KeyCode::Enter => {
                        self.compose_text.push('\n');
                    }
                    KeyCode::Char(c) => {
                        self.compose_text.push(c);
                    }
                    KeyCode::Backspace => {
                        self.compose_text.pop();
                    }
                    _ => {}
                }
            }
            ComposeFocus::RelayList => {
                match key.code {
                    KeyCode::Tab => {
                        self.compose_focus = ComposeFocus::Text;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.selected_index > 0 {
                            self.selected_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.selected_index < self.compose_relay_selection.len().saturating_sub(1) {
                            self.selected_index += 1;
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        // Toggle selected relay
                        if let Some((_, selected)) = self.compose_relay_selection.get_mut(self.selected_index) {
                            *selected = !*selected;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Handle input when in help modal
    fn handle_help_modal_input(&mut self, _key: KeyEvent) -> Result<()> {
        // Help modal just shows information, Esc handled by global shortcuts
        Ok(())
    }

    /// Publish the composed post
    fn publish_post(&mut self) -> Result<()> {
        if self.compose_text.trim().is_empty() {
            self.status_message = Some("Cannot post empty message".to_string());
            return Ok(());
        }

        // Get selected relays
        let selected_relays: Vec<String> = self.compose_relay_selection
            .iter()
            .filter(|(_, selected)| *selected)
            .map(|(url, _)| url.clone())
            .collect();

        if selected_relays.is_empty() {
            self.status_message = Some("Please select at least one relay".to_string());
            return Ok(());
        }

        // For now, just show a placeholder message
        self.status_message = Some(format!(
            "Publishing to {} relays: {}",
            selected_relays.len(),
            selected_relays.join(", ")
        ));

        // TODO: Integrate with actual posting functionality
        // Clear compose modal and return to feed
        self.compose_text.clear();
        self.current_view = CurrentView::Feed;

        Ok(())
    }

    /// Refresh the current view
    fn refresh_view(&mut self) {
        match self.current_view {
            CurrentView::Feed => {
                self.status_message = Some("Feed refreshed".to_string());
                // TODO: Refresh feed content
            }
            _ => {
                self.status_message = Some("Refreshed".to_string());
            }
        }
    }

    /// Update application state (called on tick)
    pub fn tick(&mut self) {
        // Clear status message after some time
        // TODO: Implement proper status message timeout
    }

    /// Get current account information for display
    pub fn get_current_account_display(&self) -> String {
        if !self.keystore_unlocked {
            return "No account (locked)".to_string();
        }

        match self.account_manager.get_active_account() {
            Ok(Some(account)) => {
                let npub = &account.info.public_key_npub;
                let short_npub = if npub.len() > 16 {
                    format!("{}...", &npub[..16])
                } else {
                    npub.clone()
                };
                format!("{} ({})", account.info.name, short_npub)
            }
            Ok(None) => "No active account".to_string(),
            Err(_) => "Error loading account".to_string(),
        }
    }

    /// Get relay connection status for display
    pub fn get_relay_status_display(&self) -> String {
        // TODO: Implement actual relay status checking
        "🟡 0 relays".to_string()
    }
}