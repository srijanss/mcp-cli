# mcpctl — NEXT MVPs (13–27)

Continuation of the existing `mcpctl` roadmap after MVP 12.

This phase evolves `mcpctl` from a machine-level MCP installer/runtime manager
into a **project-scoped MCP environment manager** with reproducible version
selection, multi-MCP setup, GitHub package sources, and fresh-clone bootstrap.

The key architectural rule for this phase is:

> Projects own MCP selection and version resolution.
> `mcpctl` owns installation, isolation, caching, and execution.
> Package sources only provide packages.

A second important rule:

> MCP runtime storage may remain machine-global and deduplicated, while MCP
> selection and version ownership are project-local.

---

# MVP 13 — Project manifest

**Goal:** Move MCP selection/version intent from global machine state into the
project.

Introduce:

```text
.mcpctl.toml
```

Example:

```toml
[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"

[[mcp]]
name = "outside-in-tdd-mcp"
version = "^1.3"
source = "../outside-in-tdd-mcp"

[[mcp]]
name = "design-advisor-mcp"
version = "^0.2"
source = "../design-advisor-mcp"
```

Initially, `source` may support local paths only.

## Project discovery

When a project-scoped command runs, search upward from the current directory:

```text
/work/project/src/foo
       ↓
/work/project/.mcpctl.toml
```

Stop at filesystem root.

Explicit CLI version/source arguments override project configuration where
applicable.

## Acceptance criteria

- project manifest parses
- multiple MCP declarations work
- semantic version constraints parse
- duplicate MCP names are rejected
- invalid local sources fail clearly
- running from a nested directory discovers the project root
- existing machine/global usage still works outside a project

---

# MVP 14 — Project lock file

**Depends on:** MVP 13.

Introduce:

```text
.mcpctl.lock
```

The manifest expresses intent:

```toml
[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"
```

The lock file records exactly what was resolved:

```toml
version = 1

[[mcp]]
name = "project-mcp"
version = "0.4.3"
source = "../project-mcp"
manifest_digest = "sha256:..."
```

Later GitHub entries may contain:

```toml
source = "github:owner/project-mcp"
tag = "v0.4.3"
archive_digest = "sha256:..."
```

## Rules

```text
.mcpctl.toml
= desired dependencies

.mcpctl.lock
= reproducible resolution
```

Do not put mutable machine/runtime paths in the lock file.

The lock file should be suitable for committing to Git.

## Acceptance criteria

- same manifest produces stable lock representation
- lock records exact MCP versions
- lock is portable across machines
- changing an MCP constraint causes a resolution change
- unchanged inputs do not rewrite lock unnecessarily
- malformed lock fails clearly rather than being silently replaced

---

# MVP 15 — Project-aware `run`

**Depends on:** MVP 13–14.

Change:

```bash
mcpctl run project-mcp
```

resolution order to:

```text
1. explicit CLI version
2. project's locked version
3. global active version
4. error
```

Example:

```text
Project A
project-mcp -> 0.4.3

Project B
project-mcp -> 0.5.1
```

Both projects may use the same client configuration:

```json
{
  "command": "mcpctl",
  "args": ["run", "project-mcp"]
}
```

The working directory determines the project and therefore the locked MCP
version.

## Acceptance criteria

- two projects can pin different MCP versions
- switching working directories changes resolved MCP version
- no `mcpctl use` is required for normal project usage
- explicit `name@version` overrides project lock
- outside a project, global active behavior still works
- stdio forwarding behavior remains unchanged

---

# MVP 16 — Project `sync`

**Depends on:** MVP 13–15.

Add:

```bash
mcpctl sync
```

Behavior:

```text
read .mcpctl.toml
       ↓
read/create .mcpctl.lock
       ↓
resolve required MCP versions
       ↓
check shared package store
       ↓
install missing versions
       ↓
verify runtimes
```

Keep these responsibilities distinct:

```text
sync
= dependency/runtime state

init
= project files/configuration
```

## Shared physical store

Continue using the machine-level package store:

```text
$MCPCTL_HOME/packages/
```

Example:

```text
$MCPCTL_HOME/packages/
  project-mcp/
    0.4.3/
    0.5.1/

  outside-in-tdd-mcp/
    1.3.2/
```

Multiple projects may reference the same installed package version.

## Locked mode

Support:

```bash
mcpctl sync --locked
```

Rules:

- use exact versions from `.mcpctl.lock`
- do not resolve newer versions
- fail if manifest and lock disagree
- useful for Docker/CI/reproducible environments

## Acceptance criteria

- fresh machine + manifest/lock installs required MCPs
- already-installed versions are reused
- unrelated installed MCPs are untouched
- target project `.venv` remains untouched
- repeated sync is idempotent
- locked versions are preferred when lock exists
- `--locked` fails rather than mutating an inconsistent lock

---

# MVP 17 — Multi-MCP `init`

**Depends on:** MVP 13 and the existing scaffold engine.

Existing command remains:

```bash
mcpctl init <name>[@version]
```

Add:

```bash
mcpctl init
```

with no MCP argument.

Behavior:

```text
read project manifest
       ↓
for each configured MCP
       ↓
resolve locked/installed version
       ↓
run existing scaffold engine
       ↓
merge shared project/client config safely
```

Conceptually:

```text
mcpctl init

→ init project-mcp
→ init outside-in-tdd-mcp
→ init design-advisor-mcp
```

Do not create a second scaffold implementation.

Reuse existing:

```text
[scaffold]
dirs
files
json merge
toml merge
requires
hints
```

behavior.

## Acceptance criteria

- all project MCPs initialize in one command
- current safe merge semantics are preserved
- repeated `mcpctl init` is idempotent
- MCP without `[scaffold]` does not fail the whole operation
- errors identify the MCP that caused them
- existing single-MCP form still works

---

# MVP 18 — Interactive `setup`

**Depends on:** MVP 13–17.

This becomes the `create-react-app`-style project onboarding command.

Run:

```bash
mcpctl setup
```

Example UI:

```text
Available MCPs

Development

[x] Project MCP
[x] Outside-in TDD MCP
[x] Design Advisor MCP
[ ] Architecture Advisor MCP

Operations

[ ] Deploy MCP

Space: toggle
A: select all
N: select none
Enter: continue
```

After confirmation:

```text
Selected MCPs

  project-mcp
  outside-in-tdd-mcp
  design-advisor-mcp

Creating .mcpctl.toml
Resolving versions
Installing required MCPs
Creating .mcpctl.lock
Applying scaffolds
Checking requirements

Project MCP environment ready.
```

Conceptually:

```text
setup
  =
select
+ write/update project manifest
+ resolve
+ sync
+ init
```

## Re-running setup

If `.mcpctl.toml` already exists:

- existing project MCPs should be pre-selected
- user may add/remove selections
- unchanged dependencies should not reinstall
- removed selections update project intent only
- scaffold removal is deferred

## Non-interactive mode

Support:

```bash
mcpctl setup --all
```

for automated environments.

Profiles are introduced later.

## Acceptance criteria

- selector supports toggle/select-all/select-none
- selections become project dependencies
- required versions are resolved
- runtimes are installed
- scaffolds are initialized
- re-running setup reflects existing project selections
- setup can add/remove dependencies without reinstalling unchanged packages

---

# MVP 19 — Local MCP catalog

**Depends on:** MVP 18.

Do not hardcode MCP names inside Rust.

Introduce a local catalog.

Example:

```toml
# platform config directory / mcpctl / catalog.toml

[[mcp]]
source = "/Users/me/code/project-mcp"

[[mcp]]
source = "/Users/me/code/outside-in-tdd-mcp"

[[mcp]]
source = "/Users/me/code/design-advisor-mcp"

[[mcp]]
source = "/Users/me/code/architecture-advisor-mcp"
```

`mcpctl` reads each source package's existing:

```text
mcpctl.toml
```

to discover:

```text
name
version
description
runtime
scaffold capability
metadata
```

## Commands

```bash
mcpctl catalog list
mcpctl catalog add ../project-mcp
mcpctl catalog remove project-mcp
```

## Acceptance criteria

- adding a new MCP does not require recompiling `mcpctl`
- catalog validates package manifests
- unavailable local paths are reported clearly
- duplicate MCP identities are handled deterministically
- `setup` uses catalog metadata
- installed/global package registry remains separate from catalog

---

# MVP 20 — GitHub package source

**Depends on:** package-source abstraction and MVP 14/16.

Add GitHub as a package source.

Initial syntax:

```text
github:<owner>/<repo>
```

and:

```bash
mcpctl install github:owner/project-mcp@v0.4.3
```

## Source abstraction

Introduce a source layer:

```text
PackageSource
├── LocalPath
└── GitHub
```

Both produce:

```text
PreparedPackage
```

Everything after that reuses the existing installer.

Flow:

```text
GitHubSource
    ↓
resolve explicit tag
    ↓
download source archive
    ↓
extract to temporary directory
    ↓
validate mcpctl.toml
    ↓
PreparedPackage
    ↓
existing runtime installer
```

The Python/native runtime installer must not know that the package came from
GitHub.

## GitHub v1 scope

Support only:

```text
public repository
explicit tag
release/tag source archive
```

Do not include yet:

```text
branches
arbitrary commits
private repos
GitHub Apps
custom binary assets
```

## Version validation

The package manifest version and requested tag/version must agree.

Example:

```text
requested tag: v0.4.3
manifest version: 0.4.3
```

Mismatch must fail.

## Acceptance criteria

- public tagged GitHub MCP installs successfully
- archive is extracted safely
- temporary data is cleaned up
- manifest/tag version mismatch fails
- local and GitHub packages share the same downstream installer
- source metadata is recorded
- runtime isolation remains unchanged

---

# MVP 21 — GitHub release/version resolver

**Depends on:** MVP 20.

Allow project dependencies such as:

```toml
[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "github:yourname/project-mcp"
```

`mcpctl` resolves available releases/tags and selects the newest compatible
stable release.

Example:

```text
constraint:
^0.4

available:
0.4.1
0.4.2
0.4.3
0.5.0

resolved:
0.4.3
```

Write the exact result to `.mcpctl.lock`.

## Rules

- use semantic versioning
- ignore prereleases by default
- do not silently fall back to a branch
- source tag/version and package manifest version must agree
- exact versions remain supported

Possible later opt-in:

```bash
--pre
```

## Acceptance criteria

- exact version resolves
- semver range resolves
- newest compatible stable release is selected
- incompatible releases are ignored
- prereleases are ignored by default
- lock records exact GitHub source/tag/version

---

# MVP 22 — Project-aware `update`

**Depends on:** MVP 21.

Support:

```bash
mcpctl update project-mcp
```

inside a project.

Behavior:

```text
read .mcpctl.toml constraint
        ↓
query configured source
        ↓
resolve newest compatible version
        ↓
install if missing
        ↓
atomically update .mcpctl.lock
```

Also support:

```bash
mcpctl update
```

to update all project MCPs within declared constraints.

Do not automatically widen/change constraints in `.mcpctl.toml`.

Example:

```text
manifest:
^0.4

current lock:
0.4.2

available:
0.4.5
0.5.0

mcpctl update project-mcp

new lock:
0.4.5
```

## Acceptance criteria

- updates respect manifest constraints
- old runtime remains available for rollback/caching
- failed update leaves old lock unchanged
- lock update is atomic
- updating one MCP does not modify unrelated entries
- no automatic constraint widening occurs

---

# MVP 23 — GitHub release assets for native MCPs

**Depends on:** MVP 20–21.

Rust/native MCPs should not require compilation on target machines.

Example release assets:

```text
design-advisor-mcp-v0.3.0-aarch64-apple-darwin.tar.gz
design-advisor-mcp-v0.3.0-x86_64-unknown-linux-gnu.tar.gz
design-advisor-mcp-v0.3.0-aarch64-unknown-linux-gnu.tar.gz
SHA256SUMS
```

`mcpctl` detects:

```text
OS
architecture
```

and selects the matching artifact.

Possible package manifest extension:

```toml
[runtime]
type = "binary"

[distribution]
strategy = "github-release-assets"
```

## Installation flow

```text
resolve release
     ↓
select platform asset
     ↓
download
     ↓
verify checksum
     ↓
extract
     ↓
validate entrypoint
     ↓
install in shared package store
```

## Acceptance criteria

- macOS ARM64 chooses correct artifact
- Linux x86_64 chooses correct artifact
- Linux ARM64 chooses correct artifact
- unsupported platform fails clearly
- checksum is verified before activation
- target system does not need Rust toolchain

---

# MVP 24 — Private GitHub repository support

**Depends on:** MVP 20–23.

Support authentication through:

```text
GITHUB_TOKEN
```

Never persist credentials in:

```text
.mcpctl.toml
.mcpctl.lock
registry.json
logs
diagnostic output
```

Project configuration remains:

```toml
source = "github:my-org/project-mcp"
```

Authentication is supplied separately by the environment or future credential
provider.

## Acceptance criteria

- public repos still work without token
- private repo works with valid token
- missing/insufficient permissions produce useful errors
- token is never printed
- token is never written to disk by `mcpctl`
- lock remains portable and secret-free

---

# MVP 25 — Setup profiles

**Depends on:** MVP 18–19.

Profiles provide curated default selections.

Example:

```toml
[profiles.developer]
mcps = [
  "project-mcp",
  "outside-in-tdd-mcp",
  "design-advisor-mcp",
]

[profiles.docker]
mcps = [
  "project-mcp",
  "outside-in-tdd-mcp",
  "design-advisor-mcp",
  "architecture-advisor-mcp",
]
```

Usage:

```bash
mcpctl setup --profile developer
mcpctl setup --profile docker
```

Interactive mode should pre-select profile packages but still allow changes.

Non-interactive environments may use profile defaults directly.

## Important rule

Do not hardcode:

```text
docker = every MCP that exists
```

Profiles are explicit maintained sets.

## Acceptance criteria

- profile controls setup defaults
- interactive users can toggle profile selections
- non-interactive profile setup works
- profile definitions remain data-driven
- adding an unrelated MCP does not automatically change existing profiles

---

# MVP 26 — Project `add` / `remove`

**Depends on:** setup/catalog/resolution.

## `add`

Support:

```bash
mcpctl add design-advisor-mcp
```

Behavior:

```text
resolve catalog/source
       ↓
add dependency to .mcpctl.toml
       ↓
resolve version
       ↓
update .mcpctl.lock
       ↓
install runtime if missing
       ↓
run that MCP scaffold
```

This is effectively `setup` for one MCP.

## `remove`

Support:

```bash
mcpctl remove design-advisor-mcp
```

Initial behavior:

```text
remove dependency from .mcpctl.toml
remove project lock entry
leave shared runtime cached
leave contributed scaffold files unchanged
```

Do not attempt automatic reverse-merging yet.

### Rabbit-hole deferred

Automatic scaffold uninstall/provenance tracking is deliberately deferred
because shared JSON/TOML/hook merges are difficult to reverse safely.

## Acceptance criteria

- add installs and initializes one MCP
- remove updates project dependency state
- remove does not delete runtime used by another project
- remove does not destructively rewrite shared project configuration
- shared cached runtime may remain installed

---

# MVP 27 — `bootstrap`

**Depends on:** MVP 13–26 as relevant.

Add:

```bash
mcpctl bootstrap
```

This is the fresh-clone / fresh-environment command.

Behavior:

```text
read .mcpctl.toml
       ↓
validate/read .mcpctl.lock
       ↓
sync required runtimes
       ↓
apply MCP scaffolds
       ↓
check declared external requirements
       ↓
run project-level MCP health checks
       ↓
print final summary
```

Typical workflow:

```bash
git clone my-project
cd my-project

mcpctl bootstrap
```

Docker:

```dockerfile
COPY .mcpctl.toml .mcpctl.lock ./
RUN mcpctl sync --locked

COPY . .
RUN mcpctl init
```

or, when appropriate:

```dockerfile
RUN mcpctl bootstrap --locked
```

## Acceptance criteria

A fresh:

```text
macOS ARM64
Linux x86_64
Linux ARM64
Docker environment
```

can reproduce the project's MCP environment from committed project metadata
without manually installing MCPs one-by-one.

---

# Suggested implementation order

Do not jump directly into GitHub distribution.

First complete project ownership:

```text
MVP 13  Project manifest
MVP 14  Project lock
MVP 15  Project-aware run
MVP 16  Sync
```

Then complete project onboarding:

```text
MVP 17  Multi-MCP init
MVP 18  Interactive setup
MVP 19  Local catalog
```

At this point the core UX should already work:

```text
mcpctl setup

[x] Project MCP
[x] Outside-in TDD MCP
[x] Design Advisor MCP

→ write project manifest
→ pin versions
→ install
→ initialize config
```

Then add remote distribution:

```text
MVP 20  GitHub package source
MVP 21  GitHub semver resolution
MVP 22  Project-aware update
MVP 23  Native release assets
MVP 24  Private GitHub support
```

Then polish developer UX:

```text
MVP 25  Profiles
MVP 26  Add/remove
MVP 27  Bootstrap
```

---

# Target architecture

```text
                         PROJECT

                    .mcpctl.toml
                 desired MCP versions
                          |
                          v
                     mcpctl setup
                          |
             +------------+------------+
             |            |            |
             v            v            v
          catalog       resolver      init
             |            |            |
             |            v            |
             |       .mcpctl.lock      |
             |            |            |
             +------------+------------+
                          |
                          v
                  PACKAGE SOURCES
                  /             \
                 v               v
             local path        GitHub
                 \               /
                  \             /
                   v           v
                   PreparedPackage
                          |
                          v
                  existing installer
                          |
                          v
                  shared MCP store

            $MCPCTL_HOME/packages/
               project-mcp/
                  0.4.3/
                  0.5.0/

               outside-in-tdd-mcp/
                  1.3.2/

               design-advisor-mcp/
                  0.2.1/
```

---

# Final responsibility split

```text
PROJECT
────────────────────────────────
owns:
  selected MCPs
  allowed version constraints
  exact locked versions


MCPCTL
────────────────────────────────
owns:
  discovery
  resolution
  download
  installation
  isolation
  caching
  scaffold execution
  execution


PACKAGE SOURCE
────────────────────────────────
owns:
  making package versions available

examples:
  local path
  GitHub
  future GitLab


INDIVIDUAL MCP PACKAGE
────────────────────────────────
owns:
  mcpctl.toml
  runtime requirements
  entrypoint
  scaffold templates
  its own behavior/domain logic
```

The intended end-user experience is:

```text
new project:
  mcpctl setup

existing project / fresh clone:
  mcpctl bootstrap

Docker / CI:
  mcpctl sync --locked

add one MCP:
  mcpctl add <name>

update:
  mcpctl update [name]

runtime:
  mcpctl run <name>
```
