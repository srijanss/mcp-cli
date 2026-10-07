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
