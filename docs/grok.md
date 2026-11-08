# Grok Build

Craft Anything can use the installed Grok Build CLI for a deep build through the
Agent Client Protocol (ACP). This is separate from the xAI API. When `XAI_API_KEY` is set,
Settings → AI also offers **xAI (Grok)** as an OpenAI-compatible server for questions.

## Setup

1. Install Grok Build. Its program is `grok`, usually in `~/.grok/bin` (or `%USERPROFILE%\.grok\bin`).
   Craft Anything looks there even when a desktop launch left that folder off `PATH`.
2. Sign in with `grok login`. Grok keeps that login in `auth.json`. Craft Anything checks
   whether the file exists and does not read it or sign in for you. When `auth.json` exists under
   `GROK_HOME`, or under `~/.grok` when that variable is unset, the app treats Grok as signed
   in. A missing file is unknown, so a build still starts and Grok reports an authentication
   problem itself. `XAI_API_KEY` is separate. Grok can use it when there is no `auth.json`.
   Settings → AI detection reads that variable, and the xAI API backend can use it. The Grok
   Build adapter does not read the key.
3. In Settings → AI, detect and add **Grok Build**, then choose it for **Build with this**.
   A chat and a question are refused. If `grok` is installed somewhere else, use **Choose...**
   and pick the program.

The equivalent backend entry in `config.toml` is:

```toml
[[ai.backends]]
name = "Grok Build"
kind = "agent_cli"
model = "grok"
enabled = true
```

`model` names the agent. ACP is on for this agent. Do not set `acp = false` or an SSH host:
those are refused rather than opening the interactive TUI, which has no way to ask Craft
Anything for approval. Extra `args` are refused too. They would be placed before `agent` and
could request blanket approval. There is no folder override: the run uses the sign-in Grok
already has on this computer.

## What runs

A deep build starts:

```sh
grok agent --model <cli_model> --no-leader --reasoning-effort <level> stdio
```

`--model` and `--reasoning-effort` are omitted when you have not chosen them. `--no-leader` is
always passed, so the run does not attach to a Grok session you already have open. `stdio`
comes last. Craft Anything does not pass `--always-approve`.

The CLI `--model` flag is visible while Grok starts, then `session/new` switches back to the
model saved in Grok. Craft Anything sets the chosen model again with `session/set_config_option`
before the session is ready, and sets the reasoning effort after that, because a model change
can reset it. The value on the wire is a string (`"grok-4.6"`, `"low"`), not a nested object.
If the session already has that value, the call is skipped.

## Model and thinking

Use the shared model and **Thinking depth** picker (Settings → AI → **Change...**), or set
`cli_model` and `cli_thinking` on the same backend entry. The list comes from the session's
advertised models. There is no `auto` entry and no context-window control: Grok does not
advertise one, and inventing Standard and Extended tiers would be wrong. Reasoning levels are
whatever that model lists (`low`, `medium`, `high`, `xhigh`, and so on). A level the model
does not list is refused. An older `cli_reasoning_effort` or `effort` value is still used when
`cli_thinking` is unset.

```toml
cli_model = "grok-4.6"
cli_thinking = "low"
```

Discovery sends no prompt. It does open a session long enough to read the model list and apply
`cli_model`, so the thinking choices match that model. That read is not a question run, so the
refusal below does not apply to it.

## Tools, MCP and Clean room

Builds attach Craft Anything's MCP server on `session/new` as a stdio server named
`craft-anything`, with `"type": "stdio"` because Grok's server list rejects an object that
has no type. That field is sent only for Grok. Grok also keeps the MCP servers from your own
Grok config. Craft Anything does not edit that config and cannot turn those servers off for
one run.

Clean room, deep chat, and questions are refused before Grok starts. `grok agent` does not
accept `--permission-mode`, `--deny`, or `--disallowed-tools`, and those flags do not
constrain the agent when they are placed before `agent`. A native read does not ask
permission, so Clean room cannot keep the original out of reach. A question must not change
files. Deep chat is only analysis, and a saved permission or an always-approve setting can
edit without a request, so that limit cannot be held either. A deep build is allowed: it may
read, and an edit is gated only when Grok sends `session/request_permission`. A Grok config
that approves every tool will still edit without asking. Craft Anything does not pass
`--always-approve`, and it does not replace the person's Grok config to force a prompt.

## What was checked

Against Grok Build 1.0.77: `initialize`, `session/new`, and `session/set_config_option` (a
string model id sticks; the CLI `--model` flag does not survive `session/new`;
`--reasoning-effort` after `agent` does stick and is sent again after a model change). The
argument order, the `type: stdio` server payload, and the config queue are covered by unit
tests with a scripted agent.

A live deep build was run on 2026-11-06 with Grok Build 1.0.77. The turn finished in about
30 seconds. A tool call named the `craft-anything` server or `core_session_show`, and one
permission request was answered with allow. That run used a temporary `GROK_HOME` whose
`ui.permission_mode` was `ask`, and a symlink to the existing auth file. The file was not
read, and the person's own Grok config was not changed. A config that approves every tool
does not send that request. Copilot, Gemini, and Maki were not checked live with
`"type": "stdio"`; the field is still sent only for Grok. Clean room and deep chat were
not run live, because those runs are refused.
