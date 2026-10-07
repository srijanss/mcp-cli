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
