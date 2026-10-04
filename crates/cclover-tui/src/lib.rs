#![deny(unsafe_code)]

use std::io;
use std::time::Duration;

use cclover_presentation::Dashboard;

mod cell_width;
mod frame;
mod layout;
mod native;

use native::Terminal;

pub struct TerminalUi {
    terminal: Terminal,
}

impl TerminalUi {
    pub fn enter() -> io::Result<Self> {
        Ok(Self {
            terminal: Terminal::enter()?,
        })
    }

    pub fn draw(&mut self, dashboard: Dashboard<'_>) -> io::Result<()> {
        let (width, height) = self.terminal.size()?;
        let frame = frame::build(dashboard);
        let rendered = layout::render(&frame, width as usize, height as usize);
        self.terminal.draw(&rendered)
    }

    pub fn size_changed(&mut self) -> io::Result<bool> {
        self.terminal.size_changed()
    }

    pub fn wait_for_quit(&mut self, wait: Duration) -> io::Result<bool> {
        self.terminal.wait_for_quit(wait)
    }
}
