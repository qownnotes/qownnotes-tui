pub struct Clipboard {
    system: Option<arboard::Clipboard>,
    fallback: String,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            #[cfg(not(test))]
            system: arboard::Clipboard::new().ok(),
            #[cfg(test)]
            system: None,
            fallback: String::new(),
        }
    }

    pub fn set_text(&mut self, text: String) -> bool {
        self.fallback.clone_from(&text);
        let system = self
            .system
            .as_mut()
            .is_some_and(|clipboard| clipboard.set_text(&text).is_ok());
        #[cfg(not(test))]
        let terminal = crossterm::execute!(
            std::io::stdout(),
            crossterm::clipboard::CopyToClipboard::to_clipboard_from(text)
        )
        .is_ok();
        #[cfg(test)]
        let terminal = false;
        system || terminal
    }

    pub fn text(&mut self) -> Option<String> {
        self.system
            .as_mut()
            .and_then(|clipboard| clipboard.get_text().ok())
            .or_else(|| (!self.fallback.is_empty()).then(|| self.fallback.clone()))
    }
}
