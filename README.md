# mcpctl

`mcpctl` installs and runs local Model Context Protocol (MCP) servers with a
runtime owned by the server, rather than by the project that happens to use
it. It is useful when two MCPs need incompatible Python versions or dependency
versions, or when an MCP is a native executable.

## Why mcpctl

An MCP client needs a stable command to launch each server. `mcpctl` provides
that command while keeping each installed server and its runtime isolated. A
client launches `mcpctl run <package>`; `mcpctl` selects the active installed
version and replaces itself with the server process.

## Install

Build from this checkout with Rust:

```sh
cargo build --release
install -m 755 target/release/mcp-cli /usr/local/bin/mcpctl
mcpctl --version
```

`mcpctl --version` reports the version embedded in the binary. Release
artifacts use stable platform names and include a SHA-256 checksum manifest;
verify a downloaded artifact with `shasum -a 256 -c SHA256SUMS` on macOS or
`sha256sum -c SHA256SUMS` on Linux before installing it.

## MCP runtime vs project runtime

`mcpctl` stores installed packages below its data home:

```text
$MCPCTL_HOME/packages/<name>/<version>/source
$MCPCTL_HOME/packages/<name>/<version>/runtime
$MCPCTL_HOME/registry.json
```

Set `MCPCTL_HOME` to choose that location; otherwise the platform data-home
location is used. The MCP's `runtime` belongs under this layout. The target
project's virtual environment belongs to the target project and is not used
to install or execute MCPs.

Do not share `/workspace/.venv` between MCP runtimes. A shared environment
lets one MCP upgrade or constrain dependencies used by another and makes the
result depend on installation order. Keep `/workspace/.venv` for the target
application, and let `mcpctl` create one runtime per installed MCP version.

## Configuration (`mcpctl.toml`)

Put `mcpctl.toml` at the root of every installable MCP project. Names may use
letters, numbers, hyphens, and underscores; versions are semantic versions.

Python MCP:

```toml
name = "notes-mcp"
version = "1.0.0"
description = "Notes tools"

[runtime]
type = "python"
python = ">=3.12"

[install]
strategy = "uv"
entrypoint = "notes-mcp"
```

Native binary MCP:

```toml
name = "search-mcp"
version = "1.0.0"

[runtime]
type = "binary"

[install]
entrypoint = "search-mcp"
```

For a binary MCP, `entrypoint` must be an executable filename in the source
project, not a path containing `/` or `\\`.

## Python MCPs and uv

Python installation requires [`uv`](https://docs.astral.sh/uv/). `mcpctl
install` snapshots the MCP source, asks `uv` to create the declared Python
runtime under the package's `runtime` directory, and installs that snapshot
into the isolated runtime.

This makes it safe to install two servers that require different Python
versions or incompatible package versions:

```sh
MCPCTL_HOME=/state mcpctl install ./legacy-mcp
MCPCTL_HOME=/state mcpctl install ./modern-mcp
mcpctl list
mcpctl run legacy-mcp
```

## Native binary MCPs

For `runtime.type = "binary"`, place the executable named by `entrypoint` in
the MCP project, mark it executable, and run `mcpctl install ./search-mcp`.
`mcpctl` copies it into that version's isolated runtime and runs it directly.

## Command reference

```text
mcpctl install <local-project-path>
mcpctl run <name>[@<version>] [server-arguments...]
mcpctl list
mcpctl info <name>
mcpctl use <name>@<version>
mcpctl update <name> --source <local-project-path>
mcpctl init <name>[@<version>] [target-directory]
mcpctl uninstall <name>@<version>
mcpctl doctor
mcpctl --version
```

`install` records a version without automatically selecting it when another
version is active. Use `mcpctl use name@version` to change the default.
`run name@version` runs a particular installed version without changing that
default. `update` installs from the supplied source and selects its manifest
version.

## Shipping project files (`[scaffold]` and `mcpctl init`)

An MCP often needs files in every project that uses it: an `.mcp.json`,
agent instructions, hooks, a config template. Declare them in the MCP's own
`mcpctl.toml` and let users copy them out of the installed package:

```toml
[scaffold]
dirs = [".agents", ".claude"]                  # copied recursively, same path in the project
exclude = [".claude/settings.local.json"]      # source paths skipped by the `dirs` copies
requires = ["jq"]                              # executables the files rely on; init warns if missing from PATH

[[scaffold.files]]                             # explicit source -> destination mapping
from = ".mcp.json.example"
to = ".mcp.json"
merge = "json"                                 # optional: merge into an existing file instead of skipping it

[[scaffold.files]]
from = ".codex/config.toml.example"
to = ".codex/config.toml"
merge = "toml"

[[scaffold.hints]]                             # printed after the copy
message = "Defaults to pytest."

[[scaffold.hints]]                             # only printed when the marker exists in the project
when_exists = "Cargo.toml"
message = "Rust project detected: use cargo-adapter-runner."
```

```sh
cd /path/to/project
mcpctl init notes-mcp                # active version, into the current directory
mcpctl init notes-mcp@1.0.0 ./app    # a specific version, into ./app
```

- Files are copied from the **installed** package, not from the checkout it
  was installed from. Change a template, bump the version, and run
  `mcpctl update`; projects then get the new files the next time they run
  `init`.
- `init` never overwrites: a file that already exists in the project is
  reported as `Skipping <path> (already exists)`, so it is safe to re-run.
- **Merging shared files.** Several MCPs often contribute to the same file
  (`.mcp.json`, `.claude/settings.json`, `.codex/config.toml`), and a
  skipped file would leave the second MCP unconfigured. With
  `merge = "json"` or `"toml"`, an existing destination is merged with the
  template instead of skipped: objects/tables merge key by key, arrays gain
  the items they lack (compared by value), and **existing values always
  win**. Nothing is removed or rewritten, and key order is kept. Re-running
  is a no-op (`Unchanged <path> (already up to date)`); a change prints
  `Merged <path>`. A destination that does not parse aborts with an error
  and is left untouched. A missing destination is simply copied.
- **Hook entries merge by `matcher`.** In arrays of hook registrations
  (`hooks.PreToolUse` and friends), an item whose `matcher` already exists is
  merged into that entry, so its `hooks` list gains only the missing
  commands instead of a second entry for the same matcher (which would run
  each hook twice). Keep one entry per matcher in your template.
- TOML is re-serialized when it is merged, so **comments in an existing
  TOML file are not preserved** (JSON has none).
- `exclude` only filters `dirs`; a file named in `files` is always copied.
  This lets an MCP keep its own development config next to a
  consumer-facing `.example` copy of it.
- `requires` lists bare executable names (no path separators). After the
  copy, `init` prints `WARNING: <tool> is required by <mcp> but was not found
  on PATH` for each one it cannot find. It only warns: files are still
  copied and the exit status stays 0.
- Every scaffold path must be relative and must not contain `..` (the
  manifest is rejected otherwise). `hints` are plain messages, so mcpctl
  itself stays independent of any one MCP's languages or tools.

## stdio forwarding rules

MCP transports commonly use standard input and output. `mcpctl run` executes
the selected server in place: it forwards its arguments, preserves the
caller's working directory and ordinary environment, and preserves stdin,
stdout, stderr, and the server's exit status. It deliberately removes
`VIRTUAL_ENV` and `PYTHONHOME` so a caller's Python environment cannot leak
into an installed MCP runtime. Do not print logs to stdout in an MCP stdio
server; use stderr for diagnostics.

## MCP client configuration

Here is an example `.mcp.json` entry. The absolute `mcpctl` path avoids PATH
differences between GUI clients and a login shell.

```json
{
  "mcpServers": {
    "notes": {
      "command": "/usr/local/bin/mcpctl",
      "args": ["run", "notes-mcp"]
    }
  }
}
```

The same command-and-args shape applies to Claude desktop configuration,
OpenCode configuration, and Pi configuration where those clients accept a
local stdio MCP command. Use `mcpctl run notes-mcp@1.0.0` when a client must
be pinned to a particular installed server version.

## Docker

The repository includes an isolation proof that installs two Python MCPs with
different dependency versions in a container:

```sh
docker build --target isolation-proof -t mcpctl-isolation .
docker run --rm mcpctl-isolation
```

For your own container, install `mcpctl` and `uv`, set an explicit
`MCPCTL_HOME` such as `/state`, install the MCP projects during image build or
container setup, then configure the client to invoke `mcpctl run <name>`.

## macOS

On macOS, install a release binary appropriate for your architecture, make it
executable, and place it on PATH (for example `/usr/local/bin/mcpctl`). GUI
MCP clients may not inherit your shell PATH, so use the absolute binary path
in their configuration. Verify the installation with `mcpctl --version` and
`mcpctl doctor` after installing an MCP.

## `mcpctl doctor`

Run `mcpctl doctor` to validate the registry and installed entrypoints. It
reports `OK` only when active versions and runtime entrypoints are healthy;
otherwise it prints `ERROR` lines and exits unsuccessfully.

## Troubleshooting

- `uv is required`: install `uv` and ensure it is available on PATH before
  installing a Python MCP.
- `cannot read .../mcpctl.toml`: pass the local MCP project directory to
  `mcpctl install`, not an individual file.
- `<name> is not installed` or `has no active version`: run `mcpctl list`,
  then install the package or select one with `mcpctl use name@version`.
- A client cannot start `mcpctl`: use an absolute path in its configuration
  and ensure the executable bit is set.
- An MCP sees the wrong Python packages: remove any shared runtime setup,
  confirm `MCPCTL_HOME`, reinstall the MCP, and keep the target project's
  `.venv` separate from the MCP runtime.
