# OpenCode as an AI backend

Craft Anything can run OpenCode V2 locally or on an SSH host. OpenCode owns its provider
connections and sign-in. Set these up in OpenCode on the computer where it will run.

Model and thinking choices can be discovered, validated and saved. Running with an explicit
selection is currently refused: our ACP transport cannot select it, and our native adapter
does not yet supply the run's Craft Anything MCP server. For local runs, clear both choices
and enable ACP, or use another AI. Native MCP integration remains pending in roadmap R2.2.

## Desktop

1. Open **Settings → AI → Detect**, then add **OpenCode**, or **OpenCode on `<host>`**.
   Remote suggestions come from SSH aliases; they do not confirm that OpenCode is installed.
2. If needed, choose the local program, or enter its full path in **Remote program**.
3. Under the backend, click **Change…** to load the models from that OpenCode installation.
4. Choose a model and, if available, a **Thinking depth**, then **Apply**.

The model picker lists only enabled chat models returned by OpenCode. Provider/model ids
distinguish similarly named models and preserve custom aliases. The five thinking depths are
**Low**, **Medium**, **High**, **Xhigh** and **Max**; only those reported as variants of the
selected model are offered. A model can offer fewer depths, or none.

**OpenCode default** delegates model choice to OpenCode. **Model default** leaves thinking to
the selected model. Changing the draft model resets its draft thinking depth. **Reload models**
refreshes both lists after provider or configuration changes. A missing saved model or variant
is marked unavailable; there is no fallback catalogue or silent downgrade.

Discovery runs in the background with deadlines and output limits. Listing models launches
the selected OpenCode process, may start its background service, and may update its cache;
discovery is not read-only. Local queries explicitly use the current project directory;
SSH queries use the remote shell's working directory, as the existing remote runner does.
Local model lists are never substituted for a remote one.
Changing the program, backend settings or project invalidates an open picker. Saving an explicit
choice checks it against the selected installation's live catalogue.
If another task holds the session, opening or reloading the picker asks you to retry rather
than querying without the project directory. An already open picker keeps its draft and any
matching background result until the session can be checked again.
If validation fails or is cancelled, the conversation emits its error or stopped status followed
by a terminal turn event, so headless chat finishes without waiting for an agent that never started.

## CLI

```sh
craft-anything-cli ai add OpenCode --kind agent_cli --model opencode
craft-anything-cli ai model OpenCode
craft-anything-cli ai model OpenCode my-provider/my-model --thinking high
craft-anything-cli ai model OpenCode --thinking max
craft-anything-cli ai model OpenCode --default

craft-anything-cli ai add "OpenCode on studio" --kind agent_cli --model opencode --ssh-host studio
craft-anything-cli ai program "OpenCode on studio" /opt/opencode/bin/opencode
craft-anything-cli ai model "OpenCode on studio"
```

Replace `my-provider/my-model` with an id from the listing. Listing an OpenCode backend also
returns each model's supported `thinking` values. Naming a new model without `--thinking`
uses that model's default depth. `--thinking` alone applies to the configured explicit model.

The `ai_models_list` and `ai_model_set` tools expose the same discovery and validation to MCP
and other headless clients. For example, `ai_model_set` accepts
`{"name":"OpenCode","model":"my-provider/my-model","thinking":"high"}`. Omit both
`model` and `thinking` to restore the OpenCode default.
`ai_models_list` returns a total and a page controlled by `offset` and `limit` (200 by default,
at most 1000); the desktop picker and `ai model` command load the whole bounded catalogue.

## Configuration and execution

The agent identity stays `model = "opencode"`. Its optional choices are saved separately:

```toml
[[ai.backends]]
name = "OpenCode"
kind = "agent_cli"
model = "opencode"
cli_model = "my-provider/my-model"
cli_thinking = "high"
```

These fields extend the existing versioned settings file. Old settings without them retain
OpenCode's defaults; a known variant embedded in an existing `cli_model` reference is read into
`cli_thinking`. A model-only command uses `opencode run --model provider/model`;
adding a thinking depth uses `opencode run --model provider/model#variant`.
The shared `Effort` type represents all five levels. OpenCode's `cli_thinking` remains a separate
setting from API/Claude `effort`: an unavailable OpenCode variant is refused, never stepped down.
Hub and tool selections use the same save path: reread the settings file strictly, preserve unrelated
disk changes, and reject a changed backend or project. Malformed settings are left untouched, and
the session is updated only after a successful save.
Builder runs explicitly use OpenCode's `build` agent so a saved `plan` default cannot stop before
writing files. Extra arguments can still select another agent explicitly.
Closing the app also preserves the latest AI selection. If a running tool holds the session,
pending non-AI app settings are kept in `config.pending.toml` instead of waiting for the tool or
overwriting its settings. At the next startup they are merged with the latest saved AI table;
the sidecar is removed after recovery or a successful full exit save that supersedes it.
It uses the existing settings schema and migrations; unreadable or newer files are not overwritten
by recovery.
The current ACP transport cannot select models or variants. An explicit choice bypasses ACP
even if `acp = true` was previously set, but native preflight and execution reject it until
the run's MCP server can be supplied. The refusal completes the turn without starting OpenCode.

Model discovery uses `opencode api model.list --param location[directory]=...` (the V2
`GET /api/model` operation). This keeps credentials and configuration resolution inside OpenCode.
Global `--server` and `--standalone` arguments are shared with discovery. Extra model arguments
remain available when no model or thinking selection is saved. With an explicit selection,
conflicting extra model arguments are refused; choose the model through its settings instead.
