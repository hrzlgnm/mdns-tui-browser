// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};

#[derive(Clone, Debug)]
pub struct InputState {
    active: bool,
    text: String,
    title: String,
}

impl InputState {
    pub fn new(title: &str) -> Self {
        Self {
            active: false,
            text: String::new(),
            title: title.into(),
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.text.clear();
    }

    pub fn add_char(&mut self, ch: char) {
        self.text.push(ch);
    }

    pub fn remove_char(&mut self) {
        self.text.pop();
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.into();
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

pub fn render_input(
    f: &mut Frame,
    input_state: &InputState,
    area: Rect,
    border_style: Style,
    fg_color: Color,
) {
    let height = area.height.min(3);
    let input_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(height),
        area.width,
        height,
    );

    let input_text = format!("{}_", input_state.text());

    let input_widget = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(input_state.title()),
        )
        .style(Style::default().fg(fg_color));

    f.render_widget(input_widget, input_area);
}
