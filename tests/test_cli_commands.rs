use clap::Parser;
use mcp_cli::cli::{Cli, Command};

fn parse(args: &[&str]) -> Command {
    let mut argv = vec!["mcpctl"];
    argv.extend_from_slice(args);
    Cli::try_parse_from(argv).expect("should parse").command
}

#[test]
fn parses_every_command() {
    assert!(matches!(parse(&["list"]), Command::List));
    assert!(matches!(parse(&["doctor"]), Command::Doctor));
    assert!(matches!(parse(&["info", "pkg"]), Command::Info { package } if package == "pkg"));
    assert!(matches!(parse(&["uninstall", "pkg"]), Command::Uninstall { package } if package == "pkg"));
    assert!(matches!(parse(&["use", "pkg@1.0.0"]), Command::Use { selector } if selector == "pkg@1.0.0"));
    assert!(matches!(
        parse(&["update", "pkg", "--source", "./p"]),
        Command::Update { package, source } if package == "pkg" && source == "./p"
    ));
    assert!(matches!(parse(&["init", "pkg"]), Command::Init { package, target } if package.as_deref() == Some("pkg") && target.is_none()));
    assert!(matches!(
        parse(&["init", "pkg", "./proj"]),
        Command::Init { package, target } if package.as_deref() == Some("pkg") && target.as_deref() == Some("./proj")
    ));
    assert!(matches!(
        parse(&["run", "pkg", "--flag", "x"]),
        Command::Run { package, arguments } if package == "pkg" && arguments == ["--flag", "x"]
    ));
}
