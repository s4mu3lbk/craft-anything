# Status

What works today and how it was checked. The plan, owners and order of work are in
[ROADMAP.md](ROADMAP.md). Last updated 2026-11-06. The baseline audit on 2026-10-23 from `main` had 923 tests passing;
workspace clippy, layers, fmt and the secret scan of all history were clean. Chinese UI checks are listed separately below.

## What works, and how it was checked

| Area | State | Verified by |
|---|---|---|
| OpenCode builder mode | Builder runs explicitly use OpenCode's `build` agent instead of inheriting a saved `plan` default, while an explicit `--agent` extra argument still wins. This prevents detailed plans followed by an empty-`app/` error | Argument tests cover the default and override. A live OpenCode run in an empty temporary folder wrote `smoke.txt` with `ok`; earlier authentication failures are separate and still require valid provider credentials |
| Java decompilation | `java_vineflower_decompile` decompiles a real JAR to source with the pinned Vineflower 1.12.0, run through the configured Java | Live run on Linux x86-64 (OpenJDK 21.0.12.1) against Apache Commons IO 2.16.1 (497 KB, 374 entries): 253 source files, exit 0; `org.apache.commons.io.FileUtils` returned 1,350 lines with the class and its file-reading methods present. The engine install matched the pinned SHA-256. Not checked: Windows and macOS, a `.class` input, or a second JAR |
| Conversation width | Agent paragraphs, inline code and fenced code stay within the message column, including glossary terms followed by long file links (#523) | UI regression checks painted text and subsequent answers against the composer bounds in Project and Ask at 1920 × 1032 and 1280 × 820; Unicode/formatted glossary hover check |
| Settings saved on exit | Pending app settings merge the session's latest AI table when its guard is available. A busy session defers non-AI edits in `config.pending.toml` without waiting for the tool; startup recovers them with the latest committed AI table | Isolated child-process regression exercises idle and busy exit saves, a late AI commit, startup recovery and pending non-AI edits; engine tests cover sidecar migrations and damaged/newer files. No live desktop shutdown was exercised |
| SQLite payload limits | Capped records retain column positions, disclose partial TEXT and BLOB values, omit incomplete BLOB hashes, and report partial coverage with read notes | Hand-built overflow-page regressions for five oversized records (including WITHOUT ROWID in declaration order) and two controls; table and index engine coverage and limitation regressions |
| SQLite added columns | Short records include missing trailing fields using literal column defaults, or NULL when absent; unsupported default expressions produce NULL with a row note. Stored fields and records longer than the schema retain their values. Defaults are filled before restoring rowid aliases | Nine hand-built parser regressions and an engine tool test cover added-column defaults, rewritten records and short records with a rowid alias, including a missing alias field |
| OpenCode models and thinking | OpenCode V2 locally and over SSH; Settings → AI lists only that instance's enabled chat models and supported Low/Medium/High/Xhigh/Max variants. Selection is validated and saved through UI, CLI and `ai_models_list` / `ai_model_set`. Explicit-selection execution is refused until native MCP integration exists; local ACP runs require clearing the selection. See [OpenCode](opencode.md). | Stand-in catalogue/process, persistence, remote-command and UI tests; local live catalogue checks (34 models, all five depths represented). Regression checks reject native chat/builds before process I/O with ACP on/off, local/SSH and model-only/depth-only/combined selections. No live SSH host or paid generation used |
| Claude desktop app without Claude Code | When the `claude` command isn't found but the Claude desktop app is (Windows: the Store package's `claude-desktop` app alias or the older installer's `AnthropicClaude` folder; macOS: `Claude.app`), the builder dialog under "No coding agent was found" and `ai detect` (on stderr) say the desktop app is installed but building needs Claude Code, installed apart (#259). Typed as `DetectHint` from `AiHub::detect_hints` | A regression finds the app in temp folders laid out as each Windows installer leaves it, and nothing on Linux; a UI test shows the hint in the dialog. On Windows 10 the real Store package's alias was found. Not checked: macOS, the older installer, and a computer with no `claude` installed |
| Missing program paths | A program started by its path (a saved agent program, for one) that has no file there is reported with that path, as possibly moved by an update, with reason `path_missing`, instead of "isn't installed or isn't on your PATH"; a bare name still speaks of PATH. A relative path is looked up where the program starts (on Unix, its working folder), so a script there whose interpreter is missing isn't called a path with no file. It stays a missing program (`EngineMissing`), so builder turns aren't retried (#721). `ai test` names the agent program it ran and the saved one it replaced (`program`, `saved_program_replaced`), and `doctor` says a saved path has no program there | `a_saved_program_that_is_gone_is_named_by_test_and_doctor` runs both with a removed path. A regression starts a program at a path with no file on Windows 10 and checks the message, remediation and reason, with a bare-name control; on Linux (Docker), a relative script with a missing interpreter started in its folder, through `Process` and `Launch`. Not checked: a real Codex desktop update, or the app's Settings screen with a stale path |
| Mach-O Objective-C pointers | Chained offsets mapped only into file-empty `__PAGEZERO` reach the image-base fallback; pointer candidates must be backed by file bytes | Synthetic thin and fat Mach-O regression, plain-pointer and authenticated-rebase controls, null and unmapped-pointer checks. Real binaries were not checked for this correction |
| Apple app graphs | Selecting a macOS bundle recognises its root-relative `Contents/Info.plist` and retains its service/extension type; nested XPC bundles appear in `services` and as `xpc-service` graph nodes, including their executables when hosted in frameworks | Synthetic inventory tests, including parent-directory and flat bundle layouts, selected services/extensions, framework-hosted service executables, and bare-program controls. Ordinary selected directories with `Contents/Info.plist` need an Apple bundle suffix; archives retain layout-based recognition. Framework libraries and dylibs stay excluded from executables. Real app bundles were not checked for this correction |
| Chinese UI | Simplified (`zh-hans`) and Traditional (`zh-hant`) Chinese in Settings → General → Language; System chooses a supported OS language, otherwise English. Both catalogs cover UI controls and tool titles; translated titles are searchable. Raw tool data, AI text, logs, CLI/MCP and the separate browser viewer keep their original text. See [localisation](localization.md). | Catalog integrity and locale-resolution tests; `cargo xtask i18n-coverage`; isolated UI language-switch test. Platform visual checks are recorded separately. |
| Engine | 300 engine tools (320 with the AI and setup layers') across native, managed, Java, Android, Apple, JS/Electron, web, firmware, packets, databases, game archives and assets; see [tools.md](tools.md) and [tool-status.md](tool-status.md). A saved result can be narrowed without running again (`core_evidence_browse` keeps the rows matching given field values, cut to chosen fields) | A functional audit that ran every tool on real inputs on Windows, plus 600+ unit and integration tests |
| Tool argument correction | Shared parameter parsing keeps schema types strict and reports the rejected value's JSON Pointer when known, the original type error and guidance to correct the call. Unknown locations are null; a root failure uses the empty pointer | Parser regressions cover string/array mismatches, nested indices, escaped keys, oversized paths, enum locations, defaults and strict fields. Shared engine dispatch is checked for Agent, MCP and CLI callers: rejected arrays create no record, and corrected input creates one. No live Qwen or Ollama build was run for this correction |
| German UI | German (`de`, informal "du") in Settings → General → Language; System picks it for a German OS language. The catalog has the same keys as the Chinese ones. Drafted with an AI, not yet reviewed by a native speaker. Requests such as "Ein einfaches Snake-Spiel" or "Eine Website für …" keep their project kind. See [localisation](localization.md). | Catalog and locale tests, key-set comparison with the Chinese catalogs, the isolated language-switch test, a kind-detection test; fmt, clippy, layers, locks and secrets are clean. Not checked: how the German texts look in the running window (length, line breaks), and the `de.tsv` wording by a native speaker. |
| Java class usage | `java_class_users_find` includes the component class of multidimensional object arrays; primitive-array descriptors do not match an unrelated class with the same descriptor letter | Synthetic class files parsed by the built-in reader: one- and two-dimensional allocations, class names beginning with `L`, primitive arrays, hit limits and method filters; formats tests. This correction was not checked with javac output or a JVM |
| Single-file apps and game files | `core_packed_files_list` and `core_packed_files_unpack` read Bun and Node.js single-file programs, .NET single-file apps, PyInstaller and Godot packs through one seam (`formats::packed`); `formats_bytecode_show` reads Python bytecode, `game_unity_code_names_show` Unity IL2CPP metadata, `formats_ai_weights_show` GGUF and safetensors headers. `app_folder_explain` says what an install folder is made with and what to run next | Unit tests with damaged files. Real files: a 279 MB Bun program, Node blobs, PyInstaller programs (Python 3.9, 3.14), .NET 8 single-file apps (macOS, Linux, Windows, compressed), Godot 3.6, 4.3 and 4.5 packs, 214 Python modules against CPython's own reader, a GGUF and a safetensors file, about 25 installed Mac apps. IL2CPP on synthetic files only |
| esbuild module dependencies | Recognised CommonJS and ESM wrappers keep their dependency edges after identifier minification; semantic bindings distinguish shadowed and reused names, and repeated calls produce one edge. Ordinary calls do not consume the 4096-wrapper dependency budget, including when the wrapper is declared later | 50 JS tests pass. Synthetic regressions cover short names, forward declarations, both wrapper kinds and function/arrow bodies, shadowed parameters, reused names, unrelated calls and duplicates. Public analysis fixtures include 4096 distinct ordinary calls before a wrapper dependency and 4097 real wrapper dependencies to check the cap and first-seen order. No real minified application was run for this correction; sources above the existing semantic-analysis size limit retain name-based resolution |
| Program facts across builds | `native_library_calls_list` (delay-load too), `native_offered_symbols_list`, `native_publisher_show` (checks the file hash and the signature, not Windows trust), `app_builds_line_up` pairs functions across two builds and says how, `native_source_folder_write` writes pseudo-C to a folder and picks up where it stopped, `android_components_list` | Unit tests. Real PE, ELF and Mach-O files; two builds of a 276 MB program; a signed Microsoft DLL; Android components from one real APK, spot-checked |
| ARM32 value flow | A32 and Thumb calls define ARM scratch registers, and word results flow through `r0` into returns in `native_value_flow_show`. Conditional A32 calls retain the earlier value when skipped and a possible returned result; calls to known non-returning targets keep only the skipped path | Synthetic instruction fixtures for leaf results and indirect calls in both modes, with AArch64 and x86-64 controls; direct and indirect A32 conditional calls within and across basic blocks, including a named `abort` target and unconditional controls. This correction was not checked on real ARM binaries or devices |
| Stand-in backends | `web_standin_build` turns a HAR into `standin.json` (secrets dropped, varying path parts templated); `web_standin_start` answers from it on loopback; `web_standin_show` lists what it had no answer for. A 304 without a usable cached body is counted at build time and skipped during replay; if its matching group has no usable answer, requests get a 404 with `cached_body_unavailable`, also shown in the request log. Writes replay what was recorded and don't change later reads. Older saved query variants keep explicit empty pairs when credential rules are reapplied; raw separators remain ignored and the 200-pair limit matches capture. App maps list GraphQL and tRPC operations; `web_walkthrough_check` checks a scripted session without a browser | Unit tests, real loopback requests, and a run with curl on a hand-written HAR. The 304 correction passes 99 analysis tests and a real-loopback regression, covering mixed samples, omitted bodies, matching boundaries and existing v1 files. [Hosted CI for #543](https://github.com/storytold/craft-anything/actions/runs/41585168291) also passed the full engine tests and workspace gates. Public capture/replay regressions cover legacy empty pairs, raw separators and the query-pair limit. No browser-made recording or real front end yet |
| Websites as targets | `web_site_open` keeps a private browser for the session and deletes its temporary profile when the browser closes or fails to start; every browser tool inspects that page; `web_site_capture` records screenshots at each width (desktop, tablet and phone), rendered HTML, design tokens (colours in hex, spacing, content widths), the CSS breakpoints, the layout at each width (page width, heading sizes, each section's padding, gap, content column and items per row), page health (script and console errors, failed requests) and an accessibility outline, layout outline, assets with font files and requests, links and form outlines. In Clean room the capture keeps only what a visitor sees (no HTML, CSS rules, scripts, requests or errors), and opening a site reports no load errors | Live runs on example.com and a local page with Chromium on Linux, including profile cleanup after success and failed startup; unit tests. Brave on macOS was not run |
| Electron window comparisons | `app_graphs_compare` retains every unassigned window and every construction with a reused local binding. Windows are paired by module, binding and occurrence, so inserting comments does not hide preference changes. Colliding display labels receive a separate `#N` suffix without changing literal binding names or occurrence indices | Synthetic applications parsed through the ordinary analyzer exercise anonymous and reused bindings, shifted source lines, unequal counts, literal names resembling occurrence or collision suffixes, and stable labels when only one window changes. Pairing remains positional within each binding group; reordered constructions and real application releases were not checked for this correction |
| Rebuild workflow (Study → Build → Check → Refine) | `crates/ai/src/rebuild/` (kinds, checks and stacks are pluggable: docs/extending.md). Three kinds: website, app, game (a project whose stack is a game engine is always a game). The request decides how to build: a language, framework, engine or platform it names wins over the kind's default stack (Rust + egui for apps, Unity for games, the original's for websites), and the build check, the recorded stack and Play follow what was built (an Electron app opens in its own Electron; a plain static site has nothing to build and passes the build check as it is). Websites: Study also captures up to 4 other pages of the site; menus are opened on a computer and on a phone; the check serves the build on loopback and compares every page at 1440, 768 and 390 px (likeness = mean of layout and perceptual detail, SSIM, averaged over pages, leaving out the parts of the original that change by themselves, found by photographing it three times; the note adds text, page structure, the colours, font, text sizes and spacing the build misses, and per width a page that scrolls sideways, heading sizes and sections whose padding, items per row or content column differ, the errors, failed requests and accessibility gaps the build adds, and the opened menus and pop-ups). A stack or library named in the request (Next.js, shadcn/ui, Tailwind CSS…, `stacks.toml`) is what the builder uses; with none named, a website's stack is the builder's choice (told to match what the original is built with) and the stack it used is recorded from app/package.json. The builder leads: it studies, asks the tools follow-up questions, records what it can't find out as unknowns (`core_unknown_add`; the run reports the open ones), and tests its own build mid-turn with `rebuild_check` (the same checks, for every kind); a check with anything it couldn't compare, or any claim that fails (a website page that scrolls sideways, logs new errors, fails to load a file, lacks a landmark, or a menu that doesn't open; for games each self-test step, for Deep-mode sheets each original number in the code, for console apps the same exit and output as the original's runs), isn't good enough; apps and games by whether they build. Up to 6 refinement rounds, or until a website reaches 90% likeness. A turn cut off by a time limit, crash or dropped connection resumes (up to 3 times); builder turns may run 24 h. A project copied from another computer (or moved to another folder) keeps building with Claude Code: the project keeps a copy of the builder's session (`agent-session.jsonl`) after every turn and puts it back where Claude Code finds it on a computer without it, so the same conversation carries on (#451); when the session still isn't there, a new one starts from the brief, what's in app/ and the kept requests and replies (or the last summary), and questions about the build put the kept session back too, else are answered from the files and forget the missing session (#452). Live runs with Claude Code 2.1.316 on one Mac (2026-11-02), a second computer stood in for by a separate signed-in `CLAUDE_CONFIG_DIR`: a copied project's session was put back and resumed (the builder recalled its first reply from memory); a kept copy of another session was refused and the new session used the handoff; a real project synced from Windows whose session was missing started afresh and reached Ready. Not yet run between two real computers | Unit tests for prompts, stack choice, site detection, feed folding; live runs reached Build and decompiling (below) |
| Context-summary usage | Successful API context summaries contribute to the message totals consumed by build usage and receive a `model_turn` journal entry with `purpose: context_summary`. Cached-token pricing is retained; an unknown summary price makes the total unknown, and a failed call adds no invented usage | Scripted-model tests exercise the real `ApiConversation`, transcript resumption, exact input/output and cache-price totals, journal entries, empty summaries, local zero cost, unknown prices and failed summaries. No live provider or end-to-end rebuild was used for this correction |
| Local model builds | Not yet: Ollama 0.40.7 with `qwen2.5-coder:14b` (32k context) writes its calls as JSON text, which run whole or after a sentence; by check 3 its project compiles, but `main.rs` is still cargo's "Hello, world!", so the screenshot check fails it at the round limit (R5.4). A lean local builder's prompt asks for no memory tools it isn't offered, and a project one folder down is named in the check | Four live CLI builds of "a scientific calculator" on Windows (RTX 5090), 2026-11-04, one fix added per run |
| Backend context budgets | `context_tokens` in `[[ai.backends]]` controls when an API conversation is trimmed and summarised. Zero or omission retains local/cloud defaults; a nonzero conversation override takes priority. Before each attempt, including a fallback, fitting adds estimated history growth since the last successful request to its reported input size. Trimming resets that reference, and successful requests refresh it; summary usage remains in message totals | Regressions cover settings and model construction, defaults/overrides, smaller fallback budgets, large-budget arithmetic and new tool output growing beyond a fallback budget even when the history estimate alone is below the reported input. The latest tool results are preserved, and a short follow-up does not count already-sent history twice. This does not set server context parameters or guarantee that a complete request fits; no live provider was used |
| Games | Engine detection for 12 engines (Unreal, Unity, Godot, Source, GameMaker, RPG Maker, Ren'Py, MonoGame, LÖVE, CryEngine, Cocos, Cube) with versions. Unreal and Godot games are rebuilt in the same engine, others in Unity; installed engines are found, Godot installs itself, others get guided installs. Picking an inner folder (Engine, Binaries, Paks…) finds the game's folder. Just look inside a game shows what it's made of (`core_target_triage` on its folder: engine and version, how its code ships, the libraries its Unity assembly names give away, its own code, scenes and size) and opens it in the chat to ask about it. The builder keeps `app/MECHANICS.md` and its structured twin `app/mechanics.json` (checked after each build), in Deep mode only, may reuse the original's readable files for private use (never decrypting; generated pictures and 3D models replace its art when the person turns them on; Clean room copies none of its files), lists them in `CREDITS.md` (checked, with the private-use line in `README.md`); a game is ready only when its `--self-test` shows the player moving, acting and the HUD drawn, and the result can be started from the project page | Unit tests; live runs on Windows |
| IDA Deep decompilation | A manually installed IDA is discovered through `engines_list` and `doctor`, with an optional path override and an `engines.deep_engine` choice of `auto`, `ida`, or `ghidra`. Auto prefers a working IDA decompiler, then Ghidra. An original IDAPython bridge runs headlessly only in Deep mode, caches analysis by target/profile, returns functions and pseudocode through `native_function_pseudocode`, and records engine id/version and rebasing in evidence. The vendor's configured licence and Python are used without bypassing either; the original target is not modified. Raw-binary loader mapping remains open in R3.14 | Live IDA 9.4 on Linux x86-64: real CLI decompilation of `main` and cached `triple` from a small C ELF, with `environment.engine = "ida"`, version 9.4 and correct addresses; an invalid address returned an actionable error; Clean room refused before opening the target; no IDA process or runtime scratch remained. Unit regressions cover discovery, settings, arguments, cache completeness, evidence and renames. Windows, macOS and older IDA versions were not run live |
| Clean room and Deep mode | Clean room (the default) runs only the tools in `engine/src/clean_room.rs`: ones that touch no target or watch from outside (the website as a visitor sees it, pictures of a window, its controls, recorded runs, a phone's screen). Every other tool, a plugin's included, is refused in the app, CLI, MCP and agents, with a way to turn on Deep mode. A Clean room build studies an app or a game only from the person's pictures and words (and a picture of its window when runtime study is on); its builder's reads and commands go through an approval rule that keeps it out of the original's folder and site (with Claude Code, and with ACP agents for what they ask about; other agents aren't held to it, see roadmap R2.2). Deep mode (`deep_mode` in Settings, `--deep` for one CLI run) uses every tool: the builder sees them all, and a website's study also reads its JavaScript and watches its start page for ten seconds (capture/how-it-works.md). Ghidra adds its decompiler in Deep mode when it's installed: it is found, or installed after asking. Study starts one analysis of the main program, saved and shared with the builder's tool server through a lock file (up to 8 h, Java gets half the machine's memory or the amount set in Settings → Engines, 4 to 31 GB, four slow passes skipped). The builder must decompile before the build is checked; addresses the built-in analysis can't place go to Ghidra, which makes a function on demand (for this run only) where its analysis has none, e.g. code reached only through tables of pointers. Without Ghidra, Deep mode uses the built-in engines | Live runs with Ghidra 12.1.4 on Windows (a shared analysis, and a decompile in seconds on a large program). Live runs with Ghidra 12.2 on Linux x86-64 (decompiles by name in position-independent programs and shared libraries, whose addresses Ghidra moves). The guided install of Ghidra and its Java is offered on Linux and macOS too, but has only been tried on Windows. Clean room and Deep mode: engine, AI, MCP, CLI and app tests; no live Clean room build run yet |
| Auto mode | Settings → Safety, on by default. Builder actions go ahead without asking, except a fixed set that is never done unasked (deleting outside the project, system changes, publishing…); questions get "decide yourself"; asset spending stops at the budget | Unit tests |
| Generated assets | Off until the person turns them on for a game (the switch on Make and on the project, `project_assets_allow`, `rebuild --generate-assets`, saved in craft.json; turning it on without a fal.ai key asks for one); the brief and the asset tools need both that and a key. `assets_image_generate` (Nano Banana Pro by default) and `assets_model_generate` (picture → Tripo P1 low-poly GLB with a face limit) through the person's key for each asset provider (fal.ai today); per-project budget, $5 by default (`[assets] budget_usd` in config.toml); models and prices from the `assets.toml` table (picture model selectable: Nano Banana Pro, FLUX dev or schnell; priced per picture or per megapixel); every call reports its cost | Unit tests; a live 3D generation with a real key (picture, then model) with FLUX dev; Nano Banana Pro not yet run with a real key |
| Claude Code builder | Runs the person's own `claude` in headless mode (Opus 5.5, high effort unless configured) with our MCP server, a permission-prompt tool and `core_person_ask`; preflights for our CLI (a build stops plainly if it's missing or won't start) and sign-in | Argument and stream-parser tests; live preflight; live builder turns; see [claude-code.md](claude-code.md) |
| Agents over ACP | Opt-in per backend (`acp = true`): Gemini CLI, Qwen Code, opencode, goose, Hermes and Maki in their ACP mode, one agent process per chat or build with our MCP server; Maki's ACP mode takes the backend's model (`cli_model`). In chat, permission requests become consent questions; in builds they go through the same approval policy as Claude Code's (inbox, Auto mode). While a saved session loads, replayed updates are withheld from the new turn | Protocol unit tests against scripted agent messages and an approval-policy test. Run live with Maki 0.7.0 only (2026-11-02, Linux, headless CLI): a static page built to Ready in 38 s, a follow-up continued the same session with `session/load`, and with Auto mode off its command and tool requests were refused in an unattended run. Replay suppression passes scripted load-history, success/error and cancellation tests; this correction has not been rerun with a live agent. Turns report 0 tokens over ACP |
| GitHub Copilot | Installed `copilot` through ACP by default, with CLI-owned login (no credential import), explicit Craft Anything MCP tool allowlist, model selection and fresh read-only build questions. Target handoff retains the last 128 entries with the active target last; context discovery respects tool cancellation and shared picker warnings name the agent | Authenticated builds, model discovery and real-account session resume confirmed by the person using the app. Target-boundary, cancellation and agent-warning regressions supplement unit tests and an offline CLI 1.0.105 compatibility test covering all five access modes. See [copilot.md](copilot.md) |
| Shared investigation for local agents | A worker saves session-only investigations before starting MCP, shares open targets in a mode-aware handoff, and reloads externally recorded evidence/findings. Builder refresh failures warn rather than ending the turn. The workbench Evidence pane shows `fnd_*` records with proof links; `core_finding_edit` preserves ids and omitted confidence while checking evidence and stale revisions | Engine persistence/correction tests, process-level MCP target tests in both modes, and headless findings-rendering test; live authenticated target-to-finding workflow confirmed by the person using the app |
| Copilot model options | Shared agent catalogue, model/thinking picker, `cli_model`/`cli_thinking` and `ai_models_list`/`ai_model_set`; selected-model reasoning metadata reloads without a prompt. Context tiers remain separate through `ai_model_options_show`/`ai_model_options_set`, preserving thinking. Legacy reasoning settings remain readable | Isolated catalogue/validation, flags, persistence, stale-result and headless UI tests. Previous authenticated metadata checks used CLI 1.0.105; no new live-account run after the shared-picker rework. Numeric context limits are not advertised in its initial ACP metadata |
| Investigation writer coordination | Every project-backed mutation holds one OS file lock over load, revision validation, mutation and state/Markdown writes. Waits are bounded and cancellable; unreadable state is not overwritten | Separate-process append/revision-race tests, matching Markdown renders, lock timeout/cancellation/release and malformed-state regressions |
| WASM plugins | With `--features wasm-plugins`: `[[plugins.wasm]]` components exporting `craft-anything:plugin/tools` add tools, and ones exporting `craft-anything:plugin/checks` add checks run after each kind's own (they get the build's file list as JSON) (added, listed, turned on or off and removed in Settings → Plugins or with `plugins_add`/`plugins_remove`/`plugins_list`); no imports, fresh instance per call, fuel and memory limits | Tests with components written in WAT (describe, call, checks, a looping call stopped, imports refused, end to end through the engine); `examples/wasm-plugin-wordcount` built from Rust and loaded |
| WebAssembly tag interfaces | `formats_wasm_compare` resolves retained imported and defined tag exports to their declared parameter types. Identical unresolved or invalid diagnostics may still count as unchanged; those counts do not establish interface compatibility, and invalid/partial limitations remain present | Synthetic core modules with tag changes and re-exports, mixed index spaces, import/tag limits, and missing or non-function type references; formats and engine tests. No real module corpus or WebAssembly execution was used for this correction |
| Questions and approvals | A file inbox per project; `core_person_ask`, `core_action_approve` and MCP consent requests become cards in the app | Cross-handle round-trip tests, a full approval round trip through the engine |
| Runtime assets in pinned builds | Frozen and light pins, size estimates and project copies keep runtime files in `obj` folders. An `obj` directory beside a regular C#, F# or Visual Basic project file is still recognised as compiler output; ambiguous folders are retained. New pins record the direct-launch program relative to the copy so a newer helper cannot replace it; older pins retain discovery behavior | Synthetic filesystem tests cover both pin modes, model bytes, compiler-output controls and project duplication. A Linux tool-level regression starts temporary scripts through `project_pin_open`, with a newer `Build/obj/helper`, and verifies the originally selected program and its output; a missing recorded program is refused. No real pinned application was launched for this correction |
| Desktop app | Simple mode: sidebar with New project and the project history (kept across restarts); New project makes something, works on a project the person already has (Work on my project: its folder's files are the app, edited in place without a question, and the project takes the folder's name), or just looks inside an app or file, a website or a game's files; a Game files screen (a game's archives; pictures from Unreal paks, Unity .assets files and Unity bundles saved as PNG after the person confirms they may unpack the game; uncompressed textures and the block formats DXT1, DXT5 and BC4–BC7), make prompt with picture attachments (picked, dropped, or pasted with Ctrl+V, shown as picture tiles) and, for anything recreated, a Clean room / Deep choice (also in the title bar; in Clean room, Just look inside, Game files, opening a file and the workbench ask for Deep mode instead), project screen (steps with the time spent and the AI cost and tokens of every build, saved in the project's usage.json by the run (a coding agent's figure is shown as an estimate at API prices, not the bill), what the build is on now and its activity as it arrives (kept in `build-activity.log`), pinned builder checklist (its X folds it away between builds, kept in the project, `project_plan_fold`; a later run's checklist shows again; its steps scroll past a height set by dragging the divider above it, or with Up and Down, kept across restarts, #826), feed (each message and answer has a copy button, shown on hover or keyboard focus, #825), question and approval cards, a Continue card when a build was paused or stopped part-way (the app or computer closed; the project shows Interrupted) that carries on the builder's own session instead of starting over, and "It's done" beside Continue or Try again to keep the build as it is (`project_build_finish`), checks with screenshots and the requirements not met yet, the builder's open questions, preview, play, folder, follow-ups while it builds, which Claude Code takes at its next step; Up in the prompt box takes back the last note still waiting). Projects can start in a folder the person picks, whose files are either the app, edited in place (its stack read from those files unless the request names another, and, with nothing to recreate, judged by its own build and tests, whatever it's written in), or material to start from (asked, or the answer remembered in Settings), and are listed from any number of folders (`project_folders`, `project_list`, `rebuild --project-folder`). Make says which AI builds ("Built by": automatic, or any AI that can build, by name); a pick stays with the project and builds its later changes. With nothing to build with, Start offers the coding agents found on the computer. Settings → AI turns an AI off or removes it, and sets how hard the Claude API or Claude Code thinks (every Claude level, low to max; also `ai effort` in the CLI). Each Claude Code AI can keep its sign-in in a folder of its own (its `CLAUDE_CONFIG_DIR`, so its own account; `ai folder` in the CLI, a folder on that computer over SSH), used for its runs, its sign-in check and its kept session, and Add another adds a second Claude Code for a second account. In the prompt box, Up at the very start and Down at the very end go through earlier prompts when no note is waiting. Questions about a build ("Just ask" under the prompt box, or a message starting with `/btw`, the first slash command; `rebuild_question_ask`) are answered in the conversation, where each folds away and is kept in the project, without a build or a round and changing nothing: Claude Code answers from a read-only copy of the builder's own session. Under each project's prompt box, "Builder's questions: Ask me / Decide yourself" chooses whether the builder's questions come to the person as cards or are decided by the builder, which says what it chose (`project_questions_set`; the default is in Settings and follows Auto mode until picked). Projects can be renamed, duplicated and deleted (right-click in the sidebar, or the project's page; a delete keeps the person's own files). A build can be pinned (Pin this build on its page, or `project_pin_*` from the CLI and MCP): a frozen copy in the project's pinned/ folder (the default) or a light one that shares dependencies and media with the project through hard links (each one's size shown first), opened from the sidebar's collapsible Pinned section (icons with a running dot when folded); a pin or Play runs once per folder unless another copy is asked for, and a start that quits at once says so with its last output. Typed drafts survive a restart. In a narrow window the sidebar folds to icons and the project's details move above its feed; messages float over the screen and errors stay until dismissed. Advanced mode: workbench with every panel wired to real tools, as a screen beside the projects (builds and their questions stay a click away). On macOS the menu bar has the app's actions (open, new project, settings, Simple / Advanced, Clean room / Deep, the command palette, help) with the usual shortcuts | UI unit and smoke tests (egui_kittest), offscreen snapshots of every screen |
| CLI | Every tool, `--deep` or `--clean-room` for one run (never saved), `rebuild` (including `--continue-in`, and `--resume` to carry on a stopped build), `mcp --inbox`, `ai`, `setup`, `doctor`, `report` | CLI integration tests |
| SQLite database files (`db_sqlite_show`) | A rowid alias column (`INTEGER PRIMARY KEY`, including the table-constraint form) shows the rowid; `DESC`, non-`INTEGER` and composite keys keep their stored values | Hand-built fixtures; generated files compared with the official SQLite library |
| SQLite WITHOUT ROWID tables | Read from the index b-tree, interior records and overflow included; rows in primary-key order, values in declaration order, `rowid` null | Hand-built fixtures; generated files compared with the official SQLite library |
| Builder discovery smoke fixture | The missing-builder smoke fixture controls discovery state and supplies a synthetic result. | All original warning, modal, action and build-availability assertions are retained. Source inspection only; automated validation has not yet run for this change. |
| Parser review follow-ups | Java reference scans retain partial coverage; DEX signed LEB128 enforces 32-bit values; legacy AAPT strings stay inside their pool with a shared recovery probe limit; missing classic ext indirect metadata is not recovered as zeros. | Shared-budget revision `29c48e03` passed [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41762780268). The merge with main `41d2175e` awaits a new CI run. Extent-tree metadata handling is unchanged. |
| Stand-in retained body budgets | Check per-body limits after redaction and charge the capture budget only for distinct retained samples. Budget-rejected cached 304 responses remain unavailable. | All four original capture regressions and six deduplication regressions passed in [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41788735130) at revision `d601bc4e`. Workspace tests, strict Clippy, formatting, layers, locks, secrets, wasm-plugin checks, shared WASM compilation and the lockfile check passed. |

### Builder retry policy

Builder failures carry a typed reason through `Conversation::last_failure`. Typed authentication,
billing, missing programs, unavailable tools, configuration and unknown protocol failures stop
for attention. Transport interruptions, rate limits and nonzero process exits without a typed
error get at most three retries; the decision no longer searches English error messages.
Structured provider waits win; the earlier numeric “retry in”
hint is only a delay fallback after a failure has already been classified as retryable. All waits
remain cancellable and bounded to fifteen minutes.
Local inbox and workspace write failures are not retried using a concurrent backend failure's
retry advice; the original local error is returned.
A missing saved session starts afresh only when a fresh request is available; otherwise the run
stops without retrying the unavailable session.

Claude Code's failed or unauthenticated `craft-anything` MCP server in its init event stops the
owned CLI process. Pending servers and failures in unrelated optional servers do not stop it.
Claude assistant error codes, HTTP status/provider errors and ACP authentication errors retain
their typed meanings. A later generic result does not erase a more specific error.

API retries continue the existing history without inserting the user's message again or replaying
completed tools. Stateless CLI retries receive the whole original request. OpenCode now uses
`run --format json`: its API error status and retry flag decide retryability while text, tools and
usage still reach the feed. Missing or invalid step costs keep the turn's cost unknown; explicit
zero remains a known cost. A CLI that fails with only a numeric exit also uses bounded retries;
its display error remains available if recovery fails. The process wrapper sends one final turn
event and becomes available for the next request.

Protocol fixtures, a loopback HTTP server, scripted API turns and executable CLI stand-ins cover
these paths, including cancellation and the retry cap. An executable that exits 1 without a
structured error resumes its saved session, while typed setup errors retain priority over the
same exit status. No live provider or paid-model run was performed. The separate source of #64's
41 attempts remains unverified.

### Bun stored-text encoding

Packed Bun members with encoding value 2 are labelled `utf16le`, matching their stored UTF-16LE
bytes. Reading or unpacking preserves those bytes. Synthetic graphs cover UTF-16LE (including a
surrogate pair), Latin1 and binary members across both supported module-record and offsets-block
sizes. A freshly compiled Bun program and the CLI/MCP transport were not run for this correction.

### Native tool-call stop reasons

Native OpenAI-compatible tool calls retain a reported output limit or refusal instead of
replacing it with a tool-use stop. The conversation can therefore reject those calls before
running a tool, including calls whose arguments happen to be valid JSON. A normal `stop`
with tool calls still works for servers that omit `tool_calls` as the finish reason.
Calls retained after a pause or another terminal stop get matching non-execution results,
so subsequent requests keep valid tool-call history. A pause continues the same turn;
terminal stops wait for the next user message.
Checked with streamed-call fixtures and HTTP/SSE stand-ins through the real conversation
and engine loop. Three history-pairing regressions fail without the follow-up and pass with
it. The affected crate's 223 tests and Clippy checks passed locally. The initial stop-reason
correction also passed [hosted CI](https://github.com/storytold/craft-anything/actions/runs/41587580930)
in PR #544. This correction concerns reported finish reasons; handling of a missing finish
reason is unchanged. No live provider or local-model build was run.

### OpenCode review follow-up

OpenCode variants now use the shared five-level `Effort` type while retaining exact catalogue
validation. Hub and headless tool selections share a strict disk-based save that preserves unrelated
settings changes and refuses damaged files or changed backends. Regression tests cover both entry
points. Rejected or cancelled model validation emits a terminal turn event so headless chat exits.

### Unreal setup

Unreal installations registered in `~/.config/Epic/UnrealEngine/Install.ini` are found on Linux.
A valid absolute `UE_ROOT` selects an installation explicitly; an invalid override falls back to
registered installations. The version comes from `Engine/Build/Build.version`, and C++ readiness
checks for the selected engine's bundled Clang, not an unrelated system compiler or another
installation's SDK. The Linux requirements summary (`Builder::needs` for Unreal) now describes
the bundled Clang toolchain. The guided installer now gives Linux-specific guidance: it names the
bundled Clang and points at the engine's `Engine/Build/BatchFiles/Linux/SetupToolchain.sh` (or `Setup.sh`
for a source build) when Clang is missing. On macOS, the status line and requirements summary now
name Xcode, matching the guided step. The guidance strings are unit-tested; no live Unreal
install or macOS run was performed for this copy change.

Checked with isolated registry and SDK fixtures and with a native Fedora install of Unreal
5.8.4. On Windows, Visual Studio C++ readiness also checks instances registered in
`%ProgramData%\Microsoft\VisualStudio\Packages\_Instances\*\state.json` and explicit developer
environment roots (`VSINSTALLDIR`, `VCINSTALLDIR`, `VCToolsInstallDir`), requiring `cl.exe`
under an MSVC toolset's `bin` tree rather than an empty `VC/Tools/MSVC` or version folder; checked with isolated
fixtures and on Windows 11 against Visual Studio Community 2026 (`18.9.2`, MSVC `14.51.36231`).
Readiness is a tool-presence check; a complete Unreal game build and SDK compatibility
validation have not been run for this change.

### Provider error messages

HTTP failures include the provider's message from `error.metadata.raw`, when it contains a
message, detail or string error. Object payloads, JSON-encoded strings and plain-text reasons
are supported, with the provider's name when supplied. A repeated outer message does not hide a
distinct nested detail. JSON-string wrappers are decoded to a bounded depth before reading message
fields; arbitrary, malformed or excessively wrapped structured metadata is not dumped. HTTP status
classification and remediation are retained, and displayed details are bounded by Unicode characters.
Checked with synthetic response bodies, repeated messages, encoded payloads with adjacent headers,
malformed metadata and Unicode length controls. The affected crate's 222 tests and all-target Clippy
checks passed locally for PR #542. No live API-key build or account-specific diagnosis was performed.

### Fixed during the audit

Web tools couldn't launch a browser on Windows/macOS (Chrome/Edge are off PATH) or be pointed at
a URL; the desktop app looked tools up by ids that didn't exist (website, phone, imports,
classes, modules, pseudo-C, disassembly, source and network panels were dead); evidence wasn't
reloaded from a project in a new process; report citations were unresolved; the CLI's
`--format` clashed with tools' own `format`; raw binaries couldn't be opened from the CLI;
function discovery claimed complete coverage when capped; .NET/Go binaries were called
"stripped"; Windows path separators and CRLF checkouts broke two tests; decompiler tail calls
and Win64 stack arguments. Added since: byte search, Rust binaries, Electron fuses, WASM
interface diff, packet captures, objdump, rizin commands, GDB sessions, then rizin debug sessions (`native_rizin_session_*`,
checked with Rizin 0.9.1 on Windows) and radio decoding with rtl_433 (`radio_recording_decode`,
`radio_decoders_list`, checked with rtl_433 25.12 on Windows) and Cutter sessions the person
shares with `=h& 9090` (`native_cutter_sessions_list`, `native_cutter_command_run`; tested
against a stand-in server, not yet a real Cutter). Every tool is listed in `docs/tools.md`.

### SquashFS extended directories

The filesystem reader recovers the same files and bytes from basic and extended directory inodes.
Synthetic images cover an extended root, a nested extended directory with a nonzero listing offset,
and both together; the results are compared with the existing basic-directory fixture. Compressed
metadata, directory indexes and a real firmware extraction were not newly checked for this fix.

### Keep-awake helper cleanup

Rebuilds release their keep-awake guard before emitting `Finished` and clearing the running flag.
The guard waits up to five seconds for the owned helper's supervised cleanup, logging a warning
if that deadline passes. Cleanup runs on the rebuild worker, with a bounded shutdown wait.

Checked on Linux with real subprocess stand-ins: dropping a guard closes the helper's listener,
a stand-in CLI can drop it and exit immediately without leaving the helper running, and an
already-finished helper releases promptly. The immediate-exit case fails with the earlier
asynchronous cleanup. The intermittent Windows report and Windows/macOS sleep inhibition have
not been rechecked for this change; Windows runtime validation remains on the roadmap.

The full workspace gates (`cargo xtask ci`) passed in [hosted CI for #541](https://github.com/storytold/craft-anything/actions/runs/41584228995).

### Tool calls written as JSON

The text fallback accepts complete bare or fenced JSON calls with `name` and `arguments`
(`parameters` also accepted), including a `function` wrapper and JSON-encoded arguments.
Automatic recovery also handles these responses when native tool calling was offered. A call
must name a tool offered in that turn; ordinary JSON answers and JSON examples surrounded by
prose are left as text. Native calls are kept once; text-call recovery skips truncated or refused replies.
Complete backtick-fenced legacy ReAct steps remain supported when their body begins with an explicit
protocol marker. Both parsers use the same fence boundaries: at least three opening backticks and a
standalone closing line of at least that length, with no earlier matching closing fence. Shorter
backtick lines inside a longer fence remain content. Ordinary fenced code, malformed fences and non-call
JSON objects or examples containing protocol markers remain text; valid call arguments may contain those markers.
The parser regressions failed on the previous guards and passed with their corrections.
Fenced JSON calls also use these boundaries during automatic recovery and accept only an empty or `json` info string.
Checked with parser fixtures, HTTP/SSE stand-ins exercising real engine tool execution in both
protocols, and non-action, stop-reason and duplicate-call controls. No live local-model build
was run; other text-call formats remain unsupported.

### Linux graphics card choice

On Linux the desktop app draws on the GPU that drives the screen (#571), read from the connected
displays in `/sys/class/drm`: the built-in panel's GPU on a laptop, the only GPU with monitors on a
desktop, and the discrete GPU when monitors are on more than one GPU or nothing can be read.
`WGPU_ADAPTER_NAME` and `WGPU_POWER_PREF` still decide when set. `desktop.log` says which adapter
was chosen and why.

Checked live on one laptop (AMD Renoir iGPU driving the panel, NVIDIA RTX 3050, CachyOS, KDE
Plasma on Wayland): the window opens in 3 s on the AMD GPU instead of 18–25 s on NVIDIA, and the
desktop no longer lags; `WGPU_POWER_PREF=high`, `WGPU_ADAPTER_NAME=nvidia` and `DRI_PRIME=1` with
`WGPU_POWER_PREF=none` still draw on NVIDIA. Desktops with monitors on one or two GPUs and Intel
laptops are covered by unit tests on fake `/sys/class/drm` folders only.

## Known limits

- Locks: every mutex but the browser viewer's goes through `platform::sync`. A deadlock on one thread or across
  threads is broken when it forms, with an error and a report in `logs/`; a watchdog looks every 5 s; a holder
  that isn't a labelled job is reported after 2 minutes and waited on for 15 at most. Tools are labelled, so they
  may run as long as they need. Checked by unit tests of each case, not by a live deadlock in the desktop app.
  Waits on channels and `join` aren't watched; each child program has its own deadlines.
- A failed agent CLI run says what ran and what it printed, and writes a private log to `logs/` in the
  config folder (the newest 50 are kept; the person's message is left out; **Open log** in the app, `log:`
  in the CLI). Checked live against a GitHub ssh alias from the CLI; the button by a UI test, not by
  clicking it in the desktop app. API backends and MCP start-up don't write these logs yet.
- Live end-to-end runs (2026-10-18, Windows 10, headless CLI): a website clone reached Ready
  at 96% likeness in 1.5 min; a small desktop app rebuilt in Rust + egui reached Ready in 9 min (checked
  only by building); a game rebuild in Rust reached Build and was stopped before Check. The
  website case is an opt-in test: `CRAFT_E2E=1 cargo test -p craft-anything-cli --test live_rebuild`.
- Approvals differ by agent (roadmap R2.2): Claude Code asks our approval tool; OpenCode's builder
  runs with `--auto`; Hermes's `-z` turns its approvals off on every run, chats included, so a Hermes
  chat isn't limited to analysing and Clean room's approval rule doesn't reach it. Hermes is opt-in
  and hasn't been run live here.
- Maki (`maki -p`, or over ACP with `acp = true`) is offered when adding a coding agent and found by
  Detect. Over ACP it asks us before commands, our own tools and writes outside its working folder;
  writes inside that folder (a build's project folder, our own records included) and every read
  never ask, so the approval policy and Clean room's read rule don't reach those. Plain `maki -p`
  doesn't ask at all, so a Clean room build's reads aren't checked either; outside a build it runs
  with its writing tools, shell and code execution switched off, and Maki refuses anything else
  that would ask (an MCP server's tools, in subagents too) unless the person allowed it in Maki's
  own permissions. It hasn't been run as a `maki -p` builder here.
- Claude Code is fully wired as a builder; Codex works through its own flags (our tools are pre-approved for
  `codex exec` and every tool permitted by the current mode is listed from startup, see `docs/codex.md`)
  but doesn't resume. Codex 0.166.0 received the full Clean room inventory through a loopback model;
  no real-account build has been run;
  Copilot builds, model discovery and real-account session
  resume were confirmed by the person using the app. Other agent programs
  without an adapter get plain text in and out. API-key
  backends (Anthropic, OpenRouter, OpenAI, Gemini through a Google AI Studio key, local; Kimi K3.1 Pro through OpenRouter is offered by Detect; a backend chosen as the Extractor takes a Deep-mode game's own files out into `app/Original/` after the study) build with our own loop and the Build tools,
  trim and summarise long conversations and keep them in `transcript.json`; no live API build has
  been run yet.
- Website checks compare the start page and up to 12 linked pages at the configured widths; the
  start page's menus, pop-ups and tabs are opened on the build too and compared in the note (the
  first few only, never menus inside menus or hover-only ones). Apps and games are checked by
  building and by a screenshot compared with the person's pictures (and the original's window
  when runtime study is on; then the build is also started normally, and its window and
  labelled controls are compared with the original's); games (and apps decompiled in Deep mode)
  also keep `app/mechanics.json`, which is checked for being readable and sourced, and fails
  while a number found in the original isn't used in the code (estimates may stay out). Every
  game must also pass its own `--self-test` (the build plays its core loop with scripted input
  and reports whether the player moved, acted and the HUD was drawn), and a Deep-mode game that
  uses the original's files must have `app/CREDITS.md` and the private-use line in
  `app/README.md`. The self-test is the build's own report: Craft Anything doesn't yet watch the
  game play or measure the values itself.
- In Deep mode, app and game targets get a static study (triage, engine detection, file
  inventory, JS app graph, Ghidra when installed); in Clean room only the person's pictures and
  words. With runtime study on, the original is also started once for a
  picture of its window and its UI tree (Windows and macOS); it isn't driven or watched over time.
- Several projects build at once (`[build] parallel`, 2 by default; Settings → Builds); more wait
  their turn. Queued builds start on their own as places come free, from MCP too.
- Some flows exist only in the desktop app: the project feed, game-engine and Ghidra installs,
  and launching results (ROADMAP R1.6).
- Engine gaps from the audit: Frida session tools are partly simulated; imports outside the
  argument-count table show only the arguments set in the calling block. See tool-status.md.

## Next

See [ROADMAP.md](ROADMAP.md) for the ordered plan.

## Build progress across runs

When app files can be compared, a build stops after two consecutive completed builder turns leave them unchanged.
The versioned `inbox/build-progress.json` record keeps that count when the project is reopened or
Try again is pressed. File contents and executable permissions decide progress, so rewriting
identical bytes does not reset it. An actual app edit or a new nonempty request starts another
attempt, including notes queued during a turn or check and notes consumed by a builder hook.
Empty notes and plain Try again keep the saved count. Dependency/build caches,
git metadata and Craft's own records, including checks in a project edited in place, do not count.
Symlinks are recorded without following their targets. A successful check may still finish the
second unchanged turn, and a run waiting only for checks does not start the builder again.
Within a run, file digests are reused while size, modification time and executable bits match;
Unix file identity and change time also invalidate a cached read. The first scan reads content,
but unchanged imported assets are not read again before and after every turn. The metadata is
only a cache key: rewriting identical bytes still produces the same content fingerprint.
The cache is isolated by project root, drops deleted or retyped paths, and is discarded after
scan errors or cancellation. It is not persisted, and the version-1 fingerprint/schema is unchanged.
Sockets, pipes and devices contribute their names and types without opening their contents.
If a scan cannot finish because of limits, permissions or changing files, the feed says why the
unchanged-turn limit is skipped for that run. The saved fingerprint and count are cleared so a
later readable run starts a fresh comparison. Checks and the existing round limit still apply.
Cancellation still stops the run; completed builder replies are saved as awaiting checks before
the post-turn scan. Broken or newer progress records and record-write failures remain errors.

Scripted API turns and executable CLI stand-ins cover the stop, persistence, fresh requests and
check recovery. This change has not been verified with a paid model or a live provider. Copying a
whole project keeps its progress record; Duplicate creates a new attempt with a fresh conversation
and count, as it already does for usage.
An existing root-level file or directory named `build-progress.json` remains app data: it is
fingerprinted, copied by Duplicate and kept when Craft's project records are removed. Its JSON
shape does not make it a Craft record, and it is never migrated into the inbox.

Notes consumed by builder hooks now only queue a progress reset while the turn is running.
The reset is saved after the successful turn and its `awaiting_check` recovery state are
persisted. Multiple consumed notes coalesce into one reset; blank notes do not reset the count.
A broken or inaccessible progress record still returns its actual error at that safe boundary,
without starting another provider request or losing the finished builder reply.
If the turn fails or is cancelled after consuming a note, its reset is saved before the run ends
so reopening keeps the new request's full allowance. The original failure or cancellation remains
the result; a reset that cannot be saved is reported separately and never overwrites a broken
or newer record. Scripted turns cover both outcomes, blank notes and unreadable records.

## Local model identifier task hints

Automatic local setup recognizes numeric suffixes on known embedding, reranking and speech task
tokens. Generic GGUF filenames (`ggml-model-*.gguf`, `ggml-model.gguf` and `model.gguf`) also keep
their immediate model directory as a task hint, with either path separator. Named model files,
publisher prefixes and unrelated words are not classified by arbitrary ancestor directories.
Regression tests cover mixed model lists and servers listing only specialised models through
the local discovery API. Classification remains a name heuristic; it does not probe model capabilities.
