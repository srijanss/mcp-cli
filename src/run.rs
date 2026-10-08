/// The line `run` writes to stderr when started from a terminal, so a server silently waiting on stdin is not
/// taken for a hang.
pub fn terminal_notice(name: &str, version: &str) -> String {
    format!("{name}@{version} running on stdio, waiting for an MCP client (Ctrl-C to stop)")
}
