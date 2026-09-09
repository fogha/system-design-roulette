<p align="center">
  <img src="docs/logo-mark.svg" width="140" alt="System Design Roulette — the elected shard" />
</p>

# System Design Roulette

**A macOS app that hijacks your laptop once a day and forces you to get better at system design.**

AI is absorbing more of the routine coding work every month. The skill that compounds is judgment — architecture, trade-offs, system design. This app makes sure you practice it daily, whether you feel like it or not.

At a time you choose, your screen is taken over — full-screen, above the menu bar, immune to Cmd+Tab, Cmd+Q, and Force Quit — until you finish a ~38-minute session: a quiz on yesterday's topic, a roulette spin that picks today's, and a 30-minute course generated on the spot by your own AI agent, taught from primary documentation the app fetches and verifies live.

![Lock-in screen](docs/screenshots/01-lock-in.png)

## The daily loop

### 1 · Quiz on yesterday's course

Every session opens with a quiz on what you read yesterday — multiple choice plus free-text. Free-text answers are graded by your agent against a rubric. Questions you failed earlier come back, badged, until you pass them.

![Quiz](docs/screenshots/02-quiz-mcq.png)

### 2 · Review your answers

Every question shows your answer, the model answer, grader feedback, and an explanation that teaches. Failed questions are flagged **returns tomorrow** and added to the next quiz — spaced repetition of exactly the things you got wrong.

![Review — correct answer](docs/screenshots/03-review-correct.png)

![Review — failed answer returns tomorrow](docs/screenshots/04-review-failed.png)

### 3 · Spin the wheel

A roulette wheel picks today's topic from a pool of 72 system design concepts across 8 categories — fundamentals, storage, caching, messaging, resilience, architecture, security, operations, plus classic case studies. No topic repeats until the whole pool is exhausted.

![Roulette landed](docs/screenshots/06-roulette-landed.png)

*(The spin is theater — the topic is drawn server-side when tomorrow's content pre-generates, so the course is ready the moment the wheel stops.)*

### 4 · Read the course — 30 minutes, enforced

A 3,500–4,500-word course written from primary documentation the app fetches and verifies for each lesson (see [Sourcing](#sourcing-what-the-courses-are-taught-from)). The reader switches to a calm paper theme for eye comfort. The **Complete session** button stays disabled until the 30-minute timer hits zero. Links are intercepted during the session and open in your browser afterward.

![Course reader](docs/screenshots/07-course-reader.png)

### 5 · Done — and tomorrow is already loading

Score, streak, and a category tease for tomorrow. The moment you finish, tomorrow's course and quiz start generating in the background.

![Completion](docs/screenshots/08-completion.png)

### Track everything

GitHub-style heatmap, per-day scores, carryover queue, and every past course readable in the archive.

![Dashboard](docs/screenshots/09-dashboard.png)

## Classroom: independent subjects, teachers, and time slots

The idle screen contains a **Classroom** with six built-in classes: German,
Italian, JavaScript/browser internals, TypeScript, frontend architecture, and
developer tooling. Each class owns its enabled state, weekday/time slots,
same-day sessions, progress view, provider, model, and versioned prompt
contract. You can use DeepSeek for frontend architecture, Claude for German,
and another provider for TypeScript without mutating a global teacher or
overwriting another class.

Classroom slots are advisory: they can surface the app, but never engage the
kiosk or mark the primary frontend session complete. Multiple engineering and
language classes can run on the same date because they use ID-keyed classroom
sessions rather than the legacy one-row-per-day primary session. When the
enforced primary session is owed, it wins: classroom start/resume actions remain
disabled until the primary session is completed or skipped.

Every built-in subject has a separate prompt under
`src-tauri/prompts/classroom/`. Engineering classes use the existing
mastery/roulette curriculum but generate through their own immutable routing
profile and strict course validator. Language teachers may enrich a curated
lesson, but cannot change its CEFR objective or assessment answers; shallow or
invalid generation falls back to the bundled lesson.

Primary and classroom engineering readers include a session tutor grounded in
the current course, curriculum outcome, cumulative artifact, and exercise. It
renders code-aware Markdown, links each answer to the exact section to reread,
and proposes three useful follow-up questions. Chat uses the configured
provider and model with one same-provider validation correction—never an
unrelated answer from a fallback provider.

Engineering lessons target 3,500–4,500 words and cannot pass the deterministic
gate below 3,200. Every major section also has its own depth floor, so a model
cannot satisfy the quantity requirement by padding one overview or repeating
summaries; the material must include worked mechanisms, runnable evidence,
production transfer, failure analysis, migration/observability, and deliberate
practice.

### German and Italian

Each language ships with 40 action-oriented scenario units across A1–B2 (80
total), covering listening, reading, spoken interaction, spoken production,
writing, grammar, and vocabulary/pragmatics. A lesson includes a model
dialogue with system-voice playback, contextual grammar and vocabulary,
pronunciation and culture notes, writing/speaking evidence, five retrieval
checks, and immediate misconception feedback. Scenarios are revisited through
different phases and spaced unit progress.

The default roadmap uses stretch dates of **A1 at one month, A2 at three
months, B1 at nine months, and B2 at eighteen months**. These are planning
targets, not automatic promotions or exam guarantees. The app shows a pace
warning when the configured weekly commitment is below lower-bound guided
practice ranges, and advances only after all seven skill strands have evidence.

## The hijack

This is the point of the app. During a session:

- The window is **full-screen at screensaver window level** (above the Dock and menu bar) on every Space.
- macOS kiosk presentation options disable **Cmd+Tab, Cmd+H, Force Quit, and Quit/Log Out/Shut Down**.
- A 300 ms **focus re-grab loop** yanks focus back from Spotlight, Mission Control, notification clicks — anything that briefly steals it.
- Extra displays are covered with **black blanker windows**.
- Closing the window and quitting the app are blocked at the Tauri event level.

### The only ways out

1. **Finish the session.** ~38 minutes.
2. **The escape phrase.** A dim "emergency exit" link reveals a long phrase rendered as non-copyable SVG (paste disabled). Type it exactly and today is marked **skipped** — and your streak resets. Three wrong attempts locks the input for 60 seconds.
3. **The dev back door.** `touch ~/sdr-unlock` releases the lock within a second. Delete this code path if you want no mercy; keep it if you value your laptop during development.

Force-shutdown doesn't help: interrupted sessions resume at the exact step (even mid-quiz, answers persisted per question) on next launch.

### Recovery (if a broken build ever locks you out)

The kiosk refuses to engage until the webview reports ready (white-screen guard), so a dead frontend can't hold a lock. If you're ever stuck anyway:

1. **From another machine** (SSH/Screen Sharing):
   ```bash
   touch ~/sdr-unlock
   launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.darkmatter.system-design-roulette.plist
   rm -f ~/Library/LaunchAgents/com.darkmatter.system-design-roulette.plist
   ```
2. **Safe Mode** (Apple Silicon: hold power → pick disk → hold Shift): third-party LaunchAgents don't load. Run the same commands in Terminal, reboot.
3. **Recovery Mode Terminal**: `rm "/Volumes/Macintosh HD/Users/<you>/Library/LaunchAgents/com.darkmatter.system-design-roulette.plist"` and `touch "/Volumes/Macintosh HD/Users/<you>/sdr-unlock"`, then reboot.

While `~/sdr-unlock` exists the app can never hold a lock — delete it to re-arm.

## Content generation — your provider

Settings separates **Runners & models** configuration from the **Active tutor** box below it. The library offers three runner routes adapted from Remote Ledger:

- **An agent I already have:** Claude Code, Codex, Cursor Agent, Gemini CLI or a custom command. Each CLI keeps its existing authentication.
- **My own API key:** Anthropic, OpenAI, Google Gemini, OpenRouter, Groq, Mistral or DeepSeek. Provider keys are shared by the desk and its classes; on macOS, save them in Keychain from Settings. Environment keys take priority.
- **On this machine:** Ollama installation/detection/start, installed models, a model shelf with approximate download/RAM sizes, background downloads, testing and removal. Downloaded models answer locally; the teaching backend still fetches source documentation from the web.

Configure providers and save a model shortlist for each in the library. Catalogue
search and five-result pages keep long lists manageable. Then select a runner and
saved model in **Active tutor**, test its response, and choose **Use for study**.
Saving library configuration does not switch the active tutor. API catalogues load
from the provider; CLI aliases and manual model IDs are also supported. OpenRouter defaults to free
models only, with live prices and capabilities in its catalogue. Class settings
reuse these controls while retaining their own tutor choice.

Course generation uses exactly the configured provider and model. The result
must pass the full course, five-check, exercise, source, and first-principles
quality gate. Malformed JSON or a missed gate gets one correction pass through
that same provider and model, followed by a same-provider senior editor that
scores mechanism depth, specificity, production transfer, dossier use,
exercise alignment, and source discipline. Otherwise the real generation error
is shown. There is no alternate-provider or bundled-course substitution.

- **Course**: the app retrieves the source material itself before any provider call, so grounding no longer depends on the provider having search tools (DeepSeek has no provider-side search in these calls). See [Sourcing](#sourcing-what-the-courses-are-taught-from).
- **Quiz**: generated from the stored course text, no tools.
- **Quiz and grading resilience**: auxiliary question selection can use bundled question material. Free-text grading can remain unassessed when the provider is unavailable. Grading, planning, narration and language enrichment switch providers only when an explicit fallback is configured; this never replaces a researched course.
- **Pre-generation**: tomorrow's content generates the moment today's session completes (and retries hourly via a job queue), so the roulette reveal is instant.

The optional fallback has its own runner and model and starts disabled. See [the runner implementation record](docs/AGENT_BACKEND_PORT.md) for routing, validation and platform limits.

## Sourcing: what the courses are taught from

Courses are taught from documentation the app fetches itself, not from what a
model remembers. Before each generation, Rust retrieves up to five primary
documents — the concept's curated sources plus topic-specific pages discovered
through MDN's search API — extracts the readable prose, and hands it to the
teacher as quoted material it must teach from and cite inline.

- **Allowlist.** Only recognized publishers are fetched: MDN, web.dev, the
  WHATWG and W3C specifications, TC39, Chrome/V8/WebKit developer docs, Node,
  TypeScript, the major framework docs, and RFCs. A redirect that leaves the
  allowlist is dropped, and documentation landing pages lose to pages about the
  actual mechanism.
- **Relevance.** Keyword search returns near-misses (an HTTP status called "Loop
  Detected" for a lesson on the event loop), so a discovered document must
  overlap the topic on more than one term and live in the web-platform reference
  tree — tutorial and browser-extension docs are filtered out.
- **Citation gate.** A course must link at least three of the retrieved
  documents inline, on the claims they support. Failing that costs one
  same-provider correction pass, then the course is rejected.
- **Link verification.** Every URL in the reading list is checked over the
  network, and unreachable ones are deleted rather than shown — so an invented
  link shrinks the reading list instead of misleading you. The retrieved sources
  are always added, each labeled with its publisher.
- **Offline.** If retrieval fails entirely, the lesson still runs but carries no
  unverified links, and the generation log says why.

Because DeepSeek's JSON mode returns roughly a third of the prose it otherwise
writes, the course body is requested as plain markdown, and any section below
its depth floor is deepened by a further prose-only pass scoped to that one
section.

## Scheduling

A launchd LaunchAgent (`~/Library/LaunchAgents/com.darkmatter.system-design-roulette.plist`) fires the app at every enabled primary or classroom time. The plist uses one `StartCalendarInterval` array, so adding a class does not replace the original frontend time:

- Scheduled time missed while **asleep** → fires on wake.
- Missed while **powered off** → fires at next login (`RunAtLoad`) — the app checks "is a session owed?" on every launch and every 60 s.
- Already running → an in-app watcher engages the lock at the scheduled minute.
- Changing the primary time or adding/removing any class slot rewrites and reloads the full interval set.
- Primary due checks retain the configured kiosk level. Classroom due checks emit advisory reminders only. The idle countdown shows the earliest enabled primary/classroom event, and an owed primary session is explicitly shown as **due now**.

## Install

### Prerequisites

- macOS 13+
- A supported CLI, a provider API key, or Ollama with a downloaded chat model.
- To build: Rust 1.80+, Node 20+.

For DeepSeek development:

```bash
DEEPSEEK_API_KEY=... npm run tauri dev
```

For an installed macOS app, choose **My own API key** in Settings and save a key
for the provider you want. Keys use the existing `system-design-roulette` Keychain
service with separate `<provider>_api_key` accounts. On other platforms, supply
`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY` (or `GEMINI_API_KEY`),
`OPENROUTER_API_KEY`, `GROQ_API_KEY`, `MISTRAL_API_KEY` or `DEEPSEEK_API_KEY` to
the process. In-app key storage on Windows/Linux remains a release follow-up.

### Build from source

```bash
git clone https://github.com/dark-matter08/system-design-roulette.git
cd system-design-roulette
npm install
npm run tauri build -- --bundles app
cp -R "src-tauri/target/release/bundle/macos/System Design Roulette.app" /Applications/
```

Launch it, complete the setup wizard (session time, escape phrase, agent check), and it's armed. Day-1's course generates immediately so your first session starts instantly.

![Setup wizard](docs/screenshots/10-setup.png)

> **Heads-up:** the app is ad-hoc signed. First launch may require right-click → Open, or `xattr -dr com.apple.quarantine "/Applications/System Design Roulette.app"`.

## Architecture

```
┌──────────────────── Svelte 5 webview (dumb renderer) ─────────────────────┐
│  SetupWizard · Idle · Quiz · AnswerReview · Roulette · CourseReader ·     │
│  LanguageLesson · Completion · Dashboard                                  │
└────────────────────────────────────┬──────────────────────────────────────┘
                          invoke / events (typed IPC)
┌────────────────────────────────────┴──────────────────────────────────────┐
│  Rust core (all authority lives here — the webview can't bypass it)       │
│                                                                            │
│  session.rs    authoritative FSM: quiz → review → roulette → course → done│
│  kiosk.rs      NSWindow level 1000, presentation options, refocus loop,   │
│                multi-display blankers                                      │
│  generator.rs  strict course generation; resilient auxiliary assessment  │
│  scheduler.rs  launchd plist install, owed-session logic                   │
│  classroom.rs  subject registry, slots, agent routing, advisory sessions │
│  language.rs   independent CEFR curricula, evidence, and level gates      │
│  db.rs         SQLite: sessions, courses, questions, attempts, carryover, │
│                classroom/language sessions, mastery, generation job queue  │
│  roulette.rs   weighted draw, no repeats until pool exhausts               │
│  timer.rs      authoritative 30-min reading timer (persisted, resumable)  │
└────────────────────────────────────────────────────────────────────────────┘
```

Design principle: **Rust owns all authority.** The timer, the lock, the state machine, and grading all live in the backend; the webview renders and requests transitions, which Rust validates. Reloading or inspecting the webview gains you nothing.

Data lives in `~/Library/Application Support/com.darkmatter.system-design-roulette/` — a SQLite database plus every course mirrored as a markdown file under `courses/` for grepping.

## Development

```bash
npm run tauri dev          # full app against a dev server
npm run dev                # frontend only, in a browser — demo mode with mock data
npm run check              # svelte-check
cd src-tauri && cargo test # core-loop integration tests (carryover, roulette, streaks)
```

Useful flags and env vars:

| Flag / env | Effect |
|---|---|
| `--debug-day` | 30-second course timer, no kiosk lock, schedule ignored — a full day in ~2 minutes |
| `--triggered` | What launchd passes; goes straight to the owed-session check |
| `SDR_DATE=2026-06-12` | Override "today" — simulate multi-day carryover flows |
| `SDR_CLAUDE_BIN=/path` | Override the Claude binary |
| `SDR_CODEX_BIN=none` | Disable the Codex runner |
| `SDR_MODEL=sonnet` | Override the exact course-generation model (default: `opus`) |
| `DEEPSEEK_API_KEY=...` | Authenticate DeepSeek for the whole process; the in-app alternative stores one shared key in macOS Keychain |
| `OLLAMA_URL=http://127.0.0.1:11434` | Optional loopback Ollama endpoint; remote/cloud endpoints are rejected for the local route |
| `DEEPSEEK_MODEL=deepseek-v4-pro` | Override DeepSeek's default `deepseek-v4-flash` model |
| `SDR_SESSION_TYPE=pop_quiz` | Force tomorrow's planned session type (skips the planner call) |
| `touch ~/sdr-unlock` | Instantly release the kiosk lock |

Demo mode: opening the frontend in a plain browser (no Tauri) automatically serves canned data from `src/lib/mock.ts` — every screen is reachable (`?step=done`, `?setup`, `?step=quiz&type=pop_quiz`, `?class=german&classDue`, `?class=frontend-architecture`) without the Rust backend. That's how the screenshots in this README were taken.

## The Teacher

The generation layer is a persistent teaching agent — see [docs/TEACHER.md](docs/TEACHER.md) for the full spec. A per-concept **mastery ledger** (unseen → introduced → practicing → mastered → maintenance, with struggling/decayed detours) is compiled into a one-page **dossier** from both primary and classroom evidence, so each course builds on prior work and attacks recorded misconceptions. Every selectable concept has an authoritative curriculum brief: phase, outcome, irreducible mechanisms, production scenario, misconceptions, evidence, cumulative artifact, primary sources, and cross-track relationships. The wheel spans **192 selectable concepts** across JavaScript/browser internals, TypeScript, frontend architecture, and developer tooling and follows a visible foundations → mechanisms → production → synthesis path before electives. Each track has at least a 30-session core, a concrete 30-day proof of skill, milestone exercises on sessions 7/14/21/30, and a curriculum-map view. Finish reading early? The **adaptive exit check** starts with 5 fresh MCQs and unlocks immediately on a perfect round. Every miss is explained and adds one fresh, course-grounded question to the next round. **Audio mode** turns the course into a two-host dialogue. During the lock, secondary displays run the **chaos lab**, all media players are paused, and system audio is muted.

## Roadmap

- [ ] **Hard mode** — CGEventTap (Accessibility permission) to truly swallow Cmd+Tab/Spotlight instead of out-racing them
- [ ] Custom topic pools / import your own syllabus
- [ ] Difficulty progression per category based on quiz history
- [ ] Notarized builds

## License

[MIT](LICENSE)
