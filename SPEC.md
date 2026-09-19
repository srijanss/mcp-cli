# mcpctl — SPEC

Portable Rust CLI for installing, isolating, versioning, launching, and diagnosing MCP servers across macOS, Linux, Docker sandboxes, and other development environments.

The primary goal is to remove MCP runtime/dependency conflicts from AI coding environments. Each MCP server should run in its own isolated runtime instead of sharing the target project's `.venv` or the sandbox's system Python.

Typical usage:

```text
mcpctl install ./project-mcp
mcpctl install ./outside-in-tdd-mcp

mcpctl run project-mcp
mcpctl run outside-in-tdd-mcp
```

AI clients then configure MCP servers through the stable `mcpctl run <name>` interface rather than hardcoding interpreter/venv paths.

This spec is split into MVPs so they can be worked on independently by separate agents. Each MVP has its own scope, files, dependencies, and acceptance criteria.

**Read the whole "Shared context" section before starting any MVP.**

---

# Shared context (applies to every MVP)

## Purpose

`mcpctl` is an **installer + runtime manager + launcher** for MCP servers.

It is not itself an MCP server.

It must not contain TDD logic, project-analysis logic, deployment logic, business-domain logic, or prompt orchestration.

Core principle:

> MCP runtimes and target-project runtimes are separate concerns.

Example:

```text
/workspace/.venv
    |
    +-- target Django/Python project runtime

~/.local/share/mcpctl/packages/outside-in-tdd-mcp/...
    |
    +-- isolated MCP runtime

~/.local/share/mcpctl/packages/project-mcp/...
    |
    +-- isolated MCP runtime
```

The TDD MCP may still invoke the target project's test runtime:

```text
/workspace/.venv/bin/pytest
pnpm vitest
cargo test
```

but its own MCP server process runs from its isolated `mcpctl` installation.

## Primary goals

1. Provide one portable `mcpctl` binary.
2. Install each MCP into an isolated runtime.
3. Support different Python versions/dependency sets between MCPs.
4. Avoid mounting/sharing `/workspace/.venv` for MCP runtimes.
5. Provide a stable launch command: `mcpctl run <mcp-name>`.
6. Work consistently on macOS and Linux/Docker.
7. Support versioned MCP installs.
8. Make installed MCPs inspectable and diagnosable.
9. Preserve stdio transparently for MCP clients.
10. Remain runtime-agnostic enough to support native binaries and possibly Node-based MCPs later.

## Non-goals

`mcpctl` is not:

- an MCP orchestration engine
- an AI agent
- a package registry in the first MVPs
- a Python package manager replacement
- a Node package manager replacement
- a test runner
- a deployment system
- a workflow engine
- a daemon
- a container runtime
- an MCP proxy
- an HTTP gateway
- a tool that edits target projects
- a tool that manages the target project's `.venv`

## Initial platform targets

Required:

```text
macOS ARM64
Linux x86_64
Linux ARM64
Docker/Linux sandboxes
```

Windows is deferred unless a concrete need appears.

## Implementation stack

Language:

```text
Rust
```

Suggested initial crates:

```text
clap          CLI parsing
serde         data structures
serde_json    registry/state
toml          manifests/config
semver        version handling
directories   platform-specific data/config paths
thiserror     typed errors
sha2          artifact/hash verification later
tempfile      safe temporary directories
```

Add `reqwest` and `tokio` only when remote artifact downloading is introduced.

Do not make the initial CLI async unless required.

---

# Directory layout

## mcpctl source repository

Fixed initial layout:

```text
mcpctl/
  Cargo.toml
  Cargo.lock
  README.md

  src/
    main.rs
    cli.rs
    error.rs

    config.rs
    paths.rs
    registry.rs
    manifest.rs

    install/
      mod.rs
      local.rs
      python.rs
      binary.rs

    runtime/
      mod.rs
      python.rs
      binary.rs

    commands/
      mod.rs
      install.rs
      run.rs
      list.rs
      info.rs
      uninstall.rs
      use_cmd.rs
      update.rs
      doctor.rs

    process/
      mod.rs
      stdio.rs

  tests/
    fixtures/
      python-mcp/
      binary-mcp/
      broken-mcp/
    integration/
```

Do not restructure without a strong reason.

---

# Installation data layout

Use platform-appropriate data directories.

Conceptually:

```text
~/.local/share/mcpctl/
  registry.json

  packages/
    project-mcp/
      0.4.0/
        manifest.toml
        runtime/
        metadata.json

      0.4.1/
        manifest.toml
        runtime/
        metadata.json

    outside-in-tdd-mcp/
      1.2.0/
        manifest.toml
        runtime/
        metadata.json

  active/
    project-mcp.json
    outside-in-tdd-mcp.json

  cache/
```

On macOS, use the platform data-directory abstraction rather than hardcoding `~/.local/share`.

Allow an environment override for Docker/controlled environments:

```text
MCPCTL_HOME=/opt/mcpctl
```

If unset, use the OS-appropriate data directory.

---

# Package manifest

Each MCP package should include an `mcpctl.toml` manifest.

Initial schema:

```toml
name = "project-mcp"
version = "0.4.0"
description = "Project knowledge MCP"

[runtime]
type = "python"
python = ">=3.12,<3.13"

[install]
strategy = "uv"
entrypoint = "project-mcp"

[project]
requires_workspace = true
workspace_env = "PROJECT_ROOT"
```

Example native binary MCP:

```toml
name = "example-rust-mcp"
version = "1.0.0"

[runtime]
type = "binary"

[install]
entrypoint = "example-rust-mcp"
```

Rules:

- `name` is the stable MCP identity.
- `version` must be valid semver.
- Runtime-specific config belongs under `[runtime]`.
- Install strategy belongs under `[install]`.
- Workspace/project hints are optional.
- `mcpctl` must not infer workflow semantics from the manifest.

---

# Supported runtime types

Initial:

```text
python
binary
```

Deferred:

```text
node
container
remote-http
```

---

# Python runtime model

`mcpctl` should use `uv` as the implementation backend for Python runtime creation/install.

`mcpctl` is responsible for:

```text
reading manifest
selecting install path
invoking uv
checking result
recording metadata
launching executable
```

`uv` is responsible for:

```text
Python acquisition
virtual environment creation
dependency installation/resolution
package installation
```

Do not reimplement Python package resolution in Rust.

Each Python MCP gets a separate runtime directory.

Example:

```text
packages/project-mcp/0.4.0/runtime/.venv
packages/outside-in-tdd-mcp/1.2.0/runtime/.venv
```

They must never share a `.venv`.

---

# Target-project runtime separation

This distinction is mandatory.

Example:

```text
/workspace/
  .venv/                  # target project
  node_modules/           # target project
  Cargo.toml              # target project
  src/
  tests/
```

Versus:

```text
MCPCTL_HOME/
  packages/
    outside-in-tdd-mcp/
      1.2.0/
        runtime/.venv/    # MCP server runtime
```

The TDD MCP may run:

```text
/workspace/.venv/bin/pytest
pnpm vitest
cargo test
```

because those commands belong to the target project's adapters.

`mcpctl` must not manage those project runtimes.

---

# CLI commands

Target command set:

```text
mcpctl install <source>
mcpctl uninstall <name>[@version]
mcpctl list
mcpctl info <name>[@version]
mcpctl run <name>[@version] [-- <extra args>]
mcpctl use <name>@<version>
mcpctl update <name>
mcpctl doctor
```

The first MVPs do not need all commands.

---

# MCP client integration

The intended stable client configuration is:

```json
{
  "mcpServers": {
    "project": {
      "command": "mcpctl",
      "args": ["run", "project-mcp"]
    },
    "tdd": {
      "command": "mcpctl",
      "args": ["run", "outside-in-tdd-mcp"]
    }
  }
}
```

The MCP client should not need to know Python version, venv path, uv path, package installation path, or MCP executable path.

---

# Process / stdio behavior

For stdio MCP servers, `mcpctl run` must preserve:

```text
stdin
stdout
stderr
exit code
signals
```

Important:

- Never print informational logs to stdout during `mcpctl run`.
- stdout belongs to the MCP protocol.
- `mcpctl` diagnostics during `run` must go to stderr.
- On Unix, process replacement (`exec`) may be used where appropriate.
- Ctrl+C / termination should reach the child process.
- Exit status should reflect child process failure where possible.

This is a critical correctness requirement.

---

# Shared design rules

- Every installed MCP is isolated from other MCP runtimes.
- Never install MCP dependencies into the target project's `.venv`.
- Never install two MCPs into a shared Python environment.
- `mcpctl` should be deterministic and non-interactive by default.
- Expected user errors should have clear messages and non-zero exit codes.
- Internal errors should not leave half-installed active entries.
- Installation should use temporary directories and finalize atomically where practical.
- `registry.json` must not become the sole source of truth for an installation; installed metadata should also exist with the package/version directory.
- Deleting an MCP installation must not modify the target project.
- `run` must never silently choose a different MCP version than the active or explicitly requested version.
- Runtime-specific code belongs behind runtime/install abstractions.
- Keep Python/uv logic out of generic command code.
- `mcpctl` must not require Python itself.
- Do not spawn Docker from `mcpctl` in early MVPs.
- Do not mutate AI-client configuration automatically in early MVPs.
- Do not create background services/watchers.

---

# Resolved implementation decisions

These rules refine the shared context and are normative for all applicable MVPs.

## Local Python install snapshots

Python installs are self-contained snapshots. `mcpctl install <local-path>` must:

1. validate the local source package;
2. copy a normalized source snapshot to a temporary package directory;
3. create the isolated runtime from that snapshot; and
4. atomically promote the completed directory into `packages/<name>/<version>/`.

The installed MCP must remain runnable if the original checkout is changed, moved, or deleted.

The snapshot is stored at:

```text
packages/<name>/<version>/source/
```

Do not copy VCS metadata, `.venv`, Python caches, ordinary build artifacts, or `.env` files. Preserve relative symlinks only when their resolved targets remain within the source root; reject symlinks that escape the source root. Package configuration and secrets must be supplied by the caller's environment, not persisted from a local `.env` file. Templates such as `.env.example` may be included.

Python installation runs `uv sync --frozen --no-dev` from the snapshot and directs the resulting environment to:

```text
packages/<name>/<version>/runtime/.venv/
```

All MCP runtime dependencies must be normal runtime dependencies, not development dependencies. `uv` must be present on `PATH` for install and update, but is not required for run, list, info, or uninstall after installation.

## Installed state and atomicity

Installed state has three layers:

```text
packages/<name>/<version>/metadata.json   authoritative record for one installed version
active/<name>.json                        authoritative active-version selection
registry.json                             indexed convenience view
```

`registry.json` must never be the only source of truth. A corrupt registry produces a clear error and must not be silently overwritten.

Mutating commands (`install`, `uninstall`, `use`, and `update`) take one exclusive lock per `MCPCTL_HOME`. Build temporary installation directories beneath the final package parent so promotion is an atomic same-filesystem rename. Write registry and active-selection files via temporary files followed by atomic rename. Clean temporary directories on ordinary failures; leave directories from interrupted processes for `doctor` to report, rather than deleting them automatically.

`run` holds a shared state lock until it has resolved and replaced itself with the entrypoint. v1 does not reliably track an already running MCP after replacement; uninstalling such a version is a documented limitation rather than a reason to introduce a daemon.

## Versioning and activation

An MCP name is a lowercase ASCII filesystem/CLI identifier matching:

```text
[a-z0-9][a-z0-9._-]*
```

Reject whitespace, path separators, `@`, uppercase characters, and path-like names. MCP versions must be canonical SemVer. Prereleases such as `1.0.0-rc.1` are allowed; SemVer build metadata such as `1.0.0+local` is rejected because it would make installed-version identity ambiguous.

`<name>@<version>` always selects one exact installed version. v1 has no version ranges, partial versions, or `latest` selector.

Installing a version activates it only if that MCP has no active version yet. Installing later versions never changes the active selection. `mcpctl use <name>@<version>` is the only normal way to switch it. An explicit `mcpctl run <name>@<version>` never changes it.

Installed versions are immutable. Reinstalling an existing `<name>@<version>` fails even if its original local source changed; changed code must have a new manifest version.

Uninstall behavior:

- uninstalling a non-active version removes it;
- uninstalling the sole installed active version removes it and clears its active selection;
- uninstalling an active version while other versions remain refuses and instructs the user to select another version first with `mcpctl use`.

## Launch behavior and environment

On Unix, `mcpctl run` should replace itself with the installed entrypoint using `exec` semantics. It must preserve stdin, stdout, stderr, working directory, forwarded arguments, signals, and exit status without routing execution through a shell. Arguments after `--` are passed verbatim as an argument vector.

`run` emits no launcher output on stdout. All validation failures and diagnostics go to stderr. Other human-facing commands may use stdout for normal results; errors always use stderr. Any future structured logging is opt-in and stderr-only.

The child receives the caller's ordinary environment and workspace variables. Before launching, clear `VIRTUAL_ENV` and `PYTHONHOME`, preserve `PYTHONPATH`, and prepend the selected MCP runtime's bin directory to `PATH`. The installed entrypoint itself is launched by absolute path. A future strict environment-cleaning option may additionally clear `PYTHONPATH`.

## Metadata and trust boundary

Each `metadata.json` records at least:

```text
schema_version
name
version
runtime_type
installed_at
mcpctl_version
source_path
source_tree_sha256
manifest_sha256
lockfile_sha256             # Python only
uv_version                  # Python only
entrypoint_relative_path
```

The source-tree hash is calculated deterministically from normalized relative paths and file contents. For binary runtimes, metadata also records the executable checksum and detected platform/architecture when available.

Local installation treats a package as trusted code: dependency installation may execute package build hooks, and the installed MCP later runs with the invoking user's permissions. Runtime isolation is not a security sandbox.

`MCPCTL_HOME`, when set, must be an absolute writable path. Relative overrides are rejected. If unset, use the OS-appropriate platform data directory.

## Future native binary runtime

The future `binary` runtime uses an immutable copied executable payload rather than a Python runtime. Its local manifest declares a source-relative executable artifact, for example:

```toml
[runtime]
type = "binary"

[install]
strategy = "copy"
entrypoint = "dist/example-rust-mcp"
```

The initial binary MVP validates that the file exists, is a regular executable, and is host-compatible when its format can be identified. It does not execute the binary merely to validate installation and does not compile Rust source. A source-build workflow is a later explicit install strategy.

## Test and doctor policy

Automated `run` acceptance tests use a fixture MCP that performs a minimal MCP/JSON-RPC initialization exchange and verify clean stdout, stderr passthrough, arguments, working directory, and child exit/signal behavior. Client launches through Claude, OpenCode, and Pi are documented manual smoke tests in addition to that automated coverage.

`doctor` only diagnoses; it does not repair or delete state. Its exit status is `0` when there are no `ERROR` findings (including when there are `WARN` findings) and `1` when at least one `ERROR` is reported.

---

# Deferred / out of scope

Do not build these unless a later MVP explicitly adds them:

- hosted MCP package registry
- GitHub/GitLab registry integration
- automatic client config editing
- MCP discovery marketplace
- auto-update daemon
- dependency vulnerability scanning
- signing infrastructure
- containerized MCP runtime
- Node runtime support
- Windows support
- remote HTTP MCP proxying
- team-wide centralized package policy
- sandbox/security policy engine
- automatic workspace/project detection from arbitrary clients
- per-project MCP lockfile
- MCP dependency graph between servers

---

# MVP 0 — Rust CLI skeleton and platform paths

**Depends on:** nothing.

## Scope

Create the Rust project layout.

Implement:

```text
mcpctl --version
mcpctl --help
```

Implement platform path resolution.

Support `MCPCTL_HOME`.

If unset, use platform-specific data directory.

Create `registry.json`, `packages/`, `active/`, and `cache/` only when needed.

No installation logic yet.

## Acceptance criteria

- builds on macOS ARM64
- builds on Linux x86_64
- builds on Linux ARM64 or cross-build strategy is documented
- `mcpctl --help` works
- `MCPCTL_HOME=/tmp/test-mcpctl` redirects all mutable state there
- no Python runtime is required to execute `mcpctl`

---

# MVP 1 — Manifest parser and registry

**Depends on:** MVP 0.

## Scope

Implement `mcpctl.toml` parsing.

Validate `name`, `version`, `runtime.type`, and `install.entrypoint`.

For Python manifests also validate `runtime.python` and `install.strategy`.

Implement registry structures for installed MCP name, installed versions, active version, runtime type, installation path, installed timestamp, and source.

Add internal schema version to registry.

Do not add `install` command behavior yet beyond fixture parsing if needed.

## Acceptance criteria

- valid Python manifest parses
- valid binary manifest parses
- invalid semver fails clearly
- unsupported runtime type fails clearly
- registry persists and reloads
- corrupt registry returns a clear diagnostic instead of silently overwriting

---

# MVP 2 — Local Python MCP installation using uv

**Depends on:** MVP 1.

This MVP solves the immediate current problem.

## Command

```text
mcpctl install ./path/to/python-mcp
```

## Scope

For a local package containing `mcpctl.toml`, `pyproject.toml`, and `uv.lock`:

```text
1. read manifest
2. verify runtime.type = python
3. verify uv exists
4. create isolated version install directory
5. create isolated Python environment using uv
6. install/sync MCP package dependencies
7. verify configured entrypoint exists
8. write installation metadata
9. mark version active if this is first installed version
```

Prefer lockfile-respecting/frozen installation.

If the package requires a Python version unavailable locally, allow uv to acquire/manage it according to normal uv behavior.

Do not use `/workspace/.venv`, current shell virtualenv, or system pip as the MCP environment.

## Failure behavior

If installation fails:

- return non-zero
- show concise error
- preserve useful stderr from uv
- do not mark version installed/active
- clean incomplete temporary installation

## Acceptance criteria

- install two fixture MCPs requiring different Python versions
- each gets a different isolated `.venv`
- both can coexist
- installation does not modify target project `.venv`
- re-installing exact same version returns a clear already-installed result unless `--force` is explicitly added later
- broken uv install does not leave an active registry entry

---

# MVP 3 — Run installed Python MCP with stdio passthrough

**Depends on:** MVP 2.

## Command

```text
mcpctl run <name>
mcpctl run <name>@<version>
```

## Scope

Resolve explicit version, otherwise active version.

Launch the installed entrypoint from that MCP's isolated runtime.

Forward stdin, stdout, stderr, environment, working directory, signals, and exit status.

Do not alter workspace/project environment variables supplied by the caller.

## Critical stdout rule

`mcpctl run` must emit **no launcher/progress text to stdout**.

Any `mcpctl` diagnostic must use stderr.

## Acceptance criteria

- MCP stdio handshake works through `mcpctl run`
- Claude/OpenCode/Pi can launch a fixture MCP through `mcpctl`
- Ctrl+C terminates child correctly
- child stderr remains visible
- child exit code is propagated
- selecting explicit version runs that version
- missing active version returns a clear error

---

# MVP 4 — list, info, and uninstall

**Depends on:** MVP 2–3.

## Commands

```text
mcpctl list
mcpctl info <name>
mcpctl info <name>@<version>
mcpctl uninstall <name>@<version>
```

## `list`

Return a human-readable table with name, active version, installed versions, and runtime.

Optional future machine output `--json` may be added in this MVP if trivial.

## `info`

Show name, version, runtime, source, install path, entrypoint, active status, Python constraint if applicable, and installed timestamp.

Never print secrets from package environment/config.

## `uninstall`

Rules:

- remove only selected installed version
- if removing active version and others remain, do not guess silently; either require selection or choose a documented deterministic policy
- never delete target project files
- refuse to uninstall a package version currently being used if reliable detection exists; otherwise document limitation

## Acceptance criteria

- list shows two separately installed MCPs
- info shows correct isolated runtime
- uninstall removes only selected version
- registry remains consistent
- target workspace remains untouched

---

# MVP 5 — Version selection

**Depends on:** MVP 4.

## Commands

```text
mcpctl use <name>@<version>
mcpctl run <name>@<version>
```

## Scope

Support multiple installed versions.

`use` changes the active version.

`run name@version` never changes active version.

## Acceptance criteria

- two versions coexist
- switching active version is persistent
- explicit `run name@old-version` works without changing active
- selecting unknown version fails cleanly

---

# MVP 6 — Native binary MCP installation/runtime

**Depends on:** MVP 1, 3–5.

## Goal

Prove `mcpctl` is not Python-specific.

## Initial source

Local directory/package only.

## Scope

Install/copy native executable into versioned package directory.

Validate that file exists, executable permission exists on Unix, and current host can execute it.

Run through the same `mcpctl run <name>` interface.

## Acceptance criteria

- binary fixture installs
- binary fixture launches
- list/info identify runtime as binary
- no Python/uv is required for binary runtime

---

# MVP 7 — doctor

**Depends on:** MVP 2–6.

## Command

```text
mcpctl doctor
```

## Scope

Check:

```text
mcpctl data directory writable
registry readable
active-version references valid
installed package paths exist
entrypoints exist
Python MCP environments exist
uv available where needed for install/update
installed Python entrypoints executable
binary entrypoints executable
orphaned registry/package entries
```

Output `OK`, `WARN`, or `ERROR`.

Doctor should diagnose only.

Do not auto-repair in this MVP.

## Acceptance criteria

- detects deleted venv
- detects missing binary
- detects broken active-version reference
- healthy setup returns success
- secrets/config values are not printed

---

# MVP 8 — Update from local source

**Depends on:** MVP 5.

Before remote registries, support local development workflow.

## Command

Possible form:

```text
mcpctl update project-mcp --source ./project-mcp
```

Choose one consistent interface during implementation.

## Scope

Install new version alongside old version.

Do not overwrite an installed version in place by default.

After successful install, optionally make new version active and keep previous version for rollback.

Failure must leave previous active version untouched.

## Acceptance criteria

- 0.4.0 remains runnable after installing 0.5.0
- failed 0.5.0 install leaves 0.4.0 active
- successful update can set 0.5.0 active
- rollback via `mcpctl use` works

---

# MVP 9 — Docker/sandbox validation

**Depends on:** MVP 2–8.

## Goal

Validate the original use case.

Test a sandbox with:

```text
/workspace
  target project

/opt/mcpctl
  isolated MCP packages
```

Use:

```text
MCPCTL_HOME=/opt/mcpctl
```

Install `outside-in-tdd-mcp` and `project-mcp` with intentionally different Python constraints/dependencies.

Do **not** mount `/workspace/.venv` as an MCP runtime.

The target project may still have `/workspace/.venv` for its own pytest/runtime needs.

## Acceptance criteria

- both MCPs run in the same sandbox
- each uses its own interpreter/environment
- TDD MCP can still invoke target-project pytest/vitest/cargo adapters
- Project MCP can inspect `/workspace`
- MCP dependency versions do not conflict
- rebuilding sandbox from documented steps is reproducible

---

# MVP 10 — macOS validation

**Depends on:** MVP 2–8.

## Scope

Install/use the same MCP packages on macOS ARM64.

Validate `install`, `list`, `info`, `run`, `use`, and `doctor`.

Ensure paths use platform data directories correctly.

## Acceptance criteria

- no hardcoded Linux paths required
- same `mcpctl run <name>` client config pattern works
- Python MCP versions remain isolated
- native `mcpctl` binary runs without Python

---

# MVP 11 — Release packaging for mcpctl

**Depends on:** stable MVP 0–10.

## Scope

Produce release artifacts:

```text
mcpctl-macos-arm64
mcpctl-linux-x86_64
mcpctl-linux-arm64
```

Use CI release builds.

Add `mcpctl --version` with build/version metadata.

Generate SHA-256 checksums.

Signing is deferred unless needed.

## Acceptance criteria

- clean macOS host can run released binary
- clean Linux/Docker host can run matching released binary
- SHA-256 checksums verify
- artifact names are stable/documented

---

# MVP 12 — README and handoff docs

**Depends on:** MVP 0–11 as implemented.

## Scope

Document:

- why `mcpctl` exists
- MCP-runtime vs project-runtime separation
- install layout
- `MCPCTL_HOME`
- `mcpctl.toml`
- Python runtime/uv model
- native binary model
- command reference
- versioning
- stdio forwarding rules
- Docker setup
- macOS setup
- example `.mcp.json`
- Pi/OpenCode/Claude integration examples where applicable
- troubleshooting
- doctor usage
- why `/workspace/.venv` should not host multiple MCP runtimes

## Acceptance criteria

A developer with no prior context can:

1. install `mcpctl`
2. install two Python MCPs with different Python versions
3. configure an MCP client using `mcpctl run`
4. run both in Docker or macOS
5. understand why the target project's `.venv` remains separate

using only the README.

---

# Future MVPs — explicitly deferred

## Remote artifact installation

Possible future:

```text
mcpctl install project-mcp@0.5.0
```

from GitHub releases, GitLab releases, or a custom manifest registry.

Requirements before implementation:

- trusted source model
- checksums
- release metadata
- platform/runtime selection
- rollback behavior

## Node runtime support

Possible later runtime:

```toml
[runtime]
type = "node"
node = ">=22"

[install]
strategy = "pnpm"
entrypoint = "..."
```

Do not add until a real MCP requires it.

## Client configuration helpers

Possible future:

```text
mcpctl configure claude
mcpctl configure opencode
mcpctl configure pi
```

Initial `mcpctl` should **not** modify client config automatically.

## Project-local MCP lockfile

Possible future project file:

```text
.mcpctl.lock
```

Example:

```text
project-mcp = 0.5.0
outside-in-tdd-mcp = 1.3.0
```

This could make development environments reproducible across developers and sandboxes.

Do not build until versioned installs are proven useful.

---

# Suggested build order for parallel agents

## Foundation

```text
Agent A -> MVP 0
Agent A -> MVP 1
```

## Immediate problem

Then:

```text
Agent B -> MVP 2 Python install
Agent C -> MVP 3 run/stdio
```

MVP 2 and 3 should be integrated before proceeding broadly.

## Core package management

```text
Agent D -> MVP 4 list/info/uninstall
Agent E -> MVP 5 version selection
Agent F -> MVP 6 binary runtime
```

## Reliability

```text
Agent G -> MVP 7 doctor
Agent H -> MVP 8 local update
```

## Real-environment validation

```text
Agent I -> MVP 9 Docker/sandbox
Agent J -> MVP 10 macOS
```

## Distribution/docs

```text
Agent K -> MVP 11 release packaging
Any agent -> MVP 12 README
```

---

# First practical milestone

Do not build the full package manager before validating the core idea.

The first useful end-to-end milestone is:

```text
mcpctl binary
      |
      +-- install ./outside-in-tdd-mcp
      |
      +-- install ./project-mcp
      |
      +-- each gets isolated uv/Python runtime
      |
      +-- mcpctl run outside-in-tdd-mcp
      |
      +-- mcpctl run project-mcp
      |
      +-- both work in the same Docker sandbox
```

Use intentionally incompatible Python/dependency versions in test fixtures.

Success means:

> Two MCP servers with different Python/runtime requirements can coexist and run through the same stable `mcpctl` interface without sharing or modifying `/workspace/.venv`.

That validates the central reason for building `mcpctl`.

---

# Long-term target

```text
                         mcpctl

             install / version / run / doctor
                          |
          +---------------+----------------+
          |                                |
          v                                v
    Python MCP                        Native MCP
      runtime                           binary
          |                                |
          v                                v
    isolated env                      executable
          |
          +----------------+---------------+
                           |
                           v
                     MCP stdio client
                           |
                           v
                    Claude / Codex /
                   OpenCode / Pi / etc.

Target project remains separate:

/workspace
  source
  tests
  .venv
  node_modules
  Cargo target/runtime state

mcpctl never turns the project's runtime into a shared MCP runtime.
```
