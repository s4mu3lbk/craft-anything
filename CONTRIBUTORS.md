# Contributors

Craft Anything is built by the people below and everyone in the repository's history. Anyone
whose pull request is merged is a contributor.

## Before the public history (2026-10-12 to 2026-10-30)

Craft Anything was first developed in a private repository. The public history starts with a
single commit, "Craft Anything" (2026-10-31), that brought in that work, so it carries one
author. This is who did that work: 261 commits on the private `main`.

**echelon** (4 commits) started the project: the first engine, 184 tools, the egui app, the CLI and the MCP
server, then SQLite, Java and Solana sBPF analysis, readable reports and provenance.

**bflatastic** (137 commits) built most of what people see and what a rebuild does:
- Simple mode, the rebuild workflow and the Claude Code builder; the ArtCraft look, the title
  bar, typefaces, the raven icon and the New project front door.
- Games: engine detection and builds with the original's engine, guided installs, Play, the
  game files screen, Unity and Unreal archive and texture readers, and generated game assets.
- Deep mode with Ghidra, Auto mode, and Clean room as a switch of its own.
- Website, app and console checks: pages compared at every width, menus and pop-ups, the claims
  a build must meet, program runs compared, Windows UI Automation for studying and checking apps.
- Scripted web sessions, the build's time and cost, pausing and continuing builds, and the plain
  tool ids, error codes and output names the project uses now.

**tanish2k09** (120 commits) set up how the project is worked on and moved it to the
architecture it has now:
- The roadmap, AGENTS.md and its working rules, docs/pitfalls.md, `cargo xtask pending`, the
  contributing and security guides, pull request and issue templates, and CI on every pull
  request.
- Safety checks: licence checks (`deny.toml`), every external program through the platform
  runner (checked by `cargo xtask layers`), tests that never touch the real machine, and
  `cargo xtask secrets` for keys and local paths in commits.
- Extension points: target kinds and checks as registered traits, stacks as data
  (`stacks.toml`), coding agents as adapters, asset providers with a priced model table, one
  engine table with hash-checked install pins, versioned files with migrations and published
  schemas, and the tool id scheme with aliases for old ids.
- Headless first: projects, engines, settings, keys, budgets, setup, Play and preview as tools
  that the UI, CLI, MCP and agents all call; higher layers registering their own tools.
- Plugins: MCP servers (local and over HTTP) and sandboxed WASM plugins for tools and checks,
  with example plugins.
- Building: the builder's file and command tools under one approval policy, builds on API-key
  backends, ACP agents, several builds at once with a queue, the website likeness score, checks
  by what an app or game draws, and the mechanics sheet for games.
- The release pipeline, packaging and the browser viewer.
- The desktop app: projects in a folder you pick, choosing which AI builds, renaming,
  duplicating and deleting projects, drafts that survive a restart, pinned builds, questions about
  a build answered without starting one, prompt history and slash commands, the desktop log, a
  sidebar that folds in narrow windows, and drawing that never reads files.

## Since 2026-10-31

Everything since is in the public history, under each commit's author.
