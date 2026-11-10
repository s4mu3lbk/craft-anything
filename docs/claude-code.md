# Using Claude Code as the builder

Craft Anything can build with the person's own local Claude Code. This page records what the
integration does, why it is shaped this way, and what remains to confirm.

## How it works

- Craft Anything finds the `claude` program on the computer (PATH or `~/.local/bin`) and runs it
  as a child process in headless mode, as documented by Anthropic: `claude -p <task>
  --output-format stream-json --verbose`, with `--resume <session>` to continue a project.
- The working folder is the project folder. Craft Anything's own tools are offered through
  `--mcp-config` (our MCP server, `craft-anything-cli mcp --project … --inbox …`) with
  `--strict-mcp-config`. A build checks that `craft-anything-cli` starts before doing anything
  else, and says so plainly if not. Without the server Claude Code stops at its first approval
  with `MCP tool mcp__craft-anything__core_action_approve … not found. Available MCP tools: none`,
  and its MCP log blames an executable called `stdio`: that's Claude Code's wording for a command
  that wouldn't start, whatever the command was, not something in our config.
- Our MCP server runs in the build's mode (`--deep` or `--clean-room`); in Deep mode it lists
  every tool (`--all-tools`).
- Permissions: read-only tools (`Read`, `Glob`, `Grep`) and our MCP tools are pre-allowed in Deep
  mode. In Clean room only `TodoWrite` is: reads go through the approval tool too, which refuses
  anything in the original's folder or on its site (the project folder excepted).
  A Deep builder may start subagents (`Agent`): they get only the builder's tools and the
  session's permissions, and starting one is an "other tools" action for the approval tool.
  Clean room leaves `Agent` out (#1105).
  Every other action (writing files, running commands, fetching the web) goes through
  `--permission-prompt-tool mcp__craft-anything__core_action_approve`. That tool allows edits
  inside the project's `app/` folder and a fixed list of build commands (`npm install`,
  `npm run build`, `cargo build`…), refuses dev servers (they never exit), and decides the rest
  by its kind (the web, commands, files outside the project, pushing and publishing, deletes
  outside the project, system changes, other tools): each project can choose allow, ask or
  refuse per kind (`project_approvals_set`), and an unchosen kind follows Auto mode. Asking shows
  a card in the app ("Allow once / Always for this project / Don't allow"); "Always" on a push,
  delete or system change allows that whole kind for the project. A builder's own call to
  `project_approvals_set` can only make a kind stricter, also through `craft-anything-cli` run
  from its shell (agents we start have `CRAFT_ANYTHING_AGENT` set), and a command that names
  `craft.json`, the inbox, the approvals tool or that variable counts as a system change. These
  raise the bar; they aren't a wall: a builder allowed to run any command runs with the person's
  own permissions and could change those files another way, so "commands: allow" (Auto mode's
  default) trusts the builder.
- Questions: Claude Code's own question tool is disabled for these runs (`--disallowedTools
  AskUserQuestion`); the agent uses our `core_person_ask` tool, which shows a card with buttons.
- Before a build starts, `claude auth status` is checked. If Claude Code isn't signed in, the run
  stops with: "Sign in to Claude Code: open a terminal and run `claude auth login`".

## Chats and questions in Clean room

Outside a build there is no approval tool, so Clean room's rule that nothing inside the original is
read can't rest on it. It isn't left to Claude Code's defaults either (they let it read in its own
folder, and a release or an allow rule added later could change them) (#564):

- A **chat** in Clean room starts Claude Code with `--disallowedTools` naming its own file tools
  (`Read`, `Glob`, `Grep`, `LS`, `NotebookRead`) and its changing and web tools. A deny rule beats
  any allow, so it works only through our tool server, which keeps the original out of reach
  (`clean_room.rs`). In Deep mode a chat is unchanged.
- A **question** about a build in Clean room refuses the same file tools (it already offered none).
- A chat over **ACP** in Clean room refuses the agent's own read, write, command and fetch
  requests instead of showing them as a card the person could allow with one click; its other
  requests (planning, our tools) are allowed.
- Other agents (Codex, Gemini CLI, OpenCode, Antigravity, Hermes, Maki) are not held to this in a
  chat: they keep whatever sandbox they have, if any. See the roadmap's R2.2.

Not yet checked live against Claude Code or an ACP agent (the tests cover the flags and the
approver).

## Under Flatpak

The Flatpak build can read the home folder but not write to it, so Claude Code can't save its
conversations in `~/.claude`: the first turn ran, and `--resume` then said "No conversation found
with session ID" (#506). When the app runs in a Flatpak and `CLAUDE_CONFIG_DIR` isn't set, it starts
Claude Code with `CLAUDE_CONFIG_DIR` pointing at `craft-anything/claude` in the app's own data
folder (`~/.var/app/ai.storyteller.craft-anything/data/`), which is writable. Two consequences:

- Claude Code there is signed in separately from the one on the host. The sign-in message gives the
  command that does it through the sandbox:
  `flatpak run --command=sh ai.storyteller.craft-anything -c 'CLAUDE_CONFIG_DIR="$XDG_DATA_HOME/craft-anything/claude" "$HOME/.local/bin/claude" auth login'`
  (Craft Anything never reads Claude Code's credentials, so they aren't copied over.)
- Setting `CLAUDE_CONFIG_DIR` yourself, for example with `flatpak override --user --env=…`, wins.
  Builds on another computer over SSH are not changed.

Not yet checked in a real Flatpak (the tests cover the choice of folder and the message).

## Why this is within Anthropic's terms (as published, October 2026)

From Claude Code's *Legal and compliance* page:

- Third-party developers may not offer Claude.ai login in their own apps, route requests through
  Free/Pro/Max credentials on users' behalf, or collect, store or intermediate Claude.ai
  credentials or session tokens. Sign-in must complete through Anthropic's own flow.
- This "does not … prevent an end user from signing in to the unmodified Claude Code binary with
  their own Claude subscription".
- Products that run Claude Code must not modify the binary or disable its authentication
  methods, and may not pay for, resell or intermediate Claude usage.

Craft Anything:

- runs the unmodified `claude` binary that the person installed;
- never reads, stores or forwards credentials or tokens, and offers no Claude login of its own
  (sign-in happens in Claude Code: `claude auth login`);
- doesn't pay for, resell or meter Claude usage — usage is billed to the person under their own
  agreement;
- uses only documented flags (headless mode, MCP config, permission-prompt tool);
- describes it in plain text ("Builds with Claude Code") and doesn't use Anthropic's logos or
  the Claude Code name as a product or feature name.

## Whose account and terms

Craft Anything is open source and runs on the person's own computer. Whoever runs it runs their
own Claude Code, signed in to their own account, so their use falls under their agreement with
Anthropic, as with any other script that calls `claude -p`. The free release ships on that basis.
The questions below matter for a paid offering, where we would be the party.

## Open questions (need confirmation before a paid offering)

1. The Agent SDK overview says third-party developers may not "offer claude.ai login or rate
   limits for their products, including agents built on the Claude Agent SDK" without approval.
   Craft Anything doesn't offer a login, but it does let a person's subscription do work that
   our app orchestrates. The legal page explicitly allows end users to use their own
   subscription with the unmodified binary, which is what we do. Still, **confirm with Anthropic
   (sales/partnerships) that orchestrating the user's own CLI this way is acceptable at scale**,
   especially for the planned managed-subscription tier.
2. "Advertised usage limits for Pro and Max plans assume ordinary, individual usage." Rebuild
   rounds are bounded (default: first build + up to 6 refinements in the app, 2 in the CLI; `--rounds` changes it) to keep
   usage ordinary.
3. The managed tier (we pay for model access) must use the Claude API with our own key under
   the Commercial Terms, never users' subscriptions.

## The compliant alternative

If the subscription route is ever unsuitable, the same workflow runs with API-key backends:
Settings → AI → Anthropic (Claude API) or OpenRouter. Today those backends answer questions and
analyse targets; building with them needs a file-writing tool loop in our own agent (see
docs/STATUS.md, "Next").
