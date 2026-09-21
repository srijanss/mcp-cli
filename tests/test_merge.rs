use mcp_cli::merge::{merge_json, merge_toml};
use serde_json::json;

#[test]
fn objects_merge_key_by_key_arrays_append_missing_items_and_existing_scalars_win() {
    let existing = json!({
        "keep": 1,
        "mcpServers": { "other": { "command": "x" } },
        "hooks": { "PreToolUse": [ { "matcher": "Edit", "hooks": [ { "command": "b" } ] } ] }
    });
    let template = json!({
        "keep": 2,
        "mcpServers": { "scaf": { "command": "scaf" } },
        "hooks": { "PreToolUse": [ { "matcher": "Bash", "hooks": [ { "command": "a" } ] } ] }
    });

    let (merged, changed) = merge_json(existing, &template);

    assert!(changed);
    assert_eq!(merged["keep"], 1);
    assert_eq!(merged["mcpServers"]["other"]["command"], "x");
    assert_eq!(merged["mcpServers"]["scaf"]["command"], "scaf");
    assert_eq!(
        merged["hooks"]["PreToolUse"],
        json!([
            { "matcher": "Edit", "hooks": [ { "command": "b" } ] },
            { "matcher": "Bash", "hooks": [ { "command": "a" } ] }
        ])
    );
}

#[test]
fn merging_is_idempotent_and_keeps_the_existing_key_order() {
    let existing: serde_json::Value = serde_json::from_str(r#"{"worktree":{"a":1},"hooks":{"Pre":[1]}}"#).unwrap();
    let template = json!({ "added": true, "hooks": { "Pre": [1, 2] } });

    let (merged, changed) = merge_json(existing, &template);

    assert!(changed);
    let keys: Vec<&String> = merged.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["worktree", "hooks", "added"], "existing keys keep their order, new keys go last");

    let (again, changed_again) = merge_json(merged.clone(), &template);
    assert!(!changed_again, "a second merge of the same template changes nothing");
    assert_eq!(again, merged);
    assert_eq!(again["hooks"]["Pre"], json!([1, 2]), "no duplicated array items");
}

fn toml_value(source: &str) -> toml::Value {
    source.parse().unwrap()
}

#[test]
fn toml_tables_merge_arrays_of_tables_append_and_existing_scalars_win() {
    let existing = toml_value("model = \"mine\"\n\n[mcp_servers.other]\ncommand = \"x\"\n\n[[hooks.PreToolUse]]\nmatcher = \"Edit\"\n");
    let template = toml_value("model = \"theirs\"\n\n[mcp_servers.scaf]\ncommand = \"scaf\"\n\n[[hooks.PreToolUse]]\nmatcher = \"Bash\"\n");

    let (merged, changed) = merge_toml(existing, &template);

    assert!(changed);
    assert_eq!(merged["model"].as_str(), Some("mine"));
    assert_eq!(merged["mcp_servers"]["other"]["command"].as_str(), Some("x"));
    assert_eq!(merged["mcp_servers"]["scaf"]["command"].as_str(), Some("scaf"));
    let hooks = merged["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(hooks.len(), 2);
    assert_eq!(hooks[0]["matcher"].as_str(), Some("Edit"));
    assert_eq!(hooks[1]["matcher"].as_str(), Some("Bash"));

    let (again, changed_again) = merge_toml(merged.clone(), &template);
    assert!(!changed_again, "merging the same template again changes nothing");
    assert_eq!(again, merged);
}

#[test]
fn toml_merge_keeps_the_existing_key_order() {
    let existing = toml_value("zeta = 1\n\n[mid]\nx = 1\n");
    let template = toml_value("alpha = 2\n");

    let (merged, _) = merge_toml(existing, &template);

    let keys: Vec<&String> = merged.as_table().unwrap().keys().collect();
    assert_eq!(keys, ["zeta", "mid", "alpha"], "existing keys keep their order, new keys go last");
}

#[test]
fn json_hook_entries_with_the_same_matcher_are_merged_not_duplicated() {
    let existing = json!({ "hooks": { "PreToolUse": [
        { "matcher": "Bash", "hooks": [ { "command": "a" }, { "command": "b" } ] }
    ] } });
    let template = json!({ "hooks": { "PreToolUse": [
        { "matcher": "Bash", "hooks": [ { "command": "a" }, { "command": "c" } ] },
        { "matcher": "Read", "hooks": [ { "command": "d" } ] }
    ] } });

    let (merged, changed) = merge_json(existing, &template);

    assert!(changed);
    assert_eq!(
        merged["hooks"]["PreToolUse"],
        json!([
            { "matcher": "Bash", "hooks": [ { "command": "a" }, { "command": "b" }, { "command": "c" } ] },
            { "matcher": "Read", "hooks": [ { "command": "d" } ] }
        ]),
        "one Bash entry holding every command once, plus the new Read entry"
    );

    let (again, changed_again) = merge_json(merged.clone(), &template);
    assert!(!changed_again);
    assert_eq!(again, merged);
}

#[test]
fn toml_hook_entries_with_the_same_matcher_are_merged_not_duplicated() {
    let existing = toml_value(
        "[[hooks.PreToolUse]]\nmatcher = \"Bash\"\n\n  [[hooks.PreToolUse.hooks]]\n  command = \"a\"\n\n  [[hooks.PreToolUse.hooks]]\n  command = \"b\"\n",
    );
    let template = toml_value(
        "[[hooks.PreToolUse]]\nmatcher = \"Bash\"\n\n  [[hooks.PreToolUse.hooks]]\n  command = \"a\"\n\n  [[hooks.PreToolUse.hooks]]\n  command = \"c\"\n\n[[hooks.PreToolUse]]\nmatcher = \"Read\"\n\n  [[hooks.PreToolUse.hooks]]\n  command = \"d\"\n",
    );

    let (merged, changed) = merge_toml(existing, &template);

    assert!(changed);
    let entries = merged["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "one Bash entry plus the new Read entry: {entries:?}");
    let bash_commands: Vec<&str> = entries[0]["hooks"].as_array().unwrap().iter().map(|hook| hook["command"].as_str().unwrap()).collect();
    assert_eq!(bash_commands, ["a", "b", "c"]);
    assert_eq!(entries[1]["matcher"].as_str(), Some("Read"));

    let (again, changed_again) = merge_toml(merged.clone(), &template);
    assert!(!changed_again);
    assert_eq!(again, merged);
}
