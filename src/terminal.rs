// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::io::Stdout;

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{
    Frame, Terminal,
    backend::{Backend, ClearType, CrosstermBackend, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
    style::Color,
};

/// Whether terminal colors are disabled via `NO_COLOR`.
///
/// Mirrors crossterm's own predicate so the wrapper below and the
/// escape-sequence layer agree on when colors are off.
fn colors_disabled() -> bool {
    std::env::var("NO_COLOR")
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

/// [`Backend`] wrapper that drops colors when `NO_COLOR` is set.
///
/// crossterm skips color escape sequences under `NO_COLOR`, but still emits
/// the now-empty `SetColors` sequence (`ESC[;m`), which terminals interpret
/// as SGR reset. Since color sequences are queued after attribute sequences,
/// that stray reset wipes modifiers such as `REVERSED` selection or
/// `CROSSED_OUT` offline markers on every explicitly colored cell. Mapping
/// all cells to reset colors avoids emitting the sequence entirely, so
/// attribute-only styling survives in monochrome mode.
#[derive(Debug)]
struct NoColorBackend<B: Backend> {
    inner: B,
    strip_colors: bool,
}

impl<B: Backend> NoColorBackend<B> {
    fn new(inner: B) -> Self {
        Self {
            inner,
            strip_colors: colors_disabled(),
        }
    }

    fn inner_mut(&mut self) -> &mut B {
        &mut self.inner
    }
}

impl<B: Backend> Backend for NoColorBackend<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        if !self.strip_colors {
            return self.inner.draw(content);
        }
        let mut plain = Vec::new();
        for (x, y, cell) in content {
            let mut cell = cell.clone();
            cell.fg = Color::Reset;
            cell.bg = Color::Reset;
            cell.underline_color = Color::Reset;
            plain.push((x, y, cell));
        }
        self.inner
            .draw(plain.iter().map(|(x, y, cell)| (*x, *y, cell)))
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

pub struct TuiTerminal {
    terminal: Terminal<NoColorBackend<CrosstermBackend<Stdout>>>,
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
        match Terminal::new(NoColorBackend::new(backend)) {
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
                && first_error.is_none()
            {
                first_error = Some(e.into());
            }

            if let Err(e) = self.terminal.show_cursor()
                && first_error.is_none()
            {
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
        self.terminal.backend_mut().inner_mut()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[derive(Debug, Default)]
    struct RecordingBackend {
        drawn: Vec<(u16, u16, Cell)>,
    }

    impl Backend for RecordingBackend {
        type Error = io::Error;

        fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
        where
            I: Iterator<Item = (u16, u16, &'a Cell)>,
        {
            self.drawn
                .extend(content.map(|(x, y, cell)| (x, y, cell.clone())));
            Ok(())
        }

        fn hide_cursor(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn show_cursor(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
            Ok(Position::new(0, 0))
        }

        fn set_cursor_position<P: Into<Position>>(
            &mut self,
            _position: P,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        fn clear(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn clear_region(&mut self, _clear_type: ClearType) -> Result<(), Self::Error> {
            Ok(())
        }

        fn size(&self) -> Result<Size, Self::Error> {
            Ok(Size::new(80, 24))
        }

        fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
            Ok(WindowSize {
                columns_rows: Size::new(80, 24),
                pixels: Size::new(0, 0),
            })
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn selected_cell() -> Cell {
        let mut cell = Cell::default();
        cell.set_symbol("x");
        cell.fg = Color::White;
        cell.bg = Color::DarkGray;
        cell.underline_color = Color::White;
        cell.modifier = ratatui::style::Modifier::REVERSED;
        cell
    }

    #[test]
    fn test_no_color_backend_strips_colors_keeps_modifiers() {
        let cell = selected_cell();
        let mut backend = NoColorBackend {
            inner: RecordingBackend::default(),
            strip_colors: true,
        };
        backend.draw([(0u16, 0u16, &cell)].into_iter()).unwrap();

        assert_eq!(backend.inner.drawn.len(), 1);
        let (_, _, drawn) = &backend.inner.drawn[0];
        assert_eq!(drawn.fg, Color::Reset);
        assert_eq!(drawn.bg, Color::Reset);
        assert_eq!(drawn.underline_color, Color::Reset);
        assert_eq!(drawn.modifier, ratatui::style::Modifier::REVERSED);
        assert_eq!(drawn.symbol(), "x");
    }

    #[test]
    fn test_no_color_backend_passes_cells_through_when_colors_enabled() {
        let cell = selected_cell();
        let mut backend = NoColorBackend {
            inner: RecordingBackend::default(),
            strip_colors: false,
        };
        backend.draw([(0u16, 0u16, &cell)].into_iter()).unwrap();

        assert_eq!(backend.inner.drawn.len(), 1);
        let (_, _, drawn) = &backend.inner.drawn[0];
        assert_eq!(drawn.fg, Color::White);
        assert_eq!(drawn.bg, Color::DarkGray);
        assert_eq!(drawn.modifier, ratatui::style::Modifier::REVERSED);
    }

    #[test]
    fn test_no_color_backend_delegates_terminal_ops() {
        let mut backend = NoColorBackend {
            inner: RecordingBackend::default(),
            strip_colors: true,
        };
        assert!(backend.hide_cursor().is_ok());
        assert!(backend.show_cursor().is_ok());
        assert!(backend.clear().is_ok());
        assert!(backend.flush().is_ok());
        assert_eq!(backend.size().unwrap(), Size::new(80, 24));
        assert_eq!(backend.get_cursor_position().unwrap(), Position::new(0, 0));
    }

    /// End-to-end through the real [`CrosstermBackend`]: under `NO_COLOR`,
    /// crossterm renders an emptied `SetColors` as `ESC[;m`, which terminals
    /// read as SGR reset and which used to wipe the `REVERSED` selection right
    /// before the glyphs. The wrapper must prevent that sequence entirely.
    /// This holds regardless of the process `NO_COLOR` value because the
    /// wrapper strips colors before crossterm ever sees them.
    #[test]
    fn test_no_color_backend_emits_reverse_without_color_reset_wipe() {
        use std::cell::RefCell;
        use std::rc::Rc;

        #[derive(Clone, Default)]
        struct SharedBuf(Rc<RefCell<Vec<u8>>>);

        impl std::io::Write for SharedBuf {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.borrow_mut().extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let shared = SharedBuf::default();
        let mut backend = NoColorBackend {
            inner: CrosstermBackend::new(shared.clone()),
            strip_colors: true,
        };
        let style = ratatui::style::Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(ratatui::style::Modifier::REVERSED);
        let cells: Vec<Cell> = "hi"
            .chars()
            .map(|ch| {
                let mut cell = Cell::default();
                cell.set_symbol(&ch.to_string());
                cell.set_style(style);
                cell
            })
            .collect();
        backend
            .draw(
                cells
                    .iter()
                    .enumerate()
                    .map(|(i, cell)| (i as u16, 0u16, cell)),
            )
            .unwrap();
        backend.flush().unwrap();
        let emitted = String::from_utf8_lossy(&shared.0.borrow()).into_owned();
        assert!(
            emitted.contains("\u{1b}[7m"),
            "reverse video must be emitted, got {emitted:?}"
        );
        assert!(
            !emitted.contains(";m"),
            "no emptied SetColors reset may precede the glyphs, got {emitted:?}"
        );
    }
}
