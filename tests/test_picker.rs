use mcp_cli::picker::{Picker, PickerKey, PickerOutcome};

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn picker_toggles_the_highlighted_mcp_and_confirms_the_selection_in_catalog_order() {
    let mut picker = Picker::new(names(&["design-advisor-mcp", "project-mcp", "review-mcp"]), &names(&["review-mcp"]));

    assert_eq!(picker.handle(PickerKey::Down), PickerOutcome::Pending);
    assert_eq!(picker.handle(PickerKey::Toggle), PickerOutcome::Pending);
    assert_eq!(picker.handle(PickerKey::Up), PickerOutcome::Pending);
    assert_eq!(picker.handle(PickerKey::Toggle), PickerOutcome::Pending);

    assert_eq!(picker.handle(PickerKey::Confirm), PickerOutcome::Confirmed(names(&["design-advisor-mcp", "project-mcp", "review-mcp"])));
}

#[test]
fn picker_cursor_stays_on_the_first_and_last_mcp_at_either_end() {
    let mut picker = Picker::new(names(&["design-advisor-mcp", "project-mcp"]), &[]);

    picker.handle(PickerKey::Up);
    assert_eq!(picker.cursor(), 0);
    picker.handle(PickerKey::Down);
    picker.handle(PickerKey::Down);
    assert_eq!(picker.cursor(), 1);
    picker.handle(PickerKey::Toggle);

    assert_eq!(picker.handle(PickerKey::Confirm), PickerOutcome::Confirmed(names(&["project-mcp"])));
}

#[test]
fn picker_selects_every_mcp_or_none_at_once() {
    let mut picker = Picker::new(names(&["design-advisor-mcp", "project-mcp"]), &names(&["project-mcp"]));

    picker.handle(PickerKey::SelectAll);
    assert!(picker.is_selected(0) && picker.is_selected(1));
    picker.handle(PickerKey::SelectNone);

    assert_eq!(picker.handle(PickerKey::Confirm), PickerOutcome::Confirmed(vec![]));
}

#[test]
fn picker_cancel_ends_without_a_selection() {
    let mut picker = Picker::new(names(&["design-advisor-mcp"]), &names(&["design-advisor-mcp"]));

    assert_eq!(picker.handle(PickerKey::Cancel), PickerOutcome::Cancelled);
}

#[test]
fn picker_renders_a_marked_line_per_mcp_with_the_cursor_and_a_key_help_line() {
    let mut picker = Picker::new(names(&["design-advisor-mcp", "project-mcp"]), &names(&["project-mcp"]));
    picker.handle(PickerKey::Down);

    let details = names(&["1.0.0  Design advice", "0.2.0  Project context"]);

    assert_eq!(
        picker.render(&details),
        names(&[
            "Select MCPs for this project",
            "  [ ] design-advisor-mcp  1.0.0  Design advice",
            "> [x] project-mcp  0.2.0  Project context",
            "",
            "up/down move  space toggle  a all  n none  enter confirm  esc/q cancel",
        ])
    );
}

#[test]
fn picker_maps_terminal_keys_to_picker_keys() {
    use crossterm::event::KeyCode;

    let keys: Vec<_> = [KeyCode::Up, KeyCode::Char('k'), KeyCode::Down, KeyCode::Char('j'), KeyCode::Char(' '), KeyCode::Char('a'), KeyCode::Char('n'), KeyCode::Enter, KeyCode::Esc, KeyCode::Char('q'), KeyCode::Tab]
        .into_iter()
        .map(PickerKey::from_key_code)
        .collect();

    assert_eq!(
        keys,
        vec![
            Some(PickerKey::Up),
            Some(PickerKey::Up),
            Some(PickerKey::Down),
            Some(PickerKey::Down),
            Some(PickerKey::Toggle),
            Some(PickerKey::SelectAll),
            Some(PickerKey::SelectNone),
            Some(PickerKey::Confirm),
            Some(PickerKey::Cancel),
            Some(PickerKey::Cancel),
            None,
        ]
    );
}

#[test]
fn picker_lines_are_cut_to_the_terminal_width_unless_it_is_unknown() {
    use mcp_cli::picker::fit_to_width;

    assert_eq!(fit_to_width("> [x] project-mcp  0.2.0", 12), "> [x] proje");
    assert_eq!(fit_to_width("> [x] project-mcp", 80), "> [x] project-mcp");
    assert_eq!(fit_to_width("> [x] project-mcp", 0), "> [x] project-mcp");
}

/// A terminal that records each mode change and fails the steps named in `failing`.
#[derive(Default)]
struct FakeTerminal {
    calls: Vec<&'static str>,
    failing: Vec<&'static str>,
}

impl FakeTerminal {
    fn step(&mut self, name: &'static str) -> std::io::Result<()> {
        self.calls.push(name);
        if self.failing.contains(&name) {
            return Err(std::io::Error::other(format!("{name} failed")));
        }
        Ok(())
    }
}

impl mcp_cli::picker::Terminal for FakeTerminal {
    fn enable_raw_mode(&mut self) -> std::io::Result<()> {
        self.step("enable raw mode")
    }
    fn disable_raw_mode(&mut self) -> std::io::Result<()> {
        self.step("disable raw mode")
    }
    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.step("hide cursor")
    }
    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.step("show cursor")
    }
}

#[test]
fn picker_terminal_is_restored_when_hiding_the_cursor_fails() {
    let mut terminal = FakeTerminal { failing: vec!["hide cursor"], ..FakeTerminal::default() };

    let result = mcp_cli::picker::with_terminal(&mut terminal, |_| -> std::io::Result<()> { panic!("the picker must not run") });

    assert_eq!(result.unwrap_err().to_string(), "hide cursor failed");
    assert_eq!(terminal.calls, ["enable raw mode", "hide cursor", "disable raw mode", "show cursor"]);
}

#[test]
fn picker_terminal_shows_the_cursor_even_when_disabling_raw_mode_fails() {
    let mut terminal = FakeTerminal { failing: vec!["disable raw mode"], ..FakeTerminal::default() };

    let result = mcp_cli::picker::with_terminal(&mut terminal, |terminal| {
        terminal.calls.push("picker");
        Ok("picked")
    });

    assert_eq!(result.unwrap_err().to_string(), "disable raw mode failed");
    assert_eq!(terminal.calls, ["enable raw mode", "hide cursor", "picker", "disable raw mode", "show cursor"]);
}

#[test]
fn picker_renders_control_characters_in_catalog_text_as_visible_escapes() {
    let picker = Picker::new(names(&["evil\u{7}-mcp"]), &[]);

    let lines = picker.render(&names(&["1.0.0  \u{1b}[2J\u{1b}[HFAKE"]));

    assert_eq!(lines[1], "> [ ] evil\\u{7}-mcp  1.0.0  \\u{1b}[2J\\u{1b}[HFAKE");
}

#[test]
fn printable_escapes_control_characters_and_keeps_other_text() {
    assert_eq!(mcp_cli::picker::printable("a\u{1b}[2J\tb\u{9b}c é 漢"), "a\\u{1b}[2J\\tb\\u{9b}c é 漢");
}
