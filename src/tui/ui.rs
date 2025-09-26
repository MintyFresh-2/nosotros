use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Wrap,
    },
    Frame,
};

use super::app::{App, ComposeFocus, CurrentView};

/// Main UI drawing function
pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Create the main layout: top status bar, content area, bottom status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top status bar
            Constraint::Min(0),    // Main content area
            Constraint::Length(1), // Bottom status bar
        ])
        .split(size);

    // Draw top status bar
    draw_top_status_bar(f, app, chunks[0]);

    // Draw main content based on current view
    match app.current_view {
        CurrentView::Feed => draw_feed_view(f, app, chunks[1]),
        CurrentView::AccountModal => draw_account_modal(f, app, chunks[1]),
        CurrentView::ComposeModal => draw_compose_modal(f, app, chunks[1]),
        CurrentView::HelpModal => draw_help_modal(f, app, chunks[1]),
    }

    // Draw bottom status bar
    draw_bottom_status_bar(f, app, chunks[2]);

    // Draw password prompt if active
    if app.password_prompt_active {
        draw_password_prompt(f, app, size);
    }
}

/// Draw the top status bar showing current account and relay status
fn draw_top_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let account_display = app.get_current_account_display();
    let relay_status = app.get_relay_status_display();

    let top_bar = Paragraph::new(Line::from(vec![
        Span::styled("[nosotros] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("Current Account: {}", account_display),
            Style::default().fg(Color::White),
        ),
        Span::raw("  ".repeat(
            area.width.saturating_sub(
                25 + account_display.len() as u16 + relay_status.len() as u16
            ) as usize
        )),
        Span::styled(relay_status, Style::default().fg(Color::Yellow)),
    ]))
    .style(Style::default().bg(Color::DarkGray));

    f.render_widget(top_bar, area);
}

/// Draw the bottom status bar with context-sensitive shortcuts
fn draw_bottom_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match app.current_view {
        CurrentView::Feed => vec![
            ("q", "Quit"),
            ("a", "Accounts"),
            ("n", "New Post"),
            ("?", "Help"),
            ("↑↓", "Navigate"),
        ],
        CurrentView::AccountModal => {
            if app.password_prompt_active {
                vec![
                    ("Enter", "Confirm"),
                    ("Esc", "Cancel"),
                ]
            } else {
                vec![
                    ("u", "Unlock"),
                    ("l", "Lock"),
                    ("c", "Create"),
                    ("Esc", "Back"),
                ]
            }
        }
        CurrentView::ComposeModal => {
            match app.compose_focus {
                ComposeFocus::Text => vec![
                    ("Ctrl+Enter", "Post"),
                    ("Tab", "Switch Focus"),
                    ("Esc", "Cancel"),
                ],
                ComposeFocus::RelayList => vec![
                    ("Space", "Toggle"),
                    ("Tab", "Switch Focus"),
                    ("↑↓", "Navigate"),
                    ("Esc", "Cancel"),
                ],
            }
        }
        CurrentView::HelpModal => vec![
            ("Esc", "Back"),
        ],
    };

    let shortcut_spans: Vec<Span> = shortcuts
        .iter()
        .enumerate()
        .flat_map(|(i, (key, desc))| {
            let mut spans = vec![
                Span::styled(*key, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!(":{}", desc), Style::default().fg(Color::White)),
            ];

            if i < shortcuts.len() - 1 {
                spans.push(Span::raw("  "));
            }

            spans
        })
        .collect();

    let bottom_bar = Paragraph::new(Line::from(shortcut_spans))
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(bottom_bar, area);
}

/// Draw the main feed view
fn draw_feed_view(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("Feed")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    // Convert feed items to list items
    let items: Vec<ListItem> = app.feed_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let content = if i == app.selected_index {
                format!("> {}", item)
            } else {
                format!("  {}", item)
            };

            let style = if i == app.selected_index {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(block);

    f.render_widget(list, area);

    // Draw status message if present
    if let Some(ref message) = app.status_message {
        draw_status_message(f, message, area);
    }
}

/// Draw the account management modal
fn draw_account_modal(f: &mut Frame, app: &App, area: Rect) {
    // Create a centered modal
    let popup_area = centered_rect(60, 70, area);

    // Clear the background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Account Management")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Account management content
    let content = if app.keystore_unlocked {
        let accounts = app.account_manager.list_accounts();
        if accounts.is_empty() {
            vec![
                Line::from("No accounts found."),
                Line::from(""),
                Line::from("Available actions:"),
                Line::from("  c - Create new account"),
                Line::from("  i - Import existing account"),
                Line::from("  l - Lock keystore"),
            ]
        } else {
            let mut lines = vec![
                Line::from("Accounts:"),
                Line::from(""),
            ];

            for account in accounts {
                let status = if account.is_active { " (active)" } else { "" };
                lines.push(Line::from(format!("  {} - {}{}",
                    account.name,
                    &account.public_key_npub[..16],
                    status
                )));
            }

            lines.extend(vec![
                Line::from(""),
                Line::from("Available actions:"),
                Line::from("  c - Create new account"),
                Line::from("  i - Import existing account"),
                Line::from("  l - Lock keystore"),
            ]);

            lines
        }
    } else {
        vec![
            Line::from("Keystore is locked."),
            Line::from(""),
            Line::from("Available actions:"),
            Line::from("  u - Unlock keystore"),
        ]
    };

    let paragraph = Paragraph::new(content)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);

    // Draw status message if present
    if let Some(ref message) = app.status_message {
        draw_status_message(f, message, popup_area);
    }
}

/// Draw the compose post modal
fn draw_compose_modal(f: &mut Frame, app: &App, area: Rect) {
    // Create a large centered modal
    let popup_area = centered_rect(80, 80, area);

    // Clear the background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Compose Post")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Split into text area and relay selection
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(inner);

    // Text input area
    let text_block = Block::default()
        .title("Message")
        .borders(Borders::ALL)
        .border_style(if app.compose_focus == ComposeFocus::Text {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        });

    let text_content = if app.compose_text.is_empty() {
        "Enter your message here..."
    } else {
        &app.compose_text
    };

    let text_paragraph = Paragraph::new(text_content)
        .block(text_block)
        .wrap(Wrap { trim: true })
        .style(if app.compose_text.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        });

    f.render_widget(text_paragraph, chunks[0]);

    // Relay selection area
    let relay_block = Block::default()
        .title("Relays")
        .borders(Borders::ALL)
        .border_style(if app.compose_focus == ComposeFocus::RelayList {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        });

    let relay_items: Vec<ListItem> = app.compose_relay_selection
        .iter()
        .enumerate()
        .map(|(i, (relay, selected))| {
            let checkbox = if *selected { "☑" } else { "☐" };
            let content = format!("{} {}", checkbox, relay);

            let style = if app.compose_focus == ComposeFocus::RelayList && i == app.selected_index {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let relay_list = List::new(relay_items).block(relay_block);

    f.render_widget(relay_list, chunks[1]);

    // Character count
    let char_count_area = Rect {
        x: chunks[0].x,
        y: chunks[0].bottom().saturating_sub(1),
        width: chunks[0].width,
        height: 1,
    };

    let char_count = format!("{} chars", app.compose_text.len());
    let char_count_widget = Paragraph::new(char_count)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Right);

    f.render_widget(char_count_widget, char_count_area);
}

/// Draw the help modal
fn draw_help_modal(f: &mut Frame, _app: &App, area: Rect) {
    // Create a centered modal
    let popup_area = centered_rect(70, 80, area);

    // Clear the background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Help - Keyboard Shortcuts")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let help_text = vec![
        Line::from(Span::styled("Global Shortcuts", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  q                 - Quit application"),
        Line::from("  Ctrl+C            - Force quit"),
        Line::from("  a                 - Open account management"),
        Line::from("  n                 - Compose new post"),
        Line::from("  r                 - Refresh current view"),
        Line::from("  ?                 - Show this help"),
        Line::from("  Esc               - Return to feed / Cancel"),
        Line::from(""),
        Line::from(Span::styled("Feed Navigation", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  ↑/k               - Move selection up"),
        Line::from("  ↓/j               - Move selection down"),
        Line::from("  Home/g            - Jump to top"),
        Line::from("  End/G             - Jump to bottom"),
        Line::from("  Enter             - Expand/interact with post"),
        Line::from(""),
        Line::from(Span::styled("Account Management", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  u                 - Unlock keystore"),
        Line::from("  l                 - Lock keystore"),
        Line::from("  c                 - Create new account"),
        Line::from("  i                 - Import existing account"),
        Line::from("  Enter             - Select account"),
        Line::from(""),
        Line::from(Span::styled("Compose Post", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  Tab               - Switch between text/relays"),
        Line::from("  Ctrl+Enter        - Publish post"),
        Line::from("  Enter (in text)   - New line"),
        Line::from("  Space (in relays) - Toggle relay selection"),
        Line::from(""),
        Line::from(Span::styled("Press Esc to close this help", Style::default().fg(Color::Green).add_modifier(Modifier::ITALIC))),
    ];

    let paragraph = Paragraph::new(help_text)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Draw a password input prompt
fn draw_password_prompt(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 20, area);

    // Clear the background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Enter Password")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Show asterisks for password
    let password_display = "*".repeat(app.password_input.len());
    let content = vec![
        Line::from("Password:"),
        Line::from(""),
        Line::from(password_display),
        Line::from(""),
        Line::from("Press Enter to confirm, Esc to cancel"),
    ];

    let paragraph = Paragraph::new(content)
        .alignment(Alignment::Center);

    f.render_widget(paragraph, inner);
}

/// Draw a status message overlay
fn draw_status_message(f: &mut Frame, message: &str, area: Rect) {
    let popup_area = Rect {
        x: area.x + 2,
        y: area.bottom().saturating_sub(3),
        width: area.width.saturating_sub(4).min(message.len() as u16 + 4),
        height: 3,
    };

    // Clear the background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, inner);
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}