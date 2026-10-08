use crossterm::event::KeyCode;

/// The `setup` picker's state, free of terminal I/O: a cursor over the catalog's MCP names and the set
/// of names ticked so far.
pub struct Picker {
    names: Vec<String>,
    selected: Vec<bool>,
    cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickerKey {
    Up,
    Down,
    Toggle,
    SelectAll,
    SelectNone,
    Confirm,
    Cancel,
}

impl PickerKey {
    /// The picker key a terminal key press stands for: arrows or `k`/`j` move, space toggles, `a`/`n`
    /// select all or none, enter confirms, and esc or `q` cancels.
    pub fn from_key_code(code: KeyCode) -> Option<PickerKey> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => Some(PickerKey::Up),
            KeyCode::Down | KeyCode::Char('j') => Some(PickerKey::Down),
            KeyCode::Char(' ') => Some(PickerKey::Toggle),
            KeyCode::Char('a') => Some(PickerKey::SelectAll),
            KeyCode::Char('n') => Some(PickerKey::SelectNone),
            KeyCode::Enter => Some(PickerKey::Confirm),
            KeyCode::Esc | KeyCode::Char('q') => Some(PickerKey::Cancel),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PickerOutcome {
    Pending,
    Confirmed(Vec<String>),
    Cancelled,
}

impl Picker {
    pub fn new(names: Vec<String>, preselected: &[String]) -> Self {
        let selected = names.iter().map(|name| preselected.contains(name)).collect();
        Picker { names, selected, cursor: 0 }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected[index]
    }

    /// The picker's screen: a heading, one `[x]`/`[ ]` line per MCP followed by its `details`, the
    /// cursor marked with `>`, and a key help line. When the MCPs do not fit a terminal `rows` high, only a
    /// window of them around the cursor is shown, between lines counting the MCPs hidden above and below; the
    /// frame stays a line short of `rows` so drawing it never scrolls the terminal. A `rows` of 0 means the
    /// terminal did not report one, so every MCP is shown.
    pub fn render(&self, details: &[String], rows: u16) -> Vec<String> {
        // The heading, the blank line and the key help line.
        const CHROME: usize = 3;
        let room = usize::from(rows).saturating_sub(1 + CHROME);
        let scrolls = rows != 0 && self.names.len() > room;
        let visible = if scrolls { room.saturating_sub(2).max(1) } else { self.names.len() };
        let start = self.cursor.saturating_sub(visible / 2).min(self.names.len() - visible);
        let end = start + visible;

        let mut lines = vec!["Select MCPs for this project".to_owned()];
        if scrolls {
            lines.push(if start > 0 { format!("  ^ {start} more") } else { String::new() });
        }
        for (index, (name, detail)) in self.names.iter().zip(details).enumerate().take(end).skip(start) {
            let cursor = if index == self.cursor { '>' } else { ' ' };
            let mark = if self.selected[index] { 'x' } else { ' ' };
            lines.push(format!("{cursor} [{mark}] {}  {}", printable(name), printable(detail)));
        }
        if scrolls {
            let below = self.names.len() - end;
            lines.push(if below > 0 { format!("  v {below} more") } else { String::new() });
        }
        lines.push(String::new());
        lines.push("up/down move  space toggle  a all  n none  enter confirm  esc/q cancel".to_owned());
        lines
    }

    pub fn handle(&mut self, key: PickerKey) -> PickerOutcome {
        match key {
            PickerKey::Up => self.cursor = self.cursor.saturating_sub(1),
            PickerKey::Down => self.cursor = (self.cursor + 1).min(self.names.len().saturating_sub(1)),
            PickerKey::Toggle => self.selected[self.cursor] = !self.selected[self.cursor],
            PickerKey::SelectAll => self.selected.fill(true),
            PickerKey::SelectNone => self.selected.fill(false),
            PickerKey::Confirm => {
                let chosen = self.names.iter().zip(&self.selected).filter(|(_, selected)| **selected);
                return PickerOutcome::Confirmed(chosen.map(|(name, _)| name.clone()).collect());
            }
            PickerKey::Cancel => return PickerOutcome::Cancelled,
        }
        PickerOutcome::Pending
    }
}

/// `line` cut to fit a terminal `columns` wide without wrapping, leaving the last column free. Width is counted
/// in screen columns, so wide characters count twice and zero-width marks stay with their base character; a
/// wide character that would straddle the edge is dropped. A width of 0 means the terminal did not report
/// one, so the line is left whole.
pub fn fit_to_width(line: &str, columns: u16) -> String {
    use unicode_width::UnicodeWidthChar;
    if columns == 0 {
        return line.to_owned();
    }
    let mut room = usize::from(columns) - 1;
    let mut fitted = String::new();
    for character in line.chars() {
        let width = character.width().unwrap_or(0);
        if width > room {
            break;
        }
        room -= width;
        fitted.push(character);
    }
    fitted
}

/// The terminal mode changes the picker makes, so they can be undone even when one of them fails.
pub trait Terminal {
    fn enable_raw_mode(&mut self) -> std::io::Result<()>;
    fn disable_raw_mode(&mut self) -> std::io::Result<()>;
    fn hide_cursor(&mut self) -> std::io::Result<()>;
    fn show_cursor(&mut self) -> std::io::Result<()>;
}

/// Runs `body` with `terminal` in raw mode and its cursor hidden, then restores both whatever happened. Every
/// restore step runs even if an earlier one fails; the first error (setup, `body`, then restore) is returned.
pub fn with_terminal<T: Terminal, R>(terminal: &mut T, body: impl FnOnce(&mut T) -> std::io::Result<R>) -> std::io::Result<R> {
    terminal.enable_raw_mode()?;
    let result = terminal.hide_cursor().and_then(|()| body(terminal));
    let disabled = terminal.disable_raw_mode();
    let shown = terminal.show_cursor();
    let value = result?;
    disabled?;
    shown?;
    Ok(value)
}

/// `text` with every control character written out as an escape such as `\u{1b}`, so catalog text cannot move
/// the cursor, clear the screen or otherwise drive the terminal it is drawn on.
pub fn printable(text: &str) -> String {
    text.chars()
        .map(|character| if character.is_control() { character.escape_default().to_string() } else { character.to_string() })
        .collect()
}
