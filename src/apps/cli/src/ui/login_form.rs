//! Full-viewport GitHub account panel (Login / Account status).
//!
//! Opened by `/login`. When already logged in, shows account info and connected devices instead of the credential form.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::ui::theme::{StyleKind, Theme};
use openbitfun_product_domains::account::{AccountDevice, AccountInfo, GitHubAuthStart};

/// Action returned after handling a key event.
#[derive(Debug, Clone)]
pub(crate) enum LoginFormAction {
    None,
    /// Close the panel (Esc on most views).
    Cancel,
    /// Start GitHub sign-in or check an existing transaction.
    Submit(Option<String>),
    /// User requested logout from the account page.
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelMode {
    Login,
    Account,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountFocus {
    Logout,
    Close,
}

/// Full-screen account panel state.
pub(crate) struct LoginFormState {
    visible: bool,
    mode: PanelMode,

    authorization: Option<GitHubAuthStart>,
    error: Option<String>,
    status: Option<String>,

    // Account status
    account_focus: AccountFocus,
    account_info: Option<AccountInfo>,
    devices: Vec<AccountDevice>,
}

impl LoginFormState {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            mode: PanelMode::Login,
            authorization: None,
            error: None,
            status: None,
            account_focus: AccountFocus::Close,
            account_info: None,
            devices: Vec::new(),
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn show(&mut self) {
        self.visible = true;
        self.mode = PanelMode::Login;
        self.authorization = None;
        self.error = None;
        self.status = None;
        self.account_focus = AccountFocus::Close;
    }

    pub(crate) fn hide(&mut self) {
        self.visible = false;
        self.error = None;
        self.status = None;
    }

    pub(crate) fn show_account(&mut self, info: AccountInfo, devices: Vec<AccountDevice>) {
        self.visible = true;
        self.mode = PanelMode::Account;
        self.account_info = Some(info);
        self.devices = devices;
        self.account_focus = AccountFocus::Close;
        self.error = None;
        self.status = None;
    }

    pub(crate) fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
        self.status = None;
    }

    pub(crate) fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
        self.error = None;
    }

    pub(crate) fn set_authorization(&mut self, authorization: GitHubAuthStart) {
        self.authorization = Some(authorization);
        self.set_status("Complete GitHub authorization, then press Enter.");
    }

    pub(crate) fn insert_paste(&mut self, _text: &str) {}

    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> LoginFormAction {
        if !self.visible {
            return LoginFormAction::None;
        }
        match self.mode {
            PanelMode::Login => self.handle_login_key(key),
            PanelMode::Account => self.handle_account_key(key),
        }
    }

    fn handle_login_key(&mut self, key: KeyEvent) -> LoginFormAction {
        match key.code {
            KeyCode::Esc => {
                self.hide();
                LoginFormAction::Cancel
            }
            KeyCode::Enter => LoginFormAction::Submit(
                self.authorization
                    .as_ref()
                    .map(|a| a.transaction_id.clone()),
            ),
            KeyCode::Char('r') => {
                self.authorization = None;
                LoginFormAction::Submit(None)
            }
            _ => LoginFormAction::None,
        }
    }

    fn handle_account_key(&mut self, key: KeyEvent) -> LoginFormAction {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.hide();
                LoginFormAction::Cancel
            }
            (KeyCode::Up, _) | (KeyCode::BackTab, _) | (KeyCode::Left, _) => {
                self.account_focus = match self.account_focus {
                    AccountFocus::Logout => AccountFocus::Close,
                    AccountFocus::Close => AccountFocus::Logout,
                };
                LoginFormAction::None
            }
            (KeyCode::Down, _) | (KeyCode::Tab, _) | (KeyCode::Right, _) => {
                self.account_focus = match self.account_focus {
                    AccountFocus::Logout => AccountFocus::Close,
                    AccountFocus::Close => AccountFocus::Logout,
                };
                LoginFormAction::None
            }
            (KeyCode::Enter, _) => match self.account_focus {
                AccountFocus::Logout => LoginFormAction::Logout,
                AccountFocus::Close => {
                    self.hide();
                    LoginFormAction::Cancel
                }
            },
            _ => LoginFormAction::None,
        }
    }

    pub(crate) fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible {
            return;
        }
        frame.render_widget(Clear, area);
        match self.mode {
            PanelMode::Login => self.render_login(frame, area, theme),
            PanelMode::Account => self.render_account(frame, area, theme),
        }
    }

    fn render_login(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .title(" OpenBitFun · GitHub Sign-in ")
            .title_alignment(Alignment::Center);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(inner);
        frame.render_widget(
            Paragraph::new("Use the same GitHub account as the OpenBitFun marketplaces.")
                .style(theme.style(StyleKind::Muted))
                .wrap(Wrap { trim: false }),
            rows[0],
        );
        let text = self
            .authorization
            .as_ref()
            .map(|a| format!("Open this link in your browser:\n\n{}", a.authorization_url))
            .unwrap_or_else(|| "Press Enter to sign in with GitHub.".to_string());
        frame.render_widget(
            Paragraph::new(text)
                .style(theme.style(StyleKind::Primary))
                .wrap(Wrap { trim: false }),
            rows[1],
        );
        self.render_message(frame, rows[2], theme);
        self.render_hints(
            frame,
            rows[3],
            "Enter Continue / Check   R Restart sign-in   Esc Close",
            theme,
        );
    }

    fn render_account(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .title(" GitHub Account ")
            .title_alignment(Alignment::Center);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // account info
                Constraint::Min(4),    // devices
                Constraint::Length(1), // buttons
                Constraint::Length(1), // hints
            ])
            .split(inner);

        let info = self.account_info.as_ref();
        let info_lines = vec![
            Line::from(vec![
                Span::styled("User: ", theme.style(StyleKind::Muted)),
                Span::styled(
                    info.map(|i| i.user_id.as_str()).unwrap_or("-"),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Auth Server: ", theme.style(StyleKind::Muted)),
                Span::styled(
                    info.map(|i| i.relay_url.as_str()).unwrap_or("-"),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("This device: ", theme.style(StyleKind::Muted)),
                Span::styled(
                    info.map(|i| i.device_name.as_str()).unwrap_or("-"),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(
                        "  ({})",
                        info.map(|i| truncate_id(&i.device_id))
                            .unwrap_or_else(|| "-".into())
                    ),
                    theme.style(StyleKind::Muted),
                ),
            ]),
        ];
        frame.render_widget(Paragraph::new(info_lines), rows[0]);

        let mut device_lines = vec![Line::from(Span::styled(
            "Devices",
            theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD),
        ))];
        if self.devices.is_empty() {
            device_lines.push(Line::from(Span::styled(
                "  (no devices listed yet)",
                theme.style(StyleKind::Muted),
            )));
        } else {
            let local_id = info.map(|i| i.device_id.as_str());
            for d in &self.devices {
                let is_local = local_id == Some(d.device_id.as_str());
                let status = if d.online { "online" } else { "offline" };
                let badge = if is_local { " [this device]" } else { "" };
                device_lines.push(Line::from(Span::styled(
                    format!(
                        "  {}{}  {}  · {}",
                        d.device_name,
                        badge,
                        truncate_id(&d.device_id),
                        status
                    ),
                    if d.online {
                        Style::default().fg(Color::White)
                    } else {
                        theme.style(StyleKind::Muted)
                    },
                )));
            }
        }
        frame.render_widget(Paragraph::new(device_lines), rows[1]);

        let btn_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[2]);
        self.render_button(
            frame,
            btn_row[0],
            "[ Logout ]",
            self.account_focus == AccountFocus::Logout,
            theme,
        );
        self.render_button(
            frame,
            btn_row[1],
            "[ Close ]",
            self.account_focus == AccountFocus::Close,
            theme,
        );
        self.render_hints(
            frame,
            rows[3],
            "Tab Switch   Enter Activate   Esc Close",
            theme,
        );
    }

    fn render_button(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        active: bool,
        theme: &Theme,
    ) {
        let style = if active {
            Style::default()
                .bg(theme.primary)
                .fg(theme.selection_foreground())
                .add_modifier(Modifier::BOLD)
        } else {
            theme.style(StyleKind::Muted)
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, style))).alignment(Alignment::Center),
            area,
        );
    }

    fn render_message(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if let Some(ref err) = self.error {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    err.as_str(),
                    theme.style(StyleKind::Error),
                )))
                .alignment(Alignment::Center),
                area,
            );
        } else if let Some(ref status) = self.status {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    status.as_str(),
                    theme.style(StyleKind::Info),
                )))
                .alignment(Alignment::Center),
                area,
            );
        }
    }

    fn render_hints(&self, frame: &mut Frame, area: Rect, hints: &str, theme: &Theme) {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hints,
                theme.style(StyleKind::Muted),
            )))
            .alignment(Alignment::Center),
            area,
        );
    }
}

fn truncate_id(id: &str) -> String {
    if id.len() <= 8 {
        id.to_string()
    } else {
        format!("{}…", &id[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    #[test]
    fn enter_starts_or_checks_the_host_owned_authorization() {
        let mut form = LoginFormState::new();
        form.show();
        assert!(matches!(
            form.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            LoginFormAction::Submit(None)
        ));
        form.set_authorization(GitHubAuthStart {
            transaction_id: "txn".into(),
            authorization_url: "https://github.com/login/oauth/authorize".into(),
            expires_at: 99,
            poll_interval_seconds: 3,
        });
        assert!(
            matches!(form.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)), LoginFormAction::Submit(Some(id)) if id == "txn")
        );
        assert!(matches!(
            form.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            LoginFormAction::Cancel
        ));
    }
}
