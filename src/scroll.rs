// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

#[derive(Debug, Clone)]
pub struct ScrollState {
    pub offset: usize,
    pub visible_items: usize,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            visible_items: 0,
        }
    }

    pub fn update_offset(&mut self, selected_index: usize, total_items: usize) {
        if selected_index < self.offset {
            self.offset = selected_index;
        } else if self.visible_items > 0 && selected_index >= self.offset + self.visible_items {
            self.offset = selected_index - self.visible_items + 1;
        }

        if total_items > 0 && self.offset > total_items.saturating_sub(1) {
            self.offset = total_items.saturating_sub(1);
        }
    }

    pub fn page_scroll_amount(&self) -> usize {
        self.visible_items.saturating_sub(1)
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_new() {
        let scroll = ScrollState::new();
        assert_eq!(scroll.offset, 0);
        assert_eq!(scroll.visible_items, 0);
    }

    #[test]
    fn test_scroll_state_reset() {
        let mut scroll = ScrollState::new();
        scroll.offset = 10;
        scroll.visible_items = 5;
        scroll.reset();
        assert_eq!(scroll.offset, 0);
    }

    #[test]
    fn test_scroll_state_update_offset() {
        let mut scroll = ScrollState::new();
        scroll.visible_items = 5;

        scroll.update_offset(0, 10);
        assert_eq!(scroll.offset, 0);

        scroll.update_offset(8, 10);
        assert_eq!(scroll.offset, 4);
    }
}
