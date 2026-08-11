use std::{sync::mpsc::Receiver, time::Duration};

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};

use crate::notes::model::Note;

pub type ScanResult = (usize, Result<Vec<Note>, String>);

pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize,
    ScanFinished(ScanResult),
    Tick,
}

pub struct Events {
    scans: Receiver<ScanResult>,
}

impl Events {
    pub fn new(scans: Receiver<ScanResult>) -> Self {
        Self { scans }
    }

    pub fn next(&self) -> std::io::Result<Event> {
        if let Ok(result) = self.scans.try_recv() {
            return Ok(Event::ScanFinished(result));
        }
        if !event::poll(Duration::from_millis(100))? {
            return Ok(Event::Tick);
        }
        match event::read()? {
            CrosstermEvent::Key(key) => Ok(Event::Key(key)),
            CrosstermEvent::Mouse(mouse) => Ok(Event::Mouse(mouse)),
            CrosstermEvent::Resize(_, _) => Ok(Event::Resize),
            _ => Ok(Event::Tick),
        }
    }
}
