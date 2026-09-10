# Runner execution verification

This record distinguishes a connected transport from a complete, accepted lesson.
Tests use disposable QA databases and normal provider authentication. No production
learner database is used. The OpenRouter credential remains in native Keychain;
neither these records nor the developer probes contain a key.

## September 10, 2026 checkpoint

| Runner/model | Live connection | Full lesson |
|---|---|---|
| Claude Code: opus, sonnet, haiku | All three pass through the current production adapter, about 3–4 seconds. A packaged desktop Sonnet check also passed. | Sonnet completed writing, depth correction, editorial review and saved a Linux Bash lesson through the native backend. Latest GUI handoff still needs inspection. |
| Codex: default | Passes through the isolated adapter in about 18 seconds; an earlier packaged desktop check passed in about 38 seconds. | Both long writing calls completed. The final editor shortened one section below its required floor (374 versus 400 words); targeted correction remains necessary. |
| OpenRouter: openrouter/free | Passed; the router selected `nex-agi/nex-n2.5-mini:free`. | An earlier long request returned no usable answer. A complete accepted free-router lesson is not verified. |
| OpenRouter: qwen/qwen3-14b | Connection and small JSON generation pass. | Real long writing/expansion/editor calls execute. Tests exposed output-budget, editorial-note and typed-JSON-repair problems. The latest retry completed its requests but failed after two corrections with an incomplete exit question; targeted assessment repair remains necessary. |
| Ollama: installed qwen2.5:7b | Live responses pass, including the earlier native local-library check. | The server allocated a 32,768-token context for a real long request; the initial lesson and repair returned unusable JSON. Long-form output handling remains necessary. |
| Cursor Agent: default | Installed, but the live check requires `cursor-agent login`. | Not runnable with the current account state. |
| Gemini CLI: default | Installed with cached authentication, but its account requires `GOOGLE_CLOUD_PROJECT`. | Not runnable without the user's project configuration. |
| Custom CLI | Argument, model substitution and output transport pass with an explicit synthetic fixture. | A fixture is not evidence of a connected external model. |
| Anthropic API, OpenAI API | Protocol fixtures pass. No live keys available. | Live tests explicitly deferred to the user. |
| Google, Groq, Mistral, DeepSeek APIs | Protocol fixtures pass. No live keys available. | Live availability remains unverified. |

## Changes behind these checks

Claude study calls use safe mode, medium effort and an explicit tool set. On
macOS a fixed shell `exec` launch preserves separate arguments and stdin; this
resolved the reproduced native connection startup failure. Codex study calls
retain CLI authentication but ignore personal coding configuration, use medium
effort, and disable shell work, delegation, apps and hooks. Explicit model IDs are
still passed through; `default` uses the CLI's default for this isolated session.
Custom CLI remains available for a deliberately configured external workflow.
Older CLIs that lack the isolation flags receive an update instruction.

The application retrieves teaching sources before writing. Writing turns now use
that material directly instead of starting another CLI research workflow or
executing learner exercises. Prompts follow the current subject and starting
point and no longer invent once-a-day teaching history or require frontend UI
artifacts from terminal courses. Citation requirements account for the sources
actually retrieved.

Compatible API responses accept public text blocks and distinguish truncation,
refusal and empty output. Unusable responses retain actual model, token and cost
metadata; hidden reasoning is never substituted for the answer. OpenRouter
reasoning controls follow the selected model's advertised capabilities. The
short connection check reserves a larger output allowance for reasoning models.

Editorial notes accept structured metadata without relaxing scores or lesson
validation. JSON repair receives the original request and the typed validation
error; repairing syntax alone could not fix a malformed question structure.
Repair no longer silently drops everything after 60,000 response characters.
Oversized responses fail explicitly rather than being truncated for repair.

The desktop shows preparation status across navigation and rejects duplicate
Start requests in both frontend and native code. Failure releases the guard.
Progress adapts to the panel space left by this status area. That layout was
verified in a packaged desktop app during a real preparation request.

## Reproducible checks

`runner_probe` defaults to inventory only. `--live` exercises saved models through
the production adapter; `--lesson-smoke` requests a small structured answer.
`lesson_probe DISPOSABLE_QA_DATABASE SUBJECT_ID --live` runs the real engineering
preparation and persistence pipeline without a webview or OS enforcement. It
rejects an already-active lesson so resume cannot masquerade as a new generation.
Neither tool is a substitute for native GUI verification or a learner completing
the resulting exercise and checks.

The current suite passes 232 Rust tests, with six live/external tests ignored,
plus 52 frontend tests. Svelte reports zero errors/warnings and strict Clippy
passes. The native automation surface later returned `cgWindowNotFound` for all
QA windows; GUI inspection of the newest long-lesson results remains pending.
The shared-runtime migration, class occurrence/focus ownership and P4–P9 gates
remain unfinished in [the product plan](PRODUCT_EVOLUTION_PLAN.md).
