# Native assessment verification — 2026-09-09

The packaged macOS app was tested through its native window at `tauri://localhost`,
using Rust IPC and SQLite. The isolated identifier was
`com.darkmatter.principia-desk.assessment-qa`; production data was not opened or
modified. A SQLite backup of the prior design QA profile supplied a v2 upgrade
fixture. Fixed local questions and future `SDR_DATE` values kept the test separate
from existing study records. `--debug-day` disabled kiosk and scheduler changes.
Provider executables were unavailable stubs, with no live grading calls.

## Verified journeys

- Opening the v2 fixture applied migration v3, retained its original quiz config
  bytes and created a pre-upgrade backup. SQLite integrity returned `ok`, with no
  foreign-key violations after the UI journeys.
- A legacy round with two confirmed answers reopened in the original node-panel
  quiz layout with Review answers and Grade saved answers. Grading showed the
  first review item; a full app restart restored that result and the second item
  remained accessible.
- A new three-question round combined due carryover and fresh questions. Custom
  answer buttons saved choices, and confirmation advanced to the next question.
- Free text containing punctuation and `Grüße` saved without confirmation. SQLite
  retained it as a draft at revision 5 with no submission. After terminating and
  relaunching the native executable, the app restored question 3 and the exact
  draft. The already-confirmed answers remained saved.
- Confirming that restored draft created one submission at revision 6. Grader
  failure showed self-assessment mode and stored the free-text verdict as `null`;
  the two deterministic choice answers retained their actual verdicts. No new
  round, draft or result was written through the retired config keys.

The new saved-round state was visually inspected at 1100 × 760. It preserves the
blueprint background, centered node panel, status badge and amber action styling.
Custom dropdown design and minimum-window checks are recorded separately in
[desktop design verification](DESKTOP_DESIGN_QA.md).
This build also rechecked the German start-level menu: the opaque app popup opens
above its trigger, shows the saved A2 checkmark, and Arrow Down followed by Escape
closes the menu with A2 still selected.

## Automated checks and limits

170 Rust tests and 40 frontend tests pass. Six live/external Rust tests are
explicitly ignored. Svelte reports zero errors/warnings; strict local Clippy and
the production frontend/packaged app build pass. Tests include stale writes,
changed answers during grading, owner isolation, immutable results, injected
transaction failures, legacy import and local recovery conflicts.

This verifies the primary assessment adapter on macOS. Shared session identities,
diagnostic placement UI, acceptance into an active class, classroom/language
runtime migration, Windows/Linux operation, live providers and enforcement remain
separate implementation and release gates.


## Diagnostic and suggested-path increment

The same isolated macOS profile was rebuilt and tested with the native placement
commands. Linux Bash restored a selected but unconfirmed answer after terminating
and relaunching the app. Keyboard selection, confirmation, skips, previous-answer
navigation, initial submission and both optional follow-up questions worked. The
result distinguished demonstrated samples, practice needs and unassessed criteria.
A corrected follow-up retained the original failed response in expandable evidence.

The resulting path preview proposed Practical application with 3 of 6 sampled
criteria demonstrated, five prerequisite refreshers, eleven earlier topics still
available and the required course outcome intact. It explicitly identifies
unsampled/practical/retention knowledge and pending activation. Both the check and
recommendation were visually inspected at 1100 × 760 with the original blueprint,
node panels and amber controls. SQLite integrity returned `ok`, with no foreign-key
violations. No class or accepted path was created.

Current checks: 176 Rust tests and 43 frontend tests pass, six external/live Rust
tests are ignored, Svelte has zero errors/warnings, strict Clippy and the packaged
macOS build pass. All nine authored banks and all three recommendation routes are
covered by domain/preview tests. This does not establish calibrated placement,
active-class path acceptance, practical assessment or Linux/Windows operation.

## Accepted paths and revised class tutors

The isolated profile was upgraded from v4 to v5 using the packaged macOS app.
The pre-upgrade backup was retained; SQLite integrity returned `ok` and the
foreign-key check returned no violations. Production data was not opened.

- Linux Bash restored its completed diagnostic and proposed Practical application
  with 3 of 6 sampled criteria and five prerequisite refreshers. Accept path enabled
  the class and displayed Path 1 while keeping coverage at 0 of 18 concepts.
- Revise starting point opened a fresh draft seeded from the accepted choices.
  The native custom dropdown selected Core mechanisms; the shared runner selector
  selected OpenRouter and its saved `openrouter/free` model without entering a key
  or making a provider request. The two-model shortlist remained accessible.
- Accepting the revision kept the same class and displayed Path 2 with the new
  tutor. The first diagnostic/path remained stored. Acceptance created no lessons,
  mastery or completion credit: the QA profile still had zero classroom/language
  sessions and its original single mastery row from the earlier assessment fixture.
- Settings and enrollment were visually checked at 1100 × 760. Primary, secondary,
  library and dropdown controls share subtle rounded corners with the panels.

The first rebuilt launch showed a blank native window before rendering.
The external Google Fonts stylesheet has since been replaced with bundled fonts;
this removes an external resource from the startup path. The final packaged build
rendered on the first observation after a fresh launch and restored Linux Bash
Path 2 with its OpenRouter model. This does not establish timing guarantees for
every machine or eliminate every possible startup delay.

Automated checks cover all nine course acceptances, all seven engineering entry
selectors, stale recommendations, rollback on injected write failure, duplicate
acceptance, immutable prior revisions, settings rollback and language reassessment
during an active lesson. 197 Rust and 51 frontend tests pass, six external Rust
tests remain ignored, Svelte has zero errors/warnings and strict Clippy passes.
This is still a bridge to existing session adapters, not the completed shared
learning runtime or a cross-platform release qualification.

## Shared runtime schema upgrade

Built the packaged debug app with identifier
`com.darkmatter.principia-desk.assessment-qa` and upgraded that isolated profile
from schema v5 to v6. Production data was not opened. The v5 backup was readable,
SQLite integrity returned `ok`, and foreign-key checking reported no violations.

Before/after ordered-row hashes matched for all 14 checked tables: primary,
classroom and language sessions; courses; all four assessment tables; enrollment
drafts; classes; path revisions; mastery; exercise drafts; and course snapshots.
The profile retained four primary sessions, three documents, four assessment
attempts, five rounds/submissions, four enrollment drafts, one class, two accepted
path revisions and one pre-existing mastery row. New runtime session count was
zero, as expected before engine cutover.

The direct background-binary launch initially displayed a blank native window.
Live process samples showed idle event loops in the app and WebContent; the
database upgrade was already complete. The UI appeared by the time the native
Web Inspector opened, and its console showed no error messages. This observation
does not prove what caused the delay. After quitting that process, a normal macOS
Launch Services launch rendered at the first observation and restored the Classes
view, Linux Bash Path 2/Core mechanisms, and `openrouter/free`. Learning-table
hashes still matched after relaunch. No provider calls or new study credit were
needed for this check.

Validation logs: `/tmp/principia-study-all-tests.log` (208 passed, six external
tests ignored), `/tmp/principia-study-clippy.log`, and
`/tmp/principia-study-native-build.log`. Isolated before/after fingerprints are in
`/tmp/principia-study-native-before.json` and
`/tmp/principia-study-native-after.json`. The new runtime is covered through its
native domain and storage tests; existing UI commands still use the legacy
engines, so this check verifies upgrade compatibility, not completed engine
integration or cross-platform runtime behavior.

## Primary identity and reader restart

Used a separate profile, `com.darkmatter.principia-desk.primary-id-qa`, copied with
SQLite's backup API from the earlier isolated v6 assessment profile. Added only
synthetic QA rows: a prepared September 19 JavaScript reader and a separate
September 20 TypeScript pending row. Launch Services started the packaged app
with `SDR_DATE=2026-09-20`, disabled Claude/Codex executables and `--debug-day`.
This is a controlled service-date test, not a timezone/DST test.

The v7 upgrade retained a v6 backup, generated distinct primary IDs, passed SQLite
integrity and foreign-key checks, and restored the September 19 reader. Normal
Quit initially revealed two debug-mode restrictions: native close/quit guards
and the webview's Cmd-Q interceptor used the simulated lock. Both were corrected.
Only the synthetic reader's counter was reset between these QA attempts.

With the final build, Cmd-Q terminated the process while the reader was still
unfinished. SQLite contained 17 saved seconds and the same session ID. A fresh
launch restored that lesson and continued its remaining time; after the timer
reached zero, the native Complete session button changed September 19 to
`completed` / `done` with 30 debug seconds. September 20 remained `pending` /
`quiz` with zero seconds. The UI correctly showed that next day as still due.

Prepared course bytes, all four assessment tables, classes and path revisions
retained their pre-test hashes. The original assessment profile and production
learner data were not edited. The new shared-runtime table still has zero rows:
this test exercises the primary compatibility command boundary, not the future
FSM/content migration.

Evidence: `/tmp/principia-primary-native-before.json`,
`/tmp/principia-primary-native-quit.json`,
`/tmp/principia-primary-native-completed.json`,
`/tmp/principia-primary-native-build.log`, `/tmp/principia-primary-tests.log`,
`/tmp/principia-primary-clippy.log`, `/tmp/principia-primary-ui-check.log` and
`/tmp/principia-primary-ui-tests.log`. Automated results: 211 Rust tests passed,
six live/external tests ignored, 52 frontend tests passed, no Svelte diagnostics,
and strict Clippy passed.

The final feedback integration was also checked in the packaged desktop app under
`SDR_DATE=2026-10-03`: it restored the existing three-item review directly from
native storage, displayed the saved `let` answers and moved to the second item.
Cmd-Q exited normally. Screen components now remount on owner changes; a frontend
regression rejects out-of-order refresh replies and foreign timer events. Reader
admission tests verify that duplicate starts share one worker and use the latest
persisted counter. The reader ignores a late preparation reply after unmounting.

After the final timer-admission change, a fresh October 4 fixture started at
seven saved seconds under an October 5 clock. Cmd-Q exited at 25 seconds. Relaunch
finished the remaining five seconds; Complete session changed only October 4 to
completed with 30 seconds. October 5 stayed pending at zero, and both IDs were
unchanged. The app then restored the older October 3 unfinished review, which is
also retained in this QA profile. Integrity and foreign-key checks passed.
Evidence: `/tmp/principia-primary-final-before.json`,
`/tmp/principia-primary-final-quit.json` and
`/tmp/principia-primary-final-completed.json`.

## Shared-runtime engineering lesson — 2026-09-10

Isolated profile `com.darkmatter.principia-desk.qa`, upgraded v7→v8 on launch with
a pre-upgrade backup and `integrity_check` = `ok`; `--debug-day`; the tutor
executable overridden with `/usr/bin/false` so no provider ran.

- TypeScript (active, 09:00 weekdays, no chosen starting point) → Learn now:
  a foundations path and class row were created, the session was planned for
  "Structural typing and excess property checking", preparation failed with
  "agent exited with status 1", and the header showed Retry preparation and
  Discard lesson while the sidebar read In progress. Discard lesson left the
  session `skipped` and its preparation job `cancelled`.
- With the app closed, `study_fixture` planned a new session and published the
  bundled "Contextual typing and bidirectional inference" lesson (three checks).
  After relaunch the header offered Resume; the lesson rendered with
  `v1 · qa-fixture` provenance, the accepted-path framing and the frozen check.
- Question 1 by pointer, question 2 by keyboard (deliberately wrong), question 3
  by keyboard: `assessment_work` held revision 3 with answers `0,1,0`.
- Cmd-Q with the lesson open, relaunch, Resume: the saved answers were restored
  on screen. A reflection was typed and the check submitted: the session became
  `completed` at revision 5, `study_results` stored the 67% result and the
  reflection, `classroom_exit_attempts` held three rows (two correct) keyed by
  the study session, one assessment submission existed, the attempt was
  `completed`, and the concept's mastery row read `practicing` with one encounter.
- Progress showed one completed session, TypeScript at 1 completed, the fixture
  lesson with a readable link and 67%, and the discarded lesson as Skipped.

Not covered here: live generation into the runtime, reading-position restore,
language lessons (still legacy), Windows/Linux, enforcement. Screenshots for this
pass were kept under `/tmp/principia-shots/` (20, 24–28, 30).

Reading-position follow-up on the same profile: a second fixture lesson
("Session 2", concept `ts-unknown-any-never`) was scrolled to its first check
question, the app was quit, and `study_checkpoints` held revision 1 with stage
`practice` and reading offset 4420. Relaunch and Resume reopened the lesson at
the exercise hints with the check just below (screenshot 31).
