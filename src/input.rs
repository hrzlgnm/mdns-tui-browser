// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

/// Represents the current input mode for user text input.
///
/// Different modes affect how input is processed and displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    /// No active input mode - regular navigation
    None,
    /// Quick filter mode - filters services by text query
    Filter,
    /// Service type input mode - add new service types
    ServiceType,
}

#[derive(Clone, Debug)]
pub struct InputState {
    pub text: String,
    pub mode: InputMode,
}

impl InputState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            mode: InputMode::None,
        }
    }

    pub fn start(&mut self, mode: InputMode) {
        self.mode = mode;
        self.text.clear();
    }

    pub fn clear(&mut self) {
        self.mode = InputMode::None;
        self.text.clear();
    }

    pub fn apply(&mut self) {
        self.mode = InputMode::None;
    }

    pub fn add_char(&mut self, ch: char) {
        self.text.push(ch);
    }

    pub fn remove_char(&mut self) {
        self.text.pop();
    }

    pub fn is_active(&self) -> bool {
        self.mode != InputMode::None
    }

    pub fn filter_title() -> &'static str {
        "Quick Filter (Enter to apply, Esc to cancel)"
    }

    pub fn service_type_title() -> &'static str {
        "Add Service Type (Enter to add, Esc to cancel)"
    }

    pub fn input_prefix(&self) -> char {
        match self.mode {
            InputMode::Filter => '/',
            InputMode::ServiceType => ' ',
            InputMode::None => ' ',
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_input(
    f: &mut Frame,
    input_state: &InputState,
    area: Rect,
    border_style: Style,
    fg_color: Color,
) {
    let input_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(3),
        area.width,
        3,
    );

    let title = match input_state.mode {
        InputMode::Filter => InputState::filter_title(),
        InputMode::ServiceType => InputState::service_type_title(),
        InputMode::None => return,
    };

    let prefix = input_state.input_prefix();
    let input_text = format!("{}{}_", prefix, input_state.text);

    let input_widget = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .style(Style::default().fg(fg_color));

    f.render_widget(input_widget, input_area);
}
