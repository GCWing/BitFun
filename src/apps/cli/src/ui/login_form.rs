//! Full-viewport BitFun account panel (Login / Sync choice / Account status).
//!
//! Opened by `/login`. When already logged in, shows account info and sync
//! progress instead of the credential form.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::account::{AccountDevice, AccountInfo};
use crate::account_sync::{sync_phase_label, SyncProgress, SyncStatus};
use crate::ui::theme::{StyleKind, Theme};

/// Credentials collected by the login form.
#[derive(Debug, Clone)]
pub(crate) struct LoginCredentials {
    pub relay_url: String,
    pub username: String,
    pub password: String,
}

/// Action returned after handling a key event.
#[derive(Debug, Clone)]
pub(crate) enum LoginFormAction {
    None,
    /// Close the panel (Esc on most views).
    Cancel,
    /// Submit login credentials.
    Submit(LoginCredentials),
    /// User chose "Use local" on the sync conflict page.
    SyncUseLocal,
    /// User chose "Use cloud" on the sync conflict page.
    SyncUseCloud,
    /// User cancelled the sync choice (logout + back to login).
    SyncCancel,
    /// User requested logout from the account page.
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelMode {
    Login,
    SyncChoice,
    Account,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginFocus {
    AuthServer,
    Username,
    Password,
    Login,
}

const LOGIN_FOCUS_ORDER: [LoginFocus; 4] = [
    LoginFocus::AuthServer,
    LoginFocus::Username,
    LoginFocus::Password,
    LoginFocus::Login,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncChoiceFocus {
    UseLocal,
    UseCloud,
    Cancel,
}

const SYNC_CHOICE_ORDER: [SyncChoiceFocus; 3] = [
    SyncChoiceFocus::UseLocal,
    SyncChoiceFocus::UseCloud,
    SyncChoiceFocus::Cancel,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountFocus {
    Logout,
    Close,
}

/// Full-screen account panel state.
pub(crate) struct LoginFormState {
    visible: bool,
    mode: PanelMode,

    // Login fields
    auth_server: String,
    username: String,
    password: String,
    login_focus: LoginFocus,
    cursor: usize,
    error: Option<String>,
    status: Option<String>,

    // Sync choice
    sync_choice_focus: SyncChoiceFocus,
    pending_user_id: String,
    pending_relay: String,

    // Account status
    account_focus: AccountFocus,
    account_info: Option<AccountInfo>,
    devices: Vec<AccountDevice>,
    sync_progress: SyncProgress,
}

impl LoginFormState {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            mode: PanelMode::Login,
            auth_server: String::new(),
            username: String::new(),
            password: String::new(),
            login_focus: LoginFocus::AuthServer,
            cursor: 0,
            error: None,
            status: None,
            sync_choice_focus: SyncChoiceFocus::UseLocal,
            pending_user_id: String::new(),
            pending_relay: String::new(),
            account_focus: AccountFocus::Close,
            account_info: None,
            devices: Vec::new(),
            sync_progress: SyncProgress::default(),
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn show(&mut self) {
        self.visible = true;
        self.mode = PanelMode::Login;
        self.auth_server.clear();
        self.username.clear();
        self.password.clear();
        self.login_focus = LoginFocus::AuthServer;
        self.cursor = 0;
        self.error = None;
        self.status = None;
        self.sync_choice_focus = SyncChoiceFocus::UseLocal;
        self.account_focus = AccountFocus::Close;
    }

    pub(crate) fn hide(&mut self) {
        self.visible = false;
        self.error = None;
        self.status = None;
    }

    pub(crate) fn show_sync_choice(&mut self, user_id: &str, relay_url: &str) {
        self.visible = true;
        self.mode = PanelMode::SyncChoice;
        self.pending_user_id = user_id.to_string();
        self.pending_relay = relay_url.to_string();
        self.sync_choice_focus = SyncChoiceFocus::UseLocal;
        self.error = None;
        self.status = None;
    }

    pub(crate) fn show_account(
        &mut self,
        info: AccountInfo,
        devices: Vec<AccountDevice>,
        sync_progress: SyncProgress,
    ) {
        self.visible = true;
        self.mode = PanelMode::Account;
        self.account_info = Some(info);
        self.devices = devices;
        self.sync_progress = sync_progress;
        self.account_focus = AccountFocus::Close;
        self.error = None;
        self.status = None;
    }

    pub(crate) fn update_account_progress(
        &mut self,
        devices: Option<Vec<AccountDevice>>,
        sync_progress: SyncProgress,
    ) {
        if let Some(devices) = devices {
            self.devices = devices;
        }
        self.sync_progress = sync_progress;
    }

    pub(crate) fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
        self.status = None;
        if self.mode == PanelMode::Login {
            self.login_focus = LoginFocus::Login;
        }
    }

    pub(crate) fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
        self.error = None;
    }

    pub(crate) fn insert_paste(&mut self, text: &str) {
        if !self.visible || self.mode != PanelMode::Login || self.login_focus == LoginFocus::Login {
            return;
        }
        let cleaned: String = text
            .chars()
            .filter(|c| *c != '\n' && *c != '\r' && *c != '\t')
            .collect();
        if cleaned.is_empty() {
            return;
        }
        let cursor = self.cursor;
        if let Some(buf) = self.active_buffer_mut() {
            let byte = char_to_byte(buf, cursor);
            buf.insert_str(byte, &cleaned);
            self.cursor = cursor + cleaned.chars().count();
        }
        self.error = None;
    }

    fn active_buffer(&self) -> &str {
        match self.login_focus {
            LoginFocus::AuthServer => &self.auth_server,
            LoginFocus::Username => &self.username,
            LoginFocus::Password => &self.password,
            LoginFocus::Login => "",
        }
    }

    fn active_buffer_mut(&mut self) -> Option<&mut String> {
        match self.login_focus {
            LoginFocus::AuthServer => Some(&mut self.auth_server),
            LoginFocus::Username => Some(&mut self.username),
            LoginFocus::Password => Some(&mut self.password),
            LoginFocus::Login => None,
        }
    }

    fn move_login_focus(&mut self, delta: isize) {
        let len = LOGIN_FOCUS_ORDER.len() as isize;
        let idx = LOGIN_FOCUS_ORDER
            .iter()
            .position(|f| *f == self.login_focus)
            .unwrap_or(0) as isize;
        let next = (idx + delta).rem_euclid(len) as usize;
        self.login_focus = LOGIN_FOCUS_ORDER[next];
        self.cursor = self.active_buffer().chars().count();
    }

    fn move_sync_focus(&mut self, delta: isize) {
        let len = SYNC_CHOICE_ORDER.len() as isize;
        let idx = SYNC_CHOICE_ORDER
            .iter()
            .position(|f| *f == self.sync_choice_focus)
            .unwrap_or(0) as isize;
        let next = (idx + delta).rem_euclid(len) as usize;
        self.sync_choice_focus = SYNC_CHOICE_ORDER[next];
    }

    fn validate_login(&self) -> Option<String> {
        if self.auth_server.trim().is_empty() {
            return Some("Auth Server is required".into());
        }
        if self.username.trim().is_empty() {
            return Some("Username is required".into());
        }
        if self.password.is_empty() {
            return Some("Password is required".into());
        }
        None
    }

    fn try_submit_login(&mut self) -> LoginFormAction {
        if let Some(err) = self.validate_login() {
            self.set_error(err);
            return LoginFormAction::None;
        }
        self.set_status("Logging in...");
        LoginFormAction::Submit(LoginCredentials {
            relay_url: self.auth_server.trim().to_string(),
            username: self.username.trim().to_string(),
            password: self.password.clone(),
        })
    }

    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> LoginFormAction {
        if !self.visible {
            return LoginFormAction::None;
        }
        match self.mode {
            PanelMode::Login => self.handle_login_key(key),
            PanelMode::SyncChoice => self.handle_sync_choice_key(key),
            PanelMode::Account => self.handle_account_key(key),
        }
    }

    fn handle_login_key(&mut self, key: KeyEvent) -> LoginFormAction {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.hide();
                LoginFormAction::Cancel
            }
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        self.insert_paste(&text);
                    }
                }
                LoginFormAction::None
            }
            (KeyCode::Up, _) | (KeyCode::BackTab, _) => {
                self.move_login_focus(-1);
                LoginFormAction::None
            }
            (KeyCode::Down, _) | (KeyCode::Tab, _) => {
                self.move_login_focus(1);
                LoginFormAction::None
            }
            (KeyCode::Enter, _) => match self.login_focus {
                LoginFocus::Login => self.try_submit_login(),
                _ => {
                    self.move_login_focus(1);
                    LoginFormAction::None
                }
            },
            (KeyCode::Left, _) if self.login_focus != LoginFocus::Login => {
                self.cursor = self.cursor.saturating_sub(1);
                LoginFormAction::None
            }
            (KeyCode::Right, _) if self.login_focus != LoginFocus::Login => {
                let len = self.active_buffer().chars().count();
                if self.cursor < len {
                    self.cursor += 1;
                }
                LoginFormAction::None
            }
            (KeyCode::Home, _) if self.login_focus != LoginFocus::Login => {
                self.cursor = 0;
                LoginFormAction::None
            }
            (KeyCode::End, _) if self.login_focus != LoginFocus::Login => {
                self.cursor = self.active_buffer().chars().count();
                LoginFormAction::None
            }
            (KeyCode::Backspace, _) => {
                let cursor = self.cursor;
                if let Some(buf) = self.active_buffer_mut() {
                    if cursor > 0 {
                        let byte = char_to_byte(buf, cursor - 1);
                        let end = char_to_byte(buf, cursor);
                        buf.replace_range(byte..end, "");
                        self.cursor = cursor - 1;
                    }
                }
                LoginFormAction::None
            }
            (KeyCode::Delete, _) => {
                let cursor = self.cursor;
                if let Some(buf) = self.active_buffer_mut() {
                    let len = buf.chars().count();
                    if cursor < len {
                        let start = char_to_byte(buf, cursor);
                        let end = char_to_byte(buf, cursor + 1);
                        buf.replace_range(start..end, "");
                    }
                }
                LoginFormAction::None
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT)
                if self.login_focus != LoginFocus::Login && !c.is_control() =>
            {
                let cursor = self.cursor;
                if let Some(buf) = self.active_buffer_mut() {
                    let byte = char_to_byte(buf, cursor);
                    buf.insert(byte, c);
                    self.cursor = cursor + 1;
                }
                LoginFormAction::None
            }
            _ => LoginFormAction::None,
        }
    }

    fn handle_sync_choice_key(&mut self, key: KeyEvent) -> LoginFormAction {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => LoginFormAction::SyncCancel,
            (KeyCode::Up, _) | (KeyCode::BackTab, _) => {
                self.move_sync_focus(-1);
                LoginFormAction::None
            }
            (KeyCode::Down, _) | (KeyCode::Tab, _) => {
                self.move_sync_focus(1);
                LoginFormAction::None
            }
            (KeyCode::Enter, _) => match self.sync_choice_focus {
                SyncChoiceFocus::UseLocal => LoginFormAction::SyncUseLocal,
                SyncChoiceFocus::UseCloud => LoginFormAction::SyncUseCloud,
                SyncChoiceFocus::Cancel => LoginFormAction::SyncCancel,
            },
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
            PanelMode::SyncChoice => self.render_sync_choice(frame, area, theme),
            PanelMode::Account => self.render_account(frame, area, theme),
        }
    }

    fn render_login(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .title(" BitFun Account Login ")
            .title_alignment(Alignment::Center);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let form_width = inner.width.min(72).max(40);
        let form_height = 15u16.min(inner.height.max(12));
        let form_area = Rect {
            x: inner.x + (inner.width.saturating_sub(form_width)) / 2,
            y: inner.y + (inner.height.saturating_sub(form_height)) / 2,
            width: form_width,
            height: form_height,
        };
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(form_area);

        self.render_field_label(
            frame,
            rows[0],
            "Auth Server",
            self.login_focus == LoginFocus::AuthServer,
            theme,
        );
        self.render_text_input(frame, rows[1], LoginFocus::AuthServer, theme);
        self.render_field_label(
            frame,
            rows[3],
            "Username",
            self.login_focus == LoginFocus::Username,
            theme,
        );
        self.render_text_input(frame, rows[4], LoginFocus::Username, theme);
        self.render_field_label(
            frame,
            rows[6],
            "Password",
            self.login_focus == LoginFocus::Password,
            theme,
        );
        self.render_text_input(frame, rows[7], LoginFocus::Password, theme);
        self.render_button(
            frame,
            rows[9],
            "[  Login  ]",
            self.login_focus == LoginFocus::Login,
            theme,
        );
        self.render_message(frame, rows[11], theme);
        self.render_hints(
            frame,
            rows[12],
            "Up/Down Select   Enter Next / Submit   Esc Cancel",
            theme,
        );
    }

    fn render_sync_choice(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .title(" Cloud Settings Found ")
            .title_alignment(Alignment::Center);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let form_width = inner.width.min(76).max(40);
        let form_height = 14u16.min(inner.height.max(10));
        let form_area = Rect {
            x: inner.x + (inner.width.saturating_sub(form_width)) / 2,
            y: inner.y + (inner.height.saturating_sub(form_height)) / 2,
            width: form_width,
            height: form_height,
        };
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(form_area);

        let notice = format!(
            "Account {} already has cloud settings on {}.\nChoose how to sync this device.",
            self.pending_user_id, self.pending_relay
        );
        frame.render_widget(
            Paragraph::new(notice).style(theme.style(StyleKind::Muted)),
            rows[0],
        );

        self.render_choice_row(
            frame,
            rows[2],
            "Use local",
            "Keep this device settings and upload them to cloud",
            self.sync_choice_focus == SyncChoiceFocus::UseLocal,
            theme,
        );
        self.render_choice_row(
            frame,
            rows[4],
            "Use cloud",
            "Download cloud settings and overwrite this device",
            self.sync_choice_focus == SyncChoiceFocus::UseCloud,
            theme,
        );
        self.render_button(
            frame,
            rows[6],
            "[  Cancel / Logout  ]",
            self.sync_choice_focus == SyncChoiceFocus::Cancel,
            theme,
        );
        self.render_hints(
            frame,
            rows[9],
            "Up/Down Select   Enter Confirm   Esc Cancel",
            theme,
        );
    }

    fn render_account(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .title(" BitFun Account ")
            .title_alignment(Alignment::Center);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // account info
                Constraint::Length(3), // sync progress
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

        let sync = &self.sync_progress;
        let sync_text = match sync.status {
            SyncStatus::Idle => "Sync: idle".to_string(),
            SyncStatus::Syncing => {
                format!("Syncing: {}  {}%", sync_phase_label(sync), sync.percent)
            }
            SyncStatus::Done => format!(
                "Sync done — settings={} exported={}",
                sync.settings_synced, sync.sessions_exported
            ),
            SyncStatus::Failed => format!(
                "Sync failed: {}",
                sync.error.as_deref().unwrap_or("unknown error")
            ),
        };
        let sync_style = match sync.status {
            SyncStatus::Failed => theme.style(StyleKind::Error),
            SyncStatus::Done => theme.style(StyleKind::Info),
            SyncStatus::Syncing => theme.style(StyleKind::Primary),
            SyncStatus::Idle => theme.style(StyleKind::Muted),
        };
        let bar_width = rows[1].width.saturating_sub(2) as usize;
        let filled = if sync.status == SyncStatus::Syncing || sync.status == SyncStatus::Done {
            ((sync.percent as usize) * bar_width) / 100
        } else {
            0
        };
        let bar = format!(
            "[{}{}]",
            "#".repeat(filled.min(bar_width)),
            "-".repeat(bar_width.saturating_sub(filled))
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(sync_text, sync_style)),
                Line::from(Span::styled(bar, theme.style(StyleKind::Muted))),
            ]),
            rows[1],
        );

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
        frame.render_widget(Paragraph::new(device_lines), rows[2]);

        let btn_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[3]);
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
            rows[4],
            "Tab Switch   Enter Activate   Esc Close",
            theme,
        );
    }

    fn render_field_label(
        &self,
        frame: &mut Frame,
        area: Rect,
        text: &str,
        active: bool,
        theme: &Theme,
    ) {
        let style = if active {
            theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)
        } else {
            theme.style(StyleKind::Muted)
        };
        frame.render_widget(Paragraph::new(Line::from(Span::styled(text, style))), area);
    }

    fn render_text_input(&self, frame: &mut Frame, area: Rect, item: LoginFocus, theme: &Theme) {
        let active = self.login_focus == item;
        let raw = match item {
            LoginFocus::AuthServer => self.auth_server.as_str(),
            LoginFocus::Username => self.username.as_str(),
            LoginFocus::Password => self.password.as_str(),
            LoginFocus::Login => "",
        };
        let display = if item == LoginFocus::Password {
            "*".repeat(raw.chars().count())
        } else {
            raw.to_string()
        };
        let prefix = if active { "> " } else { "  " };
        let mut spans = vec![Span::styled(
            prefix,
            if active {
                theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )];
        if active {
            let cursor = self.cursor.min(display.chars().count());
            let before: String = display.chars().take(cursor).collect();
            let after: String = display.chars().skip(cursor).collect();
            let cursor_char = after.chars().next().unwrap_or(' ');
            let after_rest: String = after.chars().skip(1).collect();
            spans.push(Span::styled(before, Style::default().fg(Color::White)));
            spans.push(Span::styled(
                cursor_char.to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            ));
            spans.push(Span::styled(after_rest, Style::default().fg(Color::White)));
        } else if !display.is_empty() {
            spans.push(Span::styled(display, Style::default().fg(Color::White)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
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

    fn render_choice_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        desc: &str,
        active: bool,
        theme: &Theme,
    ) {
        let marker = if active { "> " } else { "  " };
        let title_style = if active {
            theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(marker, title_style),
                Span::styled(format!("{title}  —  {desc}"), title_style),
            ])),
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

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
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
    use crossterm::event::KeyEventKind;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    #[test]
    fn submit_requires_all_fields() {
        let mut form = LoginFormState::new();
        form.show();
        form.login_focus = LoginFocus::Login;
        assert!(matches!(
            form.handle_key_event(key(KeyCode::Enter)),
            LoginFormAction::None
        ));
        assert!(form.error.is_some());
    }

    #[test]
    fn insert_paste_strips_newlines_into_active_field() {
        let mut form = LoginFormState::new();
        form.show();
        form.insert_paste("https://example.com/relay\nextra");
        assert_eq!(form.auth_server, "https://example.com/relayextra");
    }

    #[test]
    fn sync_choice_enter_use_local() {
        let mut form = LoginFormState::new();
        form.show_sync_choice("u1", "https://relay");
        assert!(matches!(
            form.handle_key_event(key(KeyCode::Enter)),
            LoginFormAction::SyncUseLocal
        ));
    }
}
