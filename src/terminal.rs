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
    /// Creates a new terminal in TUI mode.
    ///
    /// Enables raw mode and enters the alternate screen.
    ///
    /// # Errors
    /// Returns an error if enabling raw mode or entering alternate screen fails.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        enable_raw_mode()?;

        let mut stdout = std::io::stdout();
        if let Err(e) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(Box::new(e));
        }

        let backend = CrosstermBackend::new(stdout);
        match Terminal::new(backend) {
            Ok(terminal) => Ok(Self {
                terminal,
                active: true,
            }),
            Err(e) => {
                let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
                let _ = disable_raw_mode();
                Err(Box::new(e))
            }
        }
    }

    /// Starts (or restarts) the TUI mode.
    ///
    /// Re-enables raw mode and re-enters the alternate screen if currently stopped.
    /// This is useful for pausing and resuming the TUI.
    ///
    /// # Errors
    /// Returns an error if enabling raw mode or entering alternate screen fails.
    /// If an error occurs, raw mode is disabled before returning.
    #[cfg(unix)]
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.active {
            enable_raw_mode()?;

            if let Err(e) = execute!(self.backend_mut(), EnterAlternateScreen) {
                let _ = disable_raw_mode();
                return Err(Box::new(e));
            }

            self.active = true;
        }
        Ok(())
    }

    /// Stops the TUI mode, restoring the normal terminal state.
    ///
    /// Leaves the alternate screen, disables raw mode, and shows the cursor.
    /// After calling `stop()`, the terminal can be restarted with `start()`.
    ///
    /// # Errors
    /// Returns an error if any cleanup operation fails.
    pub fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.active {
            self.active = false;
            let mut first_error = None;

            if let Err(e) = execute!(self.backend_mut(), LeaveAlternateScreen) {
                first_error = Some(e.into());
            }

            if let Err(e) = disable_raw_mode()
                && first_error.is_none() {
                    first_error = Some(e.into());
                }

            if let Err(e) = self.terminal.show_cursor()
                && first_error.is_none() {
                    first_error = Some(e.into());
                }

            if let Some(e) = first_error {
                return Err(e);
            }
        }
        Ok(())
    }

    /// Restores the terminal to its original state.
    ///
    /// This is an alias for `stop()` provided for semantic clarity.
    /// Use this when permanently exiting the TUI application.
    ///
    /// # Errors
    /// Returns an error if any cleanup operation fails.
    pub fn restore(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop()
    }

    /// Clears the terminal and forces a full redraw on the next draw call.
    ///
    /// This is needed when the terminal state may be corrupted (e.g., after suspend/resume)
    /// because ratatui uses diff-based rendering and won't repaint unchanged areas.
    ///
    /// # Errors
    /// Returns an error if clearing fails.
    #[cfg(unix)]
    pub fn clear(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.terminal.clear().map_err(Into::into)
    }

    /// Returns the terminal area as a rectangle.
    ///
    /// Convenience method that returns a `Rect` covering the full terminal.
    ///
    /// # Errors
    /// Returns an error if the terminal size cannot be determined.
    pub fn get_area(&self) -> Result<ratatui::layout::Rect, Box<dyn std::error::Error>> {
        let size = self.terminal.size()?;
        Ok(ratatui::layout::Rect::new(0, 0, size.width, size.height))
    }

    /// Draws content to the terminal.
    ///
    /// # Arguments
    /// * `f` - A closure that receives a mutable frame for rendering.
    ///
    /// # Errors
    /// Returns an error if drawing fails.
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

impl Drop for TuiTerminal {
    fn drop(&mut self) {
        if self.active {
            let _ = execute!(self.backend_mut(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            let _ = self.terminal.show_cursor();
        }
    }
}
