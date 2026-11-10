use std::fmt;

/// Błąd z informacją, gdzie w pliku wystąpił.
/// `col == 0` oznacza "cała linia" (bez wskazywania kolumny).
#[derive(Debug, Clone)]
pub struct DsError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl DsError {
    pub fn new(line: usize, col: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            col,
            message: message.into(),
        }
    }

    /// Ładnie wypisuje błąd razem z linią źródła i strzałką pod kolumną.
    pub fn render(&self, source: &str) -> String {
        let mut out = format!("błąd (linia {}", self.line);
        if self.col > 0 {
            out.push_str(&format!(", kolumna {}", self.col));
        }
        out.push_str(&format!("): {}\n", self.message));

        if let Some(text) = source.lines().nth(self.line.saturating_sub(1)) {
            out.push_str(&format!("  {:>3} | {}\n", self.line, text));
            if self.col > 0 {
                out.push_str(&format!("      | {}^\n", " ".repeat(self.col - 1)));
            }
        }
        out
    }
}

impl fmt::Display for DsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "linia {}: {}", self.line, self.message)
    }
}
