# Roadmap

The shared plan for reaching the product described in the README, for people and agents alike.
`docs/STATUS.md` says what works today; this file says what comes next, who owns it and what an
agent may pick up; [roadmap-history.md](roadmap-history.md) keeps what's finished. Rebuilt on
2026-11-02 from `main` at `2e4afc5`, with v0.3.0 the latest release.

## How to use this file

- Each item has an **id** (`R<milestone>.<n>`), an **owner**, a **status** and a list of
  **pending changes**. Pending changes are small, independently mergeable units. Ids are never
  reused or renumbered: commits, code comments and PRs quote them.
- **Claiming:** before you start, put `— claimed: <name>, branch <branch>` on the pending change
  and push your branch early (or open a draft PR) so others see it. Don't take a change someone
  else has claimed. `— in PR #n` marks one a PR already covers: review that PR rather than starting
  again. The full working rules are in `AGENTS.md` ("Working rules", mandatory). Items owned by
  **maintainers** are steered by the project maintainers: agents may take their *unclaimed pending
  changes*, but design decisions marked **decision** wait for a maintainer.
- **Finishing:** tick the change (`[x]`) where it is, add the commit or PR, update `docs/STATUS.md`
  if what works changed, and add a line to `log/devlog.md`. Maintainers move ticked lines to
  `roadmap-history.md` when a release ships.
- **New work:** add a `- [ ]` line under the item it belongs to; a new item takes the next free id.
- **Status:** `todo` · `in progress` · `blocked (<why>)` · `done`.
- Follow `AGENTS.md` (layering, never crash, everything is a tool, `platform::process` for every
  external program) and the architecture rules below.

## Architecture rules for new work

The project should be easy to fork and bend. New features land behind a seam, not inside a
`match`:

1. **Traits plus a registry, not enums.** Target kinds, checks, builders, asset providers and
   external engines are traits with a registry that holds built-in implementations. Adding one
   means adding a file and one registration line (as tools already work).
2. **Data over code for catalogues.** Stacks, game-engine build recipes, model ids and prices,
   and engine install pins live in typed tables (or TOML files embedded with `include_str!`), not
   in string matches or prompt text.
3. **Headless first; flows are tools.** Everything works without the GUI. Anything a person
   can start from the UI (rebuild, follow-ups, answering questions, engine installs, launching a
   result) is reachable through `Engine::call` and the CLI, so MCP, agents, scripts and other
   frontends get it for free. Orchestration, persistence and process control never live in
   `ui-egui` or `apps/*`.
4. **Versioned files.** Every file we write and read back (`craft.json`, `config.toml`,
   `project.toml`) carries a version, has a JSON schema, and has a migration path.
5. **Structured results.** Decisions such as "did the build pass" read typed fields, never
   English text prefixes.

## Milestones at a glance

| Milestone | Goal | Owner | Where it stands (2026-11-02) |
|---|---|---|---|
| M0 Foundations | Docs match the code, licences checked, one live end-to-end run | maintainers | Done |
| M1 Extensibility | Seams for target kinds, checks, builders, asset providers, engines, plugins | maintainers | Done, except engine pins (R1.5) |
| M2 Product completeness | Every kind of target gets study → build → check → refine with a real check | maintainers | Websites complete; apps and games build and run; API-key builds have never run live (R5.5); agents other than Claude Code are only partly wired (R2.2) |
| M3 Engine depth | More tools, and the known partial ones | open | 314 tools; the open lane |
| M4 Release | Packaging, signing, a managed tier | maintainers | v0.3.0 released, signed, on every platform; a macOS CLI question and the managed tier remain |
| M5 Works for real people | What people hit now that the project is public | maintainers | Comes first |

## Open work

Every unticked `- [ ]` line in this file is open work, under the item it belongs to.
`cargo xtask pending` lists them all, grouped by item (`--count` for the totals). When you find
work that's missing, add it as a `- [ ]` line under the right item in the same PR, rather than
leaving it in a PR description or a chat.

## Order and dependencies

Work in phases. Items in the same phase can run in parallel. A phase says what comes first; what
stops an item from starting is its **Depends on** column, which must be done first. A *soft*
dependency should come first but doesn't block.
Issues are ranked by people affected and how badly, after leaving out what's already fixed.

| Phase | Item | Depends on | Why here |
|---|---|---|---|
| 0 Ship | R5.1 The next release | — | The Windows start-up crash and a build-breaking resume bug are fixed on `main` but not released |
| 0 | R5.2 Keeping contributions moving | — | `main` isn't protected, first-time contributors' CI is held, and a waiting PR fixes a safety bug |
| 1 Builds that work | R5.3 A builder that can't make progress stops | — | Costs people money or quota on every backend; small |
| 1 | R5.4 Local models can build | R5.3 *(soft)* | Largest open gap, with no fix anywhere yet |
| 1 | R2.2 Every builder gets the same tools (that group), and Approvals | **decision** for Approvals | Agents other than Claude Code start but can't do the job, and Clean room's rule doesn't reach them |
| 1 | R5.5 API-key builds fail in words people can use | — | This path has never been run live, and its errors say nothing |
| 1 | R5.6 Tools report only what they saw | — | Confirmed false successes on macOS; golden rule 3 |
| 2 Proof | R5.7 Live checks owed | — | Built but never run for real; each can reopen work above. Lines run when the machine or key they need is at hand, Clean room and the quick start first |
| 3 Depth | The rest of R2.2 | — | Detection waits for reports from 0.3.0 or later; ACP after the adapters work |
| 3 | R2.4 Runtime study, R2.5 Behaviour checks | scripted input (R2.4) before R2.5's sheet check | Makes "ready" mean more for apps and games |
| 3 | R2.7, R2.8 | — | Small gaps left in finished work |
| 3 | R2.9 Everyday use, R2.10 Pinned builds | — | Open to anyone at any time; most lines stand alone |
| 3 | R1.5 | — | Small leftovers |
| Any | M3: R3.3, R3.9, R3.11, R3.14 | — | Open lane |
| Later | R4.1 macOS CLI, R4.2 Managed tier | **decision** | Not before M5 |

---

## M5 Works for real people

### R5.1 The next release
Owner: maintainers · Status: todo
Two reported failures, the Windows start-up crash (#414) and a builder resuming another's
session (#471), are fixed on `main` and still broken for everyone on 0.3.0. This is the
cheapest help we can give anyone, so nothing below should hold it up for long.
- [x] #490 merged (`549ae5e`) with the CLI fix the review asked for: a string-or-object input is
      `any`, so a JSON object given to `--left` stays an object (tested both ways). What it reports
      is 0.2.0's, fixed by #93 (`f6725c2`): on Windows the
      published 0.3.0 CLI with Claude Code 2.1.179 mounted 314 of 315 tools (the 315th is the
      approval tool, which Claude Code doesn't list but calls), and a real Deep build reached Ready
      with `core_action_approve` answering five prompts. The reporter was asked for `--version`.
      macOS is still owed (R5.7)
- [ ] Cut the next release (`docs/releasing.md`). The version is 0.4.0 (#526) and `main` at
      `fb7ac86` is green, but pushing `release` is refused: its ruleset blocks every update and
      lists no one who may bypass it. A maintainer who can push it (or adds a bypass) goes next.
      0.3.0 lacks `0748ef1` (#414: Windows starts DX12 only, the crash behind "it won't open"),
      `1e8f661` (#471: a builder never continues another builder's session) and `3357daf` (#430:
      Claude Code's message on stdin)
- [ ] See that the launcher offers the new version: #74, #104 and #420 installed through it, and
      nothing in this repository says how it picks one
- [ ] After it ships, close what it fixes: #166, #346, #420, #478. #74, #104 and #222 give no
      detail, so they may be another cause: close them too, saying to file again if it still fails.
      Ask #201 whether `3861132` (#379) ended its early compression (a guess at the link)
- [ ] Close the tickets 0.3.0 already fixed, and the spam: #47, #53, #66, #95, #112, #113, #130,
      #133, #138, #152, #179, #182, #185, #202, #214, #329, #344, #377, #378, #400, #449; spam #194,
      #195. #113, #329 and #377 are 0.2.0's missing CLI, fixed by `1b54cbf`, not what #490 reports.
      Not yet: #64 (its cause is fixed, its retries aren't: R5.3), #142 (R5.3), and #462, which says
      `none`, not #490's failure, and gives no version: ask for it
- [ ] Sometimes starting the app shows nothing: after the release, see from `desktop.log` whether it
      still happens. DX12-only may have ended it; a GOG Galaxy Vulkan overlay layer was seen loading
      into the app
- [x] A program that is found but can't start (an npm script whose `node` is missing) says where it is and
      what to look at, not "isn't installed" (#152, #95); on Windows a program found is started by its
      path, not its name
- [ ] Live-check #152 on Windows 11 with OpenCode from npm, and on a machine with the agent but no
      Node.js
- [x] A start that dies before its window (a native driver crash leaves no dialog or log, #74, #104,
      #33) is remembered: `starting` beside `desktop.log` is written before the window is asked for
      and removed once it's open, and a start that finds it draws with WARP that once
      (`apps/craft-anything/src/start_guard.rs`)
- [ ] Live-check the start fallback on Windows: make a start die before its window (kill the
      process at launch), start again, and see `desktop.log` say it drew without the graphics card
      and log `drawing with` an adapter of type `Cpu` (WARP), and that the window opens
- [ ] The software retry still lets wgpu start every DX12 adapter before `pick_adapter` runs, so a
      driver that crashes while its device is created crashes the retry too. wgpu has no way to
      skip hardware adapters; needs a different route (another renderer, or a helper that starts
      the app with only WARP visible)
- [ ] Add to `docs/releasing.md`: before publishing, a Deep build through Claude Code from the
      finished package, on a Mac and on Windows. Starting the CLI from the package (#91) doesn't
      show whether Claude Code accepts its tools
- [ ] Add to `docs/releasing.md`: after a release, close every ticket marked as waiting for it, so
      they don't stay open marked `fixed`
- [x] NixOS: An app and A game open their file/folder pickers with Zenity and libdbus available;
      missing runtime dependencies made both buttons look unresponsive. Package and live picker
      checks passed
      — claimed: Kimi Code, branch bugfix/r5.1-nixos-file-dialogs

### R5.2 Keeping contributions moving
Owner: maintainers · Status: in progress
Review comes before new work while the queue is long, smallest and clean PRs first. A PR is merged
only when it's green and a maintainer has fully reviewed it, code as well as wording.
- [ ] Maintainers: protect `main` (PRs required, squash only); the only ruleset covers `release`
- [x] Auto mode's delete check is a safety bug that shows up as Mac test failures. It splits a
      command on spaces, so `rm -rf "C:/x/Game Backup/y"` from a project at `C:/x/Game` goes ahead
      unasked, and it lower-cases paths, so on a Mac or Linux a delete inside a project with
      capitals in its path is refused; `auto_mode_keeps_going_but_never_does_the_dangerous_things`
      fails on a Mac for that. `real_binary_end_to_end_if_present` fails there too, expecting
      `/bin/ls` to be Linux: both fixed (#357)
- [ ] **decision** First-time contributors' CI: GitHub holds their runs for approval, so their PRs
      show no `gate` run (#490 and #413 were among them). Approve each push, or
      change the repository's approval setting
- [ ] #193 changes the language list that #486 (merged) changed too: it needs a rebase. #468 looks
      redundant with #500: ask its author
- [ ] **decision** Wide PRs with no roadmap line: #416 (project goals, 34 files, cites an R2.11 that
      doesn't exist; that id stays free for it) and #503 (two asset providers and spending controls,
      37 files, cites the finished R1.4). Decide whether we want them before more is built on them
- [ ] **decision** Replies don't reach people who file through the ticket relay (each such ticket
      says so), yet the bot asks them for details. About half the open issues came that way. Get
      replies back to them, or stop asking
- [ ] `which::tests::usual_windows_dirs_come_after_path` (platform) fails on a Windows machine
      whose environment adds a folder (10 found, 9 expected, "nothing twice"), on `main` too; and
      `capture::console_tests::reads_what_the_program_drew_and_stops_at_the_deadline` failed once
      with the whole platform suite running and passes alone. Find what each depends on
- [ ] `a_helper_is_judged_by_whether_it_runs` (ai crate) sometimes fails when several crates'
      tests run at once and passes alone: it writes a script and runs it straight away, which a
      busy machine can refuse while another test's child still holds the file open
- [ ] `a_remote_query_expands_only_the_remote_working_directory` (ai crate) fails on a Mac: the
      temp folder is `/var/…`, a link to `/private/var/…`, and the shell's `$PWD` gives the second.
      Compare with the folder's canonical path
- [ ] Revisit the two advisory exceptions in `deny.toml`: `rsa` (no fix; we only verify with public
      keys) when rsa 0.10 lands, and unmaintained `zbus_names`/`zvariant` (Linux-only via keyring
      and accesskit) when those move on. `deny.toml`'s comments say this is tracked in R0.3: point
      them here
- [ ] Check whether the release workflows still run actions that target Node.js 20. None is named
      here any more: every job calls `storytold/craft-common-actions@main`, so a fix belongs there
- [ ] STATUS.md's "Known limits" still says the project feed, game-engine and Ghidra installs and
      launching exist only in the desktop app, which R1.6 ended: update it
- [x] A spinner on screen no longer makes the window redraw at the display's rate: egui's own asks
      for a frame at once every frame, ours at the animation's pace (`widgets::Spinner`, #579)
- [ ] Measure #579 on Windows 11 (0.3.0 portable): the app's CPU while idle, while a build runs and
      during a website capture, before and after; and why a website capture is slow and sits for a
      long time (no details were given, so nothing was changed for it)

### R5.3 A builder that can't make progress stops
Owner: open · Status: todo
Every round spends the person's money or quota. One that can't do anything shouldn't start.
- [x] Preserve the interrupted request when an agent cannot restore its session: ACP starts a new
      session, or a registered CLI adapter falls back to Plain (#552 review).
      — claimed: Afloat16, branch bugfix/r5.3-interrupted-request-recovery-20261103
- [x] A build stops and says why after two builder turns in a row that changed nothing in `app/`,
      on every backend (#44, #110, #417, #481). The count is saved in the project, so it holds
      across runs: a run ends at 7 checks (`max_rounds`) but check numbers carry on, so #44's check
      28 and #417's 35+ look like several runs (a guess: Try again is the only restart found, and
      Auto mode doesn't start runs) — implementation: `2fc2970948315b64d47359543b458509da42ab3f`
- [x] A builder whose tools didn't start isn't resumed. `turn` (`rebuild/mod.rs`) tries 3 more times
      ("Picking up where it left off", #377) because `worth_retrying` doesn't know this error. Give
      it a typed signal, as `Conversation::session_lost` does, not one more word in that function's
      list. Say what failed and stop (pitfalls: never retry what can't succeed) — commit `f80a16a66834`
- [x] `worth_retrying` decides on English words in a message (AGENTS.md: structured results). Move
      its other cases (sign-in, credit, a missing install) to the same typed signal — commit `f80a16a66834`
- [ ] Find what repeated #64's attempts and stop it: it logged 41, about 38 s apart, on 0.2.0, but
      `turn` allows 3 retries 30, 60 and 90 s apart, there too, so something else starts them
- [x] A rate limit is waited for only on agents with a session: `turn` retries only when
      `agent.session_id()` is set, which API backends and OpenCode's plain adapter never set (read
      in the code, not run). So #148 didn't reach #142, which was OpenCode. Wait and send the turn
      again for them too, then close #142 — commit `f80a16a66834`
- [x] A builder step on an API backend stops after 25 failed build commands in a row (none
      succeeding between), says so and keeps the project (`MAX_FAILED_BUILDS` in `agent.rs`, #399:
      9.4M tokens in 19 minutes of the first stage, never compiled). Try again carries on
- [ ] **decision** Whether a build gets a token or cost limit of its own, beyond rounds and that
      stop (#399): a model that changes code without ever running a build isn't caught by it
- [ ] Check live that the stop fires on a real API-backend build and the message reads well. It
      fired on a local Ollama build (the same built-in loop) after two turns that changed nothing
      in `app/` ("The build stopped after two builder turns changed no files in app/…"); an
      API-key build is still owed
- [x] #736: the build check and the stop message say when `app/` is empty, so a builder that
      claims a build can't hide that nothing was written. v0.3.0 (the reporter's release) had no
      stop after two unchanged turns (#557, merged after it); main stops, and this adds the
      empty-folder wording to the build check (it failed first, before #749's website check) and
      to the stop message — claimed: claude, branch bugfix/r5.3-empty-app-loop

- [x] A build whose checks keep finding no working build stops after 10 in a row, across "Try again"
      presses, until a new request starts the count again (`NOT_WORKING_CHECK_LIMIT`, kept in craft.json as
      `not_working_checks`). "Try again" used to start six more rounds every time with nothing counted across
      runs, so #736's build went on to check 40 and 49 times. A run at the limit stops before a builder turn.
      The unchanged-turn guard already stops a builder that writes nothing; #736's exact Windows run (gemini-cli)
      is not reproduced here. An empty `app/` is named as having no files yet (#771's wording) and the builder is asked to
      write the project and show its build's output, not to fix a build (`NO_PROJECT_PROMPT`). — claimed: fabox-os, branch bugfix/736-stop-empty-build-loop
- [ ] Live-check #736 on Windows with gemini-cli: an empty app/ and a builder that claims `npm run build`
      stops after two turns (or 10 checks) with the "no files yet" note, and Try again doesn't restart it

### R5.4 Local models can build
Owner: open · Status: todo
The built-in agent loop assumes a capable tool-calling model. Reports #24, #44, #110, #403, #417 and
#498 say otherwise (#44's backend is a guess; #116 may be the same), and no local model build has
been run live.
- [x] A live local build through Ollama, small target, to Ready or to a clear stop, and note in
      STATUS.md which models it worked with: Ollama 0.40.7 with `qwen2.5-coder:14b` on Windows
      (RTX 5090, 32k context, no prompt cut) stops clearly at "No project to build found in app/
      yet", twice. The lines below say why
- [x] A builder on a local model is offered 13 tools (about 10,000 characters of descriptions)
      instead of 33-41 (23,000-30,000): the Build tools and the Core tools the brief names, not the
      study set a model explores with instead of writing (#417, `LOCAL_BUILDER_CORE`). Measured as
      text, not on a model; Ollama's default context (4k) was too small for the old list alone
- [x] Run #417's case live (Ollama, `qwen2.5-coder:14b`, "a scientific calculator") before and
      after this: does it call `build_file_write` now? If it still doesn't, the model can't build.
      Yes, with the three fixes below: over about 50 calls it wrote `app/Cargo.toml` (eframe), fixed
      the two errors the checks reported, and check 3 said "Build succeeded". But `src/main.rs` was
      still cargo's "Hello, world!": no calculator was written, and the screenshot check rightly
      failed it at the round limit (#753)
- [x] The system prompt told every model to call `core_brief_write` and `core_plan_update` first and
      `core_handoff_write` last, tools a lean local builder isn't offered, so the model asked only
      for `core_brief_write` and did nothing else for three checks. A lean builder's prompt leaves
      the memory steps out; live, the same model then used the Build tools
- [x] A call written after a sentence ("Let's list the files.\n```json {…}```") isn't run: #554
      reads a reply that is only the JSON. Half the model's calls came this way and the turn ended
      with nothing done. A reply that ends in complete `json` fences, each a call to an offered
      tool, with only words before or between them, now runs them all (`fenced_calls`, ids
      `react_N` as other text calls); words after the last fence, other code, an unclosed fence or
      a tool that wasn't offered keep it text (#753)
- [ ] A local builder spends its rounds on the project file and never writes the app: run 4 had a
      compiling `Cargo.toml` by check 3, its last, with `main.rs` still "Hello, world!". See whether
      more rounds get there, or whether a small model needs the window and the `--screenshot`
      option written for it (a template in the Rust steps)
- [x] The model ran `cargo new app` in `app/` (commands run there), so the project landed in
      `app/app/` and every check said "No project to build found in app/ yet". The check now says
      where a cargo or npm project one folder down is, and the Rust steps say `app/Cargo.toml`,
      not a folder inside `app/` (a brief can't say `cargo init`: not every builder's commands
      start in `app/`). Live, with calls after a sentence run too, the model then made
      `app/Cargo.toml` and the build compiled (#751)
- [ ] The model reads file paths from the project folder and commands from `app/` as one base:
      it ran `cargo init app` and `cd app && cargo build` after writing `app/Cargo.toml`. Say the
      folder in `build_command_run`'s result (or its description) so the two can't be mixed up
- [ ] `usage.json` after a local build says about 4,200 input tokens and 6.5 s of work, while
      Ollama logged about six requests of 4,000-5,000 tokens each: find what it leaves out
- [x] An assistant turn with no text sends an empty string, not `content: null`, which Ollama
      refused as `invalid message content type: <nil>` (#403, #536)
- [x] Understand a tool call a model writes as JSON in its text (#417; implementation `dfe9a4db2dcb3e27edbd67311cbb4202584e7af6`). The fallback protocol reads
      only `Action:` and `Action Input:` lines, so anything else ends the turn with nothing done
- [x] Preserve `length` and `content_filter` stop reasons on native tool-call streams; a parsed
      call must not override truncation or refusal and execute tools. Pair other retained
      unexecuted calls with results before the conversation continues — PR #544
- [x] Return a rejected tool argument's JSON Pointer when known and guidance to correct the call
      (#498), keeping schema types strict
      — commit `0c3ae5cca569`
- [ ] Find how each local server is told its context size and set it, or stop with a plain message
      when the prompt won't fit. We assume 24k and send no `num_ctx`, which an OpenAI-style endpoint
      may ignore anyway; LM Studio refused a 12.6k prompt on its 8k default (#73)
- [x] A per-backend context budget in `[[ai.backends]]` (`LoopConfig::context_tokens` exists), for
      services whose real window differs from 150k (#374) — implementation: `6bd94b84828ec89b805cb8a246f557d0a778c037`
- [x] Find LM Studio on another port (#73): the port in its settings file (when it is there) and
      the ports 1235-1239 are probed like the usual servers; OpenCode's own `baseURL`s were already
      read (#83)
- [ ] Check live that LM Studio on `--port 1235` is offered, and where its settings file is on
      Windows, macOS and Linux (`LM_STUDIO_SETTINGS` is from memory of its folders, not verified)
- [ ] See why Bionic or LM Studio can't be picked (#140)
- [x] Do not select an embedding, ranking or transcription model as a chat backend when a local server has no chat models — claimed: Afloat16, branch bugfix/r5.4-nonchat-model-detection-20261103
- [x] Recognize specialised model file paths and numeric task-name versions during local chat setup
      — claimed: Afloat16, branch bugfix/r5.4-model-task-identifiers-20261103
- [x] The first run's AI step can add a model server on another computer by its address (llama.cpp,
      Ollama or LM Studio on a home server), the same form as Settings → AI
- [ ] A chat sent while a llama.cpp router is still loading the model ended after 31 s with "The
      answer stream ended early." and no next step (seen once live, 27B model); say the server may
      still be loading it and to try again in a minute

### R5.5 API-key builds fail in words people can use
Owner: open · Status: todo
- [x] `rebuild --backend <an API backend>` still adds the first agent app it finds to the person's
      settings and prints "Using Claude Code (…)": `ensure_agent` (`rebuild_cmd.rs`) runs whenever no
      agent app is set up, though this run won't use one. Run it only when the build needs one
      — claimed: AlexJoaquimPereira, branch `bugfix/r5.5-no-agent-with-api-backend`
- [ ] `--backend <a name that is turned off>` still builds with it: `builder_backend`'s named
      branch doesn't check `enabled`, while Settings treats an AI that is off as unusable
- [x] Bound accumulated data across all lines of one server-sent event, so short lines cannot
      grow a streamed API response without limit
      — claimed: Afloat16, branch bugfix/r5.5-bounded-sse-events-20261103
- [x] Show the provider's own reason: #369 bought $10 of credit and got only "OpenRouter answered
      400: Provider returned error". Find the reason in the response body and show it — PR #542
- [x] OpenRouter picks an upstream provider per request, and one without tool calling answered
      "400: Provider returned error" (#369): a request with tools now sets
      `provider.require_parameters`, and a 404 "no provider for these parameters" falls back to the
      text protocol like any server that rejects tools
- [ ] Check #369 live with Kimi K3 and a key limited to Kimi/Moonshot models (needs the person's
      OpenRouter key): that the build starts, or that the error now names the provider's reason
- [ ] A live API-key build (needs a key on the test machine)
- [x] A Google AI Studio (Gemini API) key is its own choice in Settings → AI and first run (#646):
      the `gemini` backend kind uses Gemini's OpenAI-compatible endpoint and `GEMINI_API_KEY`
- [ ] Check a Gemini build live with a real Google AI Studio key (tool calls through the
      OpenAI-compatible endpoint)
- [ ] Failure logs for the other ways an AI stops (API backends, MCP server start-up), like the
      ones agent CLIs write to `logs/` in the config folder

### R5.6 Tools report only what they saw
Owner: open · Status: todo
Golden rule 3: the result is evidence. All four tickets are confirmed in the code, with no PR yet.
Each fix needs a Mac to test on.
- [ ] macOS: a denied Accessibility permission is reported as success with an empty tree (#240)
- [ ] macOS: `runtime_ui_read` ignores `screenshot` (#248) and `accessibility` (#307): the Swift
      helper declares both and reads neither
- [ ] macOS: call observation writes a literal `\n` into its lldb callback, so the Python is
      invalid and it reports zero calls (#251)

### R5.7 Live checks owed
Owner: open · Status: todo
The code for each of these is done; what's left is to run it for real and record the result in
STATUS.md. A failure becomes a new line under its item. Grouped by what the run needs. Start with
the Clean room build and the README quick start: Clean room is the default and has no live build
yet, and the quick start is how every new person begins.

**A signed-in coding agent:**
- [ ] A live Clean room build of a website and of an app, checked end to end
- [ ] A live build in a picked folder each way (edit in place, start from), checked end to end
- [ ] Thresholds under pressure: a clone that's off by a few pixels (a page with fluid type or
      `clamp()` spacing) to see whether 4 / 2 / 8 px is too strict
- [ ] Screenshot check: in a live run the final check once got no picture from an Electron app
      within 90 s while the same check passed in the builder's turn and on a re-run; not
      reproduced. The app waited on animation frames, which a window behind others may not get;
      the Electron steps now say to capture after loading instead
- [ ] A live chat run with a big comparison result, to see Claude Code page it with
      `core_evidence_browse` (and whether 80 KB is the right size)
- [ ] Live check on Windows: rebuild a project while its frozen pin runs, and the build isn't
      blocked

**An API key or a bill:**
- [ ] Run Nano Banana Pro once with a real key (a picture, then a 3D model from it) and check the
      picture's size and the invoice ($0.15)
- [ ] fal bills pictures "by rounding up to the nearest megapixel"; estimates use the exact size.
      Confirm on a real invoice whether a 1024×1024 picture bills as 1 or 2 megapixels, then round
      in the table (needs a key)
- [ ] A live extraction run with Kimi K3.1 Pro (needs the person's OpenRouter key)

**A second computer or another system:**
- [ ] A live run of a project copied between two real computers with Claude Code (#451, #452); done
      on one Mac with a second `CLAUDE_CONFIG_DIR` standing in for the other computer (STATUS.md)
- [ ] Ghidra setup tried for real on Linux and macOS (the Java pins were checked by tests only)
- [ ] Follow the README quick start on a clean machine (Windows, and Linux or macOS)
- [ ] Build the new DMG on a Mac (release dry run) and look at the window once
- [ ] #490 on macOS: a real Deep build through Claude Code with the published 0.3.0 CLI, with
      2.1.179 and a current version. Windows passed; the Mac CLI was only checked to contain the
      fixed schema
- [ ] macOS CLI zip blocked by Gatekeeper when downloaded in a browser (#94), though Apple has its
      notarization tickets: find why the online lookup fails for people (it passes on our Mac, which
      may be exempt), on a machine that shows it

**A real program, device or file:**
- [ ] Cutter sessions: try them against a real Cutter, including whether `=h&` keeps Cutter
      responsive (only a stand-in server so far)
- [x] Vineflower: run it once on a real JAR (no Java on the machine it was written on)
      — claimed: hasnain607, branch docs/r5.7-vineflower-live-run
- [ ] Verify the 23 unverified adb tools; record results in tool-status.md
- [ ] Android locales: try `only_locales` on a real APK (only the test fixture so far)
- [ ] `app_graphs_compare` on the two real Electron releases again, to count how many of the
      162 "removed" now pair (and pair the `index-<hash>` chunks by structure if many stay)
- [ ] A real IL2CPP `global-metadata.dat` (needs a Unity build or a shipped game); synthetic so far.
      Compare a real APK's components against another tool's reading
- [ ] Stand-in backend: a browser-made HAR and a real front end talking to it (only a hand-written
      HAR so far); GraphQL and tRPC on real bundles (only hand-written minified shapes)
- [ ] Check the Kimi Code, Factory Droid and WorkBuddy registrations in the real apps

---

## M1 Extensibility

### R1.5 External engine trait in L3
Owner: open · Status: in progress
- [x] Find Chrome, Brave, Chromium and Edge installed as Flatpaks on Linux (system and per-user
      exports), and keep a Flatpak or Snap browser's private profile in the home folder its
      sandbox shares, since its /tmp is its own (#747). Unit-tested; not run on a real Flatpak
- [ ] More pins once their hashes are checked against the publisher: Chromium (no published
      checksums found yet), Rizin for Linux (the tar.xz unpacks to ~800 MB, so it needs streaming
      extraction) and macOS (a .pkg)

### R1.6 Headless first
Owner: maintainers · Status: in progress
- [x] Give Linux-specific Clang guidance in Unreal's guided installer when the bundled compiler
      is missing (the requirements summary is already platform-specific).
      — claimed: AlexJoaquimPereira, branch `bugfix/r1.6-linux-unreal-clang`
- [x] Give macOS the right C++ guidance when Xcode is missing: the status line and
      requirements summary now name Xcode, matching the guided step.
- [x] Find Visual Studio's C++ tools on Windows when Visual Studio is installed outside Program
      Files (through the Visual Studio Installer's `ProgramData` instance state or developer
      environment variables, #669, [PR #726](https://github.com/storytold/craft-anything/pull/726))
      — claimed: riyaz, branch `bugfix/r1.6-windows-vs-cpp-tools`

---

## M2 Product completeness

### R2.2 More coding agents
Owner: open · Status: in progress
- [x] A saved agent program path that no longer exists (Codex desktop moves its CLI to a new
      build folder on every update) says so and names the path, instead of "isn't installed or
      isn't on your PATH" (#721); it stays a missing program, so a builder turn isn't retried
- [x] A relative program path started in another folder is looked up where it was started
      before it's reported as missing (Unix: a script whose interpreter is missing was said to
      have no file there)
- [x] `ai test` names the program it ran and the saved one it replaced, and `doctor` says a saved
      path has no program there instead of blaming PATH (#721)
- [ ] Settings says when a saved agent program is gone and which one runs in its place; today a
      build moves to the newest build folder or the usual program without saying so, and the
      setting keeps the old path (read on a worker, not while drawing) (#721)
- [x] GitHub Copilot through its own CLI login and ACP, without credential import:
      discovery, MCP-only tool restrictions, model selection and fresh read-only questions;
      unit tests and CLI 1.0.105 with an isolated home and loopback model, all five access modes
      — claimed: qu-gg, branch `feature/r2.2-copilot-integration` (recorded during PR preparation)
- [x] Review hardening: cap target handoff at 128 while preserving the active target; propagate
      context-discovery cancellation, normalize CLI-only efforts for Claude requests, and name
      the configured agent in shared picker warnings, with focused regressions and translations
- [x] Local agent chats share a saved investigation and open-target metadata with MCP; background
      refresh makes recorded findings visible in the workbench Evidence pane. `core_finding_edit`
      corrects a finding with checked evidence and an optional revision guard, preserving confidence
      when omitted. Builder refresh failures warn instead of ending the turn; shared ACP errors
      use the current agent and its sign-in guidance. Engine, process-level MCP and headless
      rendering tests cover sharing, mode boundaries and finding ids; regressions cover confidence
      preservation, explicit overrides and agent-specific errors
- [x] Serialize project-backed investigation load, revision check, mutation and all rendered
      writes under one bounded, cancellable interprocess lock. Refuse malformed state rather
      than replacing it; separate-process regressions retain concurrent appends and reject one
      of two updates based on the same revision
- [ ] Refresh externally changed investigation/evidence records incrementally, avoiding repeated
      full reloads under the session lock while retaining mid-turn findings visibility
- [x] Offer OpenCode locally and over SSH with its own visible chat models and supported thinking
      depths (Low, Medium, High, Xhigh, Max) in settings and headless clients. Choices are validated
      before saving; explicit choices are refused at execution until native MCP support is available.
      See [OpenCode](opencode.md).
- [x] Capture OpenCode settings and project together; refuse a busy session instead of treating
      it as no project, and preserve open model pickers and matching results during contention.
- [x] Report cancelled model validation as stopped, while preserving error events for other failures.
- [x] Finish rejected or cancelled model validation with a terminal event so headless chat exits;
      document functions touched by the OpenCode model-selection changes.
- [x] Integrate the five-level `Effort` type from #395 for agent variants, and share a strict
      disk-based selection save between the hub and headless tools, preserving unrelated edits.
- [x] Rebase onto main after #395's squash merge, preserve extra model arguments without a saved
      selection, and render the remote-host placeholder visibly in the setup instructions.
- [x] Hold the UI settings worker's session guard through its disk save so it cannot overwrite
      a newer model selection; reverify the existing terminal-event regression for validation failures.
- [x] Label the settings-save guard so filesystem I/O is identifiable as a running job by the lock tracker.
- [x] Finish a failed CLI worker-thread start with a terminal event after its error so headless chat stops polling.
- [x] Synchronize the UI exit-time settings save with the session's latest AI table and other writers;
      an isolated exit-save regression preserves the latest model/depth and pending app settings.
- [x] Never wait for a running tool's session lock at shutdown: defer non-AI app edits in a versioned
      sidecar when busy, then recover against the latest committed AI settings at startup.
- [x] Discard superseded deferred settings after a successful full exit save; document model-only
      and model-with-variant command forms, and share transport selection between preflight and runs.
- [x] Rebase after #567 and translate the ten model-selection strings and two agent tool titles
      in the Russian catalogue, restoring coverage and catalogue key parity.
- [ ] Supply and verify the run's Craft Anything MCP server in native OpenCode before enabling
      explicit model/thinking execution (currently refused for chat and builds, locally and over SSH).
- [ ] Verify OpenCode model discovery and an explicit thinking selection on a live SSH host,
      and complete a live generation with the selected model and variant (local catalogue verified).

"Found on this computer" has to mean "can do the job". Today only Claude Code gets everything.
**Every builder gets the same tools:**
- [ ] A table of what each adapter gets (our tool server, approvals, resume, notes mid-turn, a
      message of any length), as typed data, with a test that fails when one is left out.
      `AgentAdapter` already answers some, such as `resumes` and `prompt_on_stdin`. The lines below
      fill it in
- [ ] OpenCode's plain adapter never passes our tool server, so its builder has none of our tools
      and hunts for a setting that doesn't exist (#481). Hand it the server, or build over ACP by
      default once that passes
- [x] Use OpenCode's build agent for builder runs unless extra arguments already choose one; a saved
      plan default produced only plans and left app/ empty. Argument tests and a live file-writing
      run passed
      — claimed: Kimi Code, branch bugfix/r2.2-opencode-build-agent
- [x] Aider has no adapter, so the plain one hands it the prompt as a file name, and it can open a
      browser sign-in nobody agreed to (#72). Give it an adapter, or stop offering it — claimed: Afloat16, branch bugfix/r2.2-aider-recommendations-20261103
- [ ] Add a non-interactive Aider adapter before recommending it again; detection currently leaves it out.
- [ ] Codex (#499): in a chat the tool server has no active target. The reporter attached a patch
      for chat continuity; look at that first
- [x] Codex (#538): list every permitted tool from startup, since Codex doesn't refresh its callable
      list after `enable_toolset` changes it — PR #805
- [x] Codex (#540): `rebuild_start` was refused by `codex exec`'s approval policy and showed as
      `null`; setup and the builder now approve our tools for Codex (we still ask before running
      code) and a refusal explains the fix (`docs/codex.md`)
- [ ] Codex (#540): run `codex exec` on a real install and check `rebuild_start` gets through
- [ ] Codex: resume support (`codex exec resume <id>`), verified on a real install
- [ ] Every adapter but Claude Code's still takes the message in argv (`prompt_on_stdin`), so a long
      one can pass Windows' command-line limit as Claude Code's did (#402); hand each its message on
      stdin or in a file where its CLI reads one
- [ ] A note sent during a build reaches the builder at its next step on other coding agents with
      tool hooks, and on the API-backend loop between tool calls (Claude Code only today)
- [ ] Session copies for other agents that keep sessions (ACP agents' `session/load`) (#451)
- [x] Hermes Agent's native Windows install (`%LOCALAPPDATA%\hermes\bin`) is searched, so an app
      started before the install, or by a launcher, finds it (#116)
- [ ] Live-check #116 on Windows: install Hermes natively, start the app from a launcher, and see
      Settings → AI offer it; #116's "error after every message" has no text yet (asked)
- [ ] See whether Hermes still fails on 0.3.0 (#460: the fix its `fixed` label names was already in
      0.3.0 when it was filed, and it gives no version)

**Approvals:**
- [ ] **decision** One approval behaviour across agents, and whether we want an auto-approve mode
      apart from Clean room and Deep (Auto mode only governs what's asked through us). Today it
      differs by agent: Claude Code asks our approval tool; OpenCode's builder runs with `--auto`;
      Hermes's `-z` turns its approvals off on every run, chats included. Clean room's approval
      rule doesn't reach an agent that never asks us
- [x] Clean room outside builds (#564): a chat or question with Claude Code refuses the agent's own
      file, shell and web tools outright (`--disallowedTools`), not through its defaults; an ACP chat
      refuses them through an approver instead of a consent card (`docs/claude-code.md`)
- [ ] Say on each adapter, as typed data, what it can be held to (asks us, its own sandbox,
      nothing), in a build, a chat and a question, and have runs act on that, once the decision above
      is made. Today Codex, Gemini CLI, OpenCode, Antigravity, Hermes and Maki are not held to
      Clean room in a chat (their own sandbox, if any, is not established)
- [ ] Check live that a Clean room chat with Claude Code can't read a file outside its folder, and
      that an ACP chat in Clean room still reaches our tools (its permission requests for them are
      allowed, only its own read/write/command/fetch are refused)
- [ ] Check live what Hermes over ACP asks about: whether its file reads reach our approval policy
- [ ] Maki's plain `maki -p` builder doesn't ask us before commands, and in Clean room its reads
      aren't checked (as with every agent that never asks us): say so in the approval decision
      above, or run Maki over ACP by default once that passes. Outside a build, tools the person
      allowed in Maki's own permissions (an MCP server's, or all of them with `always_yolo`) still
      run; the adapter can't see that config
- [ ] Maki over ACP writes inside its working folder without a permission request (its built-in
      rule), and that folder is a build's project folder, so writes to our own records (inbox,
      manifest) skip the approval policy; `maki acp` takes no tool flags to stop it (0.7.0). Start
      it in `app/` (paths in the prompt relative to that), or ask Maki for a way to make those
      writes ask; part of the approval decision above

**ACP:**
- [ ] Gemini CLI, opencode, goose via ACP or native adapters; each verified live, then turn ACP on
      by default for the ones that pass
- [x] Over ACP, suppress `session/update` notifications until `session/load` answers, keeping
      replayed messages and tool activity out of the current turn
      — commit `38ffc39be084`
- [ ] Over ACP, turns report 0 tokens: the protocol sends only `usage_update` (context used and
      size). Read real counts where an agent keeps them (Maki: `token_usage` in its session file
      under `~/.local/state/maki/sessions/`) or say the count is unknown instead of 0

**Finding them:**
- [ ] #105, #135, #259, #385 and #423 predate 0.3.0, which looks in more places, and came through
      the ticket relay, so nobody can ask them for `doctor` output. Close them pointing at 0.3.0,
      and add search paths only for a new report that has that output
- [x] #423: an agent that is set up but switched off is never offered again, so "No coding agent was
      found" stays on screen with Claude installed: the setup card and "Look again" leave out every saved
      AI by name, on or off. Offer a switched-off agent as found and turn it on, keeping its options.
      Cause is not confirmed on the reporter's machine (needs their config.toml); the fix covers the
      switched-off case that the code leaves out (`is_on`, `leave_out_set_up`, `builder_backend`). — claimed: fabox-os,
      branch bugfix/423-disabled-agent-not-offered
- [x] Say plainly when what's found is the Claude desktop app rather than the `claude` program
      (#259, a guess at its cause)
- [ ] macOS: check that `Claude.app` is where the desktop app is found (#259's hint is untried there)
- [x] macOS: `/Applications/Antigravity.app` ended up as the agent's command (#467). Found: the "Where is
      the program?" picker (`pick_file`) returns the bundle, and `set_command` saved any path. Refuse a
      folder or app bundle when it's saved (`set_command`), and don't run one that was saved before
      (`program_of` uses the usual name). Tests: `an_app_bundle_is_refused_as_an_agents_program`, `program_of`
      cases. — claimed: fabox-os, branch bugfix/467-agent-program-app-bundle
- [x] Windows: look in `%LOCALAPPDATA%\agy\bin` too (#62) — claimed: Afloat16, branch bugfix/r2.2-antigravity-search-20261103
- [x] Windows: `usual_windows_dirs_come_after_path` counts Antigravity's second folder (#748
      added it; the test runs only on Windows, which CI doesn't)
- [ ] CI runs the tests on Ubuntu only, so `cfg(windows)` tests (search paths, consoles, Job
      Objects) are checked only on contributors' machines: run them on a Windows runner too
- [ ] "Claude Code on <ssh host>" is offered for any SSH host, whether or not it has Claude Code
      (#62)
- [ ] Gemini CLI's sign-in reportedly refuses individuals now (#400): check, and stop recommending
      it in the builder setup dialog if so

### R2.4 Runtime study for apps and games (with consent)
Owner: maintainers · Status: in progress
- [ ] UI tree on Linux (AT-SPI)
- [x] `apple_calls_observe`: the breakpoint callback is one valid line (it had a literal backslash
      and `n` between statements, so lldb stopped before launching, #251), covers every breakpoint
      instead of only the first, a failed lldb run is an error and not "0 calls", and names or
      arguments with line breaks are refused (they'd add commands to lldb's command file). The
      callback body was compiled with Python; lldb itself was not run
- [ ] Run `apple_calls_observe` on a Mac against a small program with `lldb` (the #251 probe):
      the hit for `alpha` is recorded, and a program lldb can't debug says so
- [ ] Window capture tried on macOS and Linux (only Windows so far); games behind a launcher
- [ ] Short scripted input sessions (enigo) to record screens of the original

### R2.5 Behaviour checks for non-web builds
Owner: open · Status: in progress
- [ ] Behaviour checks that use the sheet: run the build with scripted input and measure values
      (speed, jump height, fire rate) against it (needs R2.4's scripted input)
- [ ] Hold the self-test to what the check can see too: picture the game after its scripted
      input and require it to differ from the `--screenshot` picture (a report alone is the
      build's own word)

### R2.7 The request decides how to build
Owner: open · Status: in progress
- [ ] Make: a game whose request names a technology the stack table doesn't know (Phaser, Bevy…)
      still needs the default engine installed to start, though the builder is told to use what
      was named; ask the person, or let such a game start without an engine
- [x] The website check's missing-site note names every place `site_dir` accepts (dist/, build/,
      out/, or a plain index.html with no package.json) and the app's real place, "app/" or "the
      project folder" (#494)
- [x] A project edited in place (Work on my project, or `rebuild --existing edit`) keeps what its
      files are built with, unless the request names another stack: an engine's project file, then
      package.json and Cargo.toml dependencies, then `found_by` in `stacks.toml`, and a project
      whose dependencies name no stack there is left to the builder. A request that read as a game
      used to ask for Unity even in a Godot project, and an app got the kind's default
      — claimed: xerenon, branch bugfix/r2.7-stack-from-files; PR #755
- [x] A person's own project (edited in place, nothing to recreate) is judged by what it does for
      itself: its own build, then its own tests, through `[[toolchain]]` in `stacks.toml` (Rust,
      Node.js, Python, Go, .NET, Maven, Gradle, Make). Nothing is added to it for our sake: no
      `--screenshot`, `--self-test` or mechanics sheet. A site stack with nothing to recreate is a
      website, whatever the request says
      — claimed: xerenon, branch feature/r2.7-own-project-checks; PR #760
- [ ] Run the Go, .NET, Maven and Gradle rows of `[[toolchain]]` on real projects, and every row
      on Windows (`windows_program`): Rust, Node.js, Python and Make were run for real, on a Mac
      only. Also a person's own Unity project: it's opened in batch mode, which should exit with
      an error when its scripts don't compile (not run: no Unity on that machine)
- [ ] A person's own tests that already failed before their request fail the check too, so the
      builder is sent to fix them. If that proves a nuisance, run them once before the first turn
      and hold it only to what it broke
- [ ] A new project in Python, Go or the like still fails the build check ("No project to build
      found") and has no program for the screenshot check: `[[toolchain]]` could build it too

### R2.8 Clean room means clean room
Owner: maintainers · Status: in progress
- [ ] A remote (SSH) builder needs a craft-anything-cli that knows `--deep` and `--clean-room`:
      check its version first and say so when it's older

### R2.9 Everyday use: typing, projects, settings
Owner: maintainers · Status: in progress
Gaps found while using the app for real projects.
- [x] Agent paragraphs with glossary terms and long file links wrap together without pushing
      their card or later messages under the sidebar (#523); Project and Ask bounds checked at two sizes
      — claimed: carl-dourado, branch hotfix/r2.9-523-message-width
- [x] A prompt brought back with Up keeps its pictures (their paths are absolute, so they don't
      come back after the project is moved)
- [ ] Change a project's builder from its page (a tool, so the CLI and MCP can too) — in PR #245
- [x] Ctrl+V (Shift+Insert, Cmd+V) in a prompt box that takes pictures pastes a copied image (saved
      as a PNG) or copied picture files; attachments show as square picture tiles, the whole
      picture on hover, decoded on a worker
- [x] A picture over 20 MB dropped on a prompt box or picked in its dialog is refused when it's
      added, as a paste does (`workspace::attachable_picture`), before anything is copied
- [ ] When the builder decides a question, show the question and its choice as an answered card
      in the feed (the run sees only the tool call today; the builder's words say what it chose)
- [ ] A question's own cost: a forked Claude Code session reports the whole session's running
      total, so a forked answer shows none; count questions in the project's usage
- [ ] "Make this change" on an answer that suggests one (sends it as a normal message)
- [x] #504: the builder's plan card can be folded away with its X, or with `project_plan_fold`
      (between builds: a fold during a build can be overwritten by its next save). The steps stay
      in the conversation (`FEED_VERSION` 4, `FeedItem::PlanDismissed`), and a later run's
      checklist shows again — claimed: claude, branch bugfix/r2.9-plan-card-dismiss
- [ ] A run that ends with steps still open marks them as stopped on its card, instead of
      showing "Doing" until someone folds it (#504: Claude's runs left the plan unticked)
- [x] #666: a long line in a feed's tool-work row (a Deep-mode decompile, a status note) ran off
      the window and pushed the feed left; the header title can't wrap in egui, so it's the note's
      first line now, cut at 90 characters, and the full note wraps in the body — claimed: claude,
      branch bugfix/r2.9-feed-text-overflow
- [ ] **decision** A program started from the CLI (`project_play`) keeps the shell's output pipe
      open, so the command doesn't return until it closes (#207). Every fix found needs `unsafe` or
      an unstable API: an exception to rule 9, or a limit we document. Meanwhile a reproducing test
      marked `#[ignore]` and a line in the CLI docs are welcome
- [ ] macOS: the window draws Windows-style controls at the top right, since the app turns off the
      system's title bar (`with_decorations(false)`), and its icon is bigger than the system's (#20)
- [ ] Show a command's output while it runs: Claude Code reports a tool's result only when it
      ends, so a long `cargo build` is one quiet line until then. Watching the agent's child
      processes from outside could say what's running and for how long
- [x] Release the keep-awake helper before a rebuild reports `Finished`, waiting up to five seconds
      for its supervised cleanup and logging a timeout. Real subprocess stand-ins checked on Linux (#541)
      — claimed: Afloat16, branch bugfix/r2.9-keep-awake-cleanup-20261103
- [x] Recheck keep-awake cleanup with a real Windows CLI `rebuild`: the helper once outlived it by
      more than 9 minutes and held the caller's output pipe; not reproduced in 8 more runs. Found:
      closing the desktop app during a build ends it without dropping the guard, and the helper
      (an endless PowerShell loop, started with a week's time limit) carried on. It now ends by
      itself once the process it serves is gone: the Windows loop checks that process every 30 s,
      and macOS uses `caffeinate -w`. Live on Windows: a CLI build killed outright, its helper
      ended 30 s later. Not run on a Mac
- [ ] Find why a site capture lost its browser ("the WebSocket is closed", 419 ms into
      `web_site_capture` for a second page, seen once; the same capture passed 41 s earlier)
- [x] Delete a private browser's temporary profile when it closes or fails to start (#810) — PR #822
- [ ] Stop private browsers that outlive the app after an abrupt exit, and investigate the Brave
      Helper crash (#810); both still need macOS reproduction
- [ ] Claude Code's Ultracode switch (a setting, not an effort level; the effort environment
      variable we use can't turn it on)
- [x] A model the list doesn't offer (`opusplan`, `[1m]` variants, one another harness serves) can
      be typed in beside the model list in Settings → AI, as `ai model` already allowed (#470)
- [x] The preview server answers only requests addressed to this computer (a Host and Origin
      check against DNS rebinding), streams large files instead of reading them into memory,
      sends no body for HEAD on a missing file, and caps open connections and the time a
      request may take to arrive (#597, #598, #600, #601, by Afloat16)
- [x] The command palette stays where the wheel scrolls it; it follows the selection only when the
      arrow keys or typing move it (#584)
- [x] Linux: draw on the GPU that drives the screen. egui-wgpu's default picks the discrete GPU, so
      on a laptop whose integrated GPU drives the panel the window took 20 s to open and the whole
      desktop lagged (#571). Pick by the connected displays in `/sys/class/drm`: the built-in
      panel's GPU, else the only GPU with displays, else the discrete one; `WGPU_POWER_PREF` and
      `WGPU_ADAPTER_NAME` still decide when set. Checked live on an AMD + NVIDIA laptop only (#578)
      — claimed: Nghiathan13, branch bugfix/r2.9-linux-gpu-choice
- [ ] Linux GPU choice on a desktop with monitors on two GPUs, and on an Intel + NVIDIA laptop:
      covered by unit tests on fake `/sys/class/drm` folders, not run on such a machine yet (#571)
- [x] "Work on my project" on New project, beside "Make my own version" and "Just look inside":
      pick the folder of a project you already have, say what to change, and its files are edited
      in place, with nothing asked; the project takes the folder's name, an empty folder is
      refused and a folder that's already a project opens. Until now that was only reachable
      through "Choose a folder…" under Make's Start, on a screen that asks what to recreate
      — claimed: xerenon, branch feature/r2.9-work-on-my-project; PR #686
- [x] Leaving "Work on my project" forgets a folder it would edit in place and drops a folder
      dialog that ends later, so a version made on Make can't edit that project, and an address
      in its request isn't read as an original to recreate (review of #686) — claimed: xerenon, branch bugfix/r2.9-own-project-review; PR #750
- [x] Choose Claude Code's folder (`CLAUDE_CONFIG_DIR`) per AI, so one computer can build with
      two Claude accounts; an app opened from the Dock never sees the shell's variable. Settings →
      AI (a typed folder, and Add another) and `ai folder`; live-checked from the CLI with an empty
      folder (not signed in, and the fix names the folder) overriding the shell's own variable,
      and by a build in the app with a typed `~/` folder. A sign-in that expired says so instead
      of "isn't signed in" (the folder's `.claude.json` still names the account)
      — claimed: Claude (Opus 5.5), branch feature/claude-config-dir
- [ ] A live build with two Claude Code AIs signed in to two accounts, and one over SSH with a
      `~/` folder (the remote line is unit-tested, not run on another computer)

- [ ] Keep the builder-availability smoke regression independent of live agent discovery.
      — claimed: Afloat16, branch test/r2.9-builder-discovery-fixture-20261104
      Test fixture implemented with controlled discovery; automated validation remains pending.
- [x] macOS: a menu bar with the app's own actions (open a file or folder, new project, settings,
      Simple / Advanced, Clean room / Deep, the command palette, help); the system's menu bar
      showed only the app menu
      — claimed: sanketdofe, branch feature/r2.9-macos-menu-bar
- [ ] macOS: an Edit menu (Undo, Cut, Copy, Paste, Select All). The keys work in text boxes, but
      the system's Edit items need the text box to answer them, so they'd show greyed out today

### R2.10 Pinned builds
Owner: maintainers · Status: in progress
Keep a build that works at hand: pin it, and open it from the sidebar whatever the builder does
to the project next.
- [ ] Say when a light pin ended up copying what it meant to share (a drive without hard links)
      — in PR #220
- [ ] Pins of one project sharing with each other, so a third frozen copy of the same assets
      takes no more space
- [ ] Pin a website (a frozen copy of the built site, each served on its own port)
- [ ] A pin started before the app restarted shows as running (keep its process id and start
      time in the pin, and check both), and can be stopped
- [ ] Cap a run's log (`last-run.log`, `play.log`): it grows as long as the program prints
      — in PR #218
- [ ] Opening a pin that's already running brings its window to the front, instead of a message
- [ ] How much space pins take in Settings

---

## M3 Engine depth

An open lane: anyone may take these at any time.

### R3.14 IDA Deep engine
Owner: open · Status: in progress
- [x] Detect a manually installed IDA and use its headless decompiler in Deep mode, with cached
      analysis, provider selection, evidence and the existing built-in fallback. Live IDA 9.4 on
      Linux x86-64 decompiled `main` and a cached helper; Clean room, error and cleanup checks passed.
      Other platforms and older IDA versions have stand-in tests only
      — claimed: Kimi Code, branch feature/r3.14-ida-deep-engine
- [ ] Map raw-binary processor and load-address settings to IDA's loader instead of falling back

### R3.3 Frida sessions for real
Owner: open · Status: todo
- [ ] Replace the in-memory session bookkeeping with real attach, resume and detach

### R3.9 Game assets out
Owner: open · Status: in progress
- [ ] Unity: sounds out (the stored stream, usually FSB5; PCM ones as WAV)
- [ ] Unity: crunched textures (DXT1Crunched, DXT5Crunched; two of the games checked use them) and
      the float formats (RHalf, RGHalf, RGBAHalf, RFloat, RGFloat, RGBAFloat)
- [ ] Unreal 5 and Oodle: IoStore packages, unversioned properties and Oodle-compressed paks
- [ ] Game files: save pictures from IoStore containers (needs the Unreal 5 support above)
- [ ] Unreal: sounds and meshes out (so a Deep-mode rebuild needs no outside converter for them)

### R3.11 Single-file apps, game files and stand-in backends
Owner: open · Status: in progress
- [x] Verify stand-in session cleanup by the original answerer's lifetime rather than polling a
      released port that another listener may reuse
      — claimed: Afloat16, branch test/r3.11-standin-session-lifetime-20261103
- [ ] Stand-in backend: writes that change later reads (recorded answers are replayed as they are)
- [ ] Tell the builder about the stand-in backend in a website's brief, and start one from a
      recording
- [x] Stand-in backend: retain 304 samples without a usable cached body for diagnostics, skip them
      during replay, and report `cached_body_unavailable` when the best matching group has no
      usable answer. Build results count them; cached bodies still answer as 200 (#543)
      — claimed: Afloat16, branch bugfix/r3.11-standin-empty-304-20261103
- [ ] Godot format 4 and IL2CPP metadata 38 and 39 are header-only; read their tables once a real
      file can be checked
- [ ] Bun: take source maps and bytecode out as files; a standalone `.pyz`
- [ ] `native_source_folder_write` uses the built-in decompiler only: use Ghidra in Deep mode when
      it's there
- [ ] `native_publisher_show` hashes the file minus the signature fields, not section by section;
      a file with unusual gaps between sections can read as different
- [x] Enforce stand-in per-body and total byte budgets on the text that is actually retained after redaction;
      deduplicate consecutive samples before charging the capture budget.
      — claimed: Afloat16, branch bugfix/r3.11-standin-redacted-budget-20261103b
      Revision `d601bc4e` passed [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41788735130):
      all four retained-budget regressions and six deduplication regressions passed.

### R3.12 Format parser correctness
Owner: open · Status: in progress
- [x] Parsers refuse what they used to half-read: binary plists keep objects inside their object
      area and reject unrepresentable widths, base64 (plists, source maps) refuses damaged tails
      and incomplete quanta through one shared decoder, OIDs must be minimally encoded, and string
      offsets past the address space are left out. Tar keeps old GNU timestamps out of names and
      PAX sizes across other metadata records; CRC cpio checks member data; source maps decode
      percent-escaped data URLs and report indexed-map columns in the generated file
      (#606–#615, by Afloat16)
- [x] Bound and cancel CRC cpio verification across firmware probes, and reject non-hex checksum
      fields — claimed: Afloat16, branch bugfix/r3.12-bound-cpio-crc-scans-20261103
- [x] More parsers keep to the bytes they declare and refuse impossible values: signed LEB128 past
      64 bits, damaged modified UTF-8 (valid text around it survives), widened Java instructions
      and invocation operands, DEX register lists and array widths, SQLite text in the database's
      encoding with cells kept inside their page and overflow cycles stopped, Android string pools,
      ZIP output limits for stored entries, decoded sizes and ZIP64 fields, ASAR header framing,
      ROMFS dot roots, cramfs, ext and JFFS2 sizes and holes, PyInstaller modules outside their
      PYZ, JSON config edits (comments kept, every duplicate key removed), and UDP, TCP, DNS over
      TCP, pcapng and TLS lengths (#688–#715, by Afloat16)

- [x] Address the combined parser review: preserve partial Java reference coverage,
      enforce DEX signed 32-bit LEB128 limits, recover legacy AAPT UTF-8 lengths within
      their pool with a shared recovery probe limit, and distinguish missing ext indirect
      metadata from sparse holes.
      — claimed: Afloat16, branch bugfix/r3.12-parser-review-followups-main-20261104
      Shared-budget revision `29c48e03` passed [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41762780268).
      The merge with main `41d2175e` awaits a new CI run.

### R3.13 Web inspection correctness
Owner: open · Status: in progress
- [x] The HTML scanner finds script preloads in `rel` token lists, decodes asset URLs once (numeric
      and named references), keeps script and title text through lookalike closing tags, and skips
      raw-text elements, markup inside attribute values and custom tag names such as
      `<script:widget>` (#591–#596, by Afloat16)
- [x] HAR reports redact credentials whose query names are percent-encoded (and the stand-in
      backend matches older recordings the same way), redact declared secrets in every kept field
      (names, methods, hosts, versions, timings), count body bytes before text conversion, and
      template only real calendar dates in API paths (#602–#605, by Afloat16)
- [x] Preserve empty query pairs when matching older stand-in recordings, without treating raw
      query separators as pairs — claimed: Afloat16, branch `bugfix/r3.13-preserve-empty-query-pairs-20261103`

---

## M4 Release

### R4.1 A macOS CLI that opens
Owner: maintainers · Status: todo
- [ ] **decision** A standalone macOS CLI that can carry its ticket, such as a stapled `.pkg`; that
      needs a Developer ID Installer certificate in the release secrets

### R4.2 Managed tier
Owner: maintainers · Status: todo
- [ ] **decision** Managed tier: account backend, usage metering (Claude API under our own key only)
- [ ] Before the managed tier or any paid offering: confirm the Claude Code orchestration question
      with Anthropic (claude-code.md)

---

## Parked

Looked at and not committed to; none of this is open work. Say in the issue if you want one.

- From the tool survey: firmware image kinds (ESP32, U-Boot, device trees, Intel HEX), Erlang BEAM,
  GPU code objects, kernel modules, GameCube DOL, Mac crash reports, SAZ and qlog captures, protobuf
  bodies, R8 mappings, JNI links, MCP over HTTP
- Ideas from issues: resume by itself after a usage limit (#476, the one tied to a real problem), a
  source folder as a target (#480), actions after a build ends (#495), WebAssembly disassembly
  (#175), an HTTPS intercepting proxy (#164), VM and Windows Sandbox detonation (#144, #163), an
  adapter for another disassembler (#147)
- More providers and agents (#14, #63, #97, #162, #170, #186, #206, #470): R2.2's table comes first,
  so each new one is a small addition rather than another half-wired agent
