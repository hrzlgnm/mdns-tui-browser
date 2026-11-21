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

/// Height of the text input overlay at the bottom of the terminal.
/// Covers the same rows as the filter status bar, whose height is
/// `FILTER_STATUS_BAR_HEIGHT` in `tui_app`; keep the two in sync or the
/// mouse will aim presses at rows the overlay does not cover.
pub const INPUT_OVERLAY_HEIGHT: u16 = 3;

/// Rect of the text input overlay: the bottom rows of `area`. Shared by
/// the renderer and the mouse path, so presses aimed at the input do
/// not fall through to the panes behind it.
pub fn input_area(area: Rect) -> Rect {
    let height = area.height.min(INPUT_OVERLAY_HEIGHT);
    Rect::new(
        area.x,
        area.y + area.height.saturating_sub(height),
        area.width,
        height,
    )
}

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
    let overlay_area = input_area(area);

    let input_text = format!("{}_", input_state.text());

    let input_widget = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(input_state.title()),
        )
        .style(Style::default().fg(fg_color));

    f.render_widget(input_widget, overlay_area);
}
