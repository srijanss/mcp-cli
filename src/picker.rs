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
    /// cursor marked with `>`, and a key help line.
    pub fn render(&self, details: &[String]) -> Vec<String> {
        let mut lines = vec!["Select MCPs for this project".to_owned()];
        for (index, (name, detail)) in self.names.iter().zip(details).enumerate() {
            let cursor = if index == self.cursor { '>' } else { ' ' };
            let mark = if self.selected[index] { 'x' } else { ' ' };
            lines.push(format!("{cursor} [{mark}] {name}  {detail}"));
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

/// `line` cut to fit a terminal `columns` wide without wrapping, leaving the last column free; a width of 0
/// means the terminal did not report one, so the line is left whole.
pub fn fit_to_width(line: &str, columns: u16) -> String {
    match columns {
        0 => line.to_owned(),
        columns => line.chars().take(usize::from(columns) - 1).collect(),
    }
}
