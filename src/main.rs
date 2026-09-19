fn main() {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [flag] if flag == "--version" => println!("mcpctl 0.1.0"),
        [flag] if flag == "--help" || flag == "-h" => println!(
            "Usage: mcpctl <COMMAND>\n\nCommands:\n  install\n  run\n  list\n  info\n  uninstall\n  use\n  update\n  doctor"
        ),
        [] => {
            eprintln!("Usage: mcpctl <COMMAND>");
            std::process::exit(2);
        }
        _ => {
            eprintln!("unrecognized argument");
            std::process::exit(2);
        }
    }
}
