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
mcpctl uninstall <name>@<version>
mcpctl doctor
mcpctl --version
```

`install` records a version without automatically selecting it when another
version is active. Use `mcpctl use name@version` to change the default.
`run name@version` runs a particular installed version without changing that
default. `update` installs from the supplied source and selects its manifest
version.

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
