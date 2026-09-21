use serde_json::Value;

/// Merges `template` into `existing` without ever removing or changing what is already there.
///
/// Objects merge key by key, arrays gain the template items they do not already contain
/// (compared by value), and any other existing value wins. Returns the merged value and
/// whether anything was added.
pub fn merge_json(existing: Value, template: &Value) -> (Value, bool) {
    match (existing, template) {
        (Value::Object(mut existing), Value::Object(template)) => {
            let mut changed = false;
            for (key, template_value) in template {
                // Merge in place: removing and re-inserting a key would reorder the object.
                match existing.get_mut(key) {
                    Some(current) => {
                        let (merged, merged_changed) = merge_json(std::mem::take(current), template_value);
                        changed |= merged_changed;
                        *current = merged;
                    }
                    None => {
                        existing.insert(key.clone(), template_value.clone());
                        changed = true;
                    }
                }
            }
            (Value::Object(existing), changed)
        }
        (Value::Array(mut existing), Value::Array(template)) => {
            let changed = merge_json_items(&mut existing, template);
            (Value::Array(existing), changed)
        }
        (existing, _) => (existing, false),
    }
}

/// Hook registrations are keyed by their `matcher`: an entry with a matcher already present is
/// merged into that entry (so its hook list gains only the missing hooks) instead of being
/// appended as a second entry for the same matcher. Any other item is appended when absent.
fn merge_json_items(existing: &mut Vec<Value>, template: &[Value]) -> bool {
    let mut changed = false;
    for item in template {
        let same_matcher = item.get("matcher").and_then(|matcher| existing.iter_mut().find(|entry| entry.get("matcher") == Some(matcher)));
        if let Some(entry) = same_matcher {
            let (merged, merged_changed) = merge_json(std::mem::take(entry), item);
            *entry = merged;
            changed |= merged_changed;
        } else if !existing.contains(item) {
            existing.push(item.clone());
            changed = true;
        }
    }
    changed
}

/// `merge_json` for TOML documents: tables merge key by key, arrays (including arrays of
/// tables) gain the template items they lack, and existing values win.
pub fn merge_toml(existing: toml::Value, template: &toml::Value) -> (toml::Value, bool) {
    use toml::Value;
    match (existing, template) {
        (Value::Table(mut existing), Value::Table(template)) => {
            let mut changed = false;
            for (key, template_value) in template {
                match existing.get_mut(key) {
                    Some(current) => {
                        let (merged, merged_changed) = merge_toml(std::mem::replace(current, Value::Boolean(false)), template_value);
                        changed |= merged_changed;
                        *current = merged;
                    }
                    None => {
                        existing.insert(key.clone(), template_value.clone());
                        changed = true;
                    }
                }
            }
            (Value::Table(existing), changed)
        }
        (Value::Array(mut existing), Value::Array(template)) => {
            let changed = merge_toml_items(&mut existing, template);
            (Value::Array(existing), changed)
        }
        (existing, _) => (existing, false),
    }
}

/// The TOML counterpart of `merge_json_items`: entries sharing a `matcher` are merged, other items appended when absent.
fn merge_toml_items(existing: &mut Vec<toml::Value>, template: &[toml::Value]) -> bool {
    let mut changed = false;
    for item in template {
        let same_matcher = item.get("matcher").and_then(|matcher| existing.iter_mut().find(|entry| entry.get("matcher") == Some(matcher)));
        if let Some(entry) = same_matcher {
            let (merged, merged_changed) = merge_toml(std::mem::replace(entry, toml::Value::Boolean(false)), item);
            *entry = merged;
            changed |= merged_changed;
        } else if !existing.contains(item) {
            existing.push(item.clone());
            changed = true;
        }
    }
    changed
}
