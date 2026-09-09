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
