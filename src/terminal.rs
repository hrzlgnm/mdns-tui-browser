// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT-0
#![forbid(unsafe_code)]

use std::io::Stdout;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{Frame, Terminal, backend::CrosstermBackend};

pub struct TuiTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl TuiTerminal {
    #[allow(dead_code)]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            active: true,
        })
    }

    #[allow(dead_code)]
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.active {
            enable_raw_mode()?;
            execute!(self.backend_mut(), EnterAlternateScreen)?;
            self.active = true;
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.active {
            execute!(self.backend_mut(), LeaveAlternateScreen)?;
            disable_raw_mode()?;
            self.terminal.show_cursor()?;
            self.active = false;
        }
        Ok(())
    }

    pub fn restore(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop()
    }

    pub fn size(&self) -> Result<ratatui::layout::Size, Box<dyn std::error::Error>> {
        self.terminal.size().map_err(Into::into)
    }

    pub fn get_area(&self) -> Result<ratatui::layout::Rect, Box<dyn std::error::Error>> {
        let size = self.size()?;
        Ok(ratatui::layout::Rect::new(0, 0, size.width, size.height))
    }

    pub fn draw<F>(&mut self, f: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    fn backend_mut(&mut self) -> &mut CrosstermBackend<Stdout> {
        self.terminal.backend_mut()
    }
}
