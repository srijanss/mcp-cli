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
