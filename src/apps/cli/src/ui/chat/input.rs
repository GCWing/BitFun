impl ChatView {
    // ============ Input handling methods (delegate to TextInput) ============

    pub fn input_text(&self) -> &str {
        self.text_input.text()
    }

    fn refresh_command_menu(&mut self) {
        self.command_menu.update(&self.text_input.input, self.text_input.cursor);
    }

    /// Send user input, returns the input text if non-empty
    pub fn send_input(&mut self) -> Option<String> {
        let text = self.text_input.take_input()?;

        self.input_history.push_front(text.clone());
        if self.input_history.len() > 50 {
            self.input_history.pop_back();
        }
        self.history_index = None;
        self.refresh_command_menu();

        Some(text)
    }

    pub fn handle_char(&mut self, c: char) {
        self.text_input.handle_char(c);
        self.refresh_command_menu();
    }

    pub fn handle_newline(&mut self) {
        self.text_input.handle_newline();
        self.refresh_command_menu();
    }

    pub fn handle_backspace(&mut self) {
        self.text_input.handle_backspace();
        self.refresh_command_menu();
    }

    pub fn move_cursor_left(&mut self) {
        self.text_input.move_cursor_left();
        self.refresh_command_menu();
    }

    pub fn move_cursor_right(&mut self) {
        self.text_input.move_cursor_right();
        self.refresh_command_menu();
    }

    pub fn set_cursor_home(&mut self) {
        self.text_input.set_cursor_home();
        self.refresh_command_menu();
    }

    pub fn set_cursor_end(&mut self) {
        self.text_input.set_cursor_end();
        self.refresh_command_menu();
    }

    pub fn clear_input(&mut self) {
        self.text_input.clear();
        self.refresh_command_menu();
    }

    /// Set input text programmatically (e.g. from skill selection)
    pub fn set_input(&mut self, text: &str) {
        self.text_input.set_text(text);
        self.refresh_command_menu();
    }

    pub fn command_menu_visible(&self) -> bool {
        self.command_menu.is_visible()
    }

    pub fn command_menu_up(&mut self) {
        self.command_menu.move_up();
    }

    pub fn command_menu_down(&mut self) {
        self.command_menu.move_down();
    }

    pub fn apply_command_menu_selection(&mut self) -> Option<String> {
        let cmd = self.command_menu.apply_selection()?;
        self.text_input.clear();
        self.refresh_command_menu();
        Some(cmd)
    }

    pub fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => 0,
            Some(i) if i + 1 < self.input_history.len() => i + 1,
            Some(i) => i,
        };

        if let Some(history_item) = self.input_history.get(new_index) {
            self.text_input.set_text(history_item);
            self.history_index = Some(new_index);
            self.refresh_command_menu();
        }
    }

    pub fn history_next(&mut self) {
        match self.history_index {
            None => {}
            Some(0) => {
                self.text_input.clear();
                self.history_index = None;
                self.refresh_command_menu();
            }
            Some(i) => {
                let new_index = i - 1;
                if let Some(history_item) = self.input_history.get(new_index) {
                    self.text_input.set_text(history_item);
                    self.history_index = Some(new_index);
                    self.refresh_command_menu();
                }
            }
        }
    }
}
