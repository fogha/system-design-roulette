# Desktop design verification — 2026-09-09

Tested the packaged macOS Tauri app from `tauri://localhost`, using the real Rust
commands and SQLite storage. The QA build uses identifier
`com.darkmatter.principia-desk.qa`, separate from the existing production data.
The shipping identifier remains `com.darkmatter.system-design-roulette` so the
display-name change does not move or strand existing study history.

The native display name is **Principia Desk**. The original blueprint grid,
Fraunces headings, compact mono controls, node panels and app mark remain the
design reference. New course glyphs are inline SVG, with no remote asset dependency.

## Verified

- Packaged app starts, completes setup and relaunches with persisted preferences.
- All nine native catalog courses render. Searching `bash` finds Linux Bash and
  Bash Scripting; study actions and management controls occupy separate rows.
- Custom language-level dropdown opens above card boundaries without a native
  translucent menu. Arrow + Enter selects; Escape cancels an uncommitted change;
  Tab advances focus; type-ahead finds B1.
- German A2 → B1 settings save through native IPC, are present in SQLite and
  remain visible after relaunch. Database integrity check returns `ok`.
- Bash Scripting enrollment uses the same dropdown; manual entry at Integration
  and capstone autosaves through the native enrollment command. The UI continues
  to state that path acceptance is pending implementation.
- The provider dropdown flips above its trigger near the window bottom. Explicit
  trigger focus handles WebKit pointer behavior, and ResizeObserver keeps the
  menu aligned when layout changes.
- At 640 × 540 the native navigation remains visible, and the enrollment menu
  fits the window and opens upward. Clicking outside dismisses it without changing
  the value. The saved Bash Scripting starting stage remains after relaunch.
- A native relaunch exposed dark text and missing navigation paint. Explicit noir
  text/color-scheme, a dark native title bar and a positioned navigation header
  restore the intended contrast; the corrected Today screen was inspected.
- Source audit finds no `<select>`, `<option>`, browser time inputs or JS native
  alert/confirm/prompt calls in the app. Remaining semantic inputs are styled.

## Checks and limits

`npm run check` reports no Svelte errors or warnings. All 34 existing frontend
tests pass. `npm run tauri build -- --debug --bundles app` completes with the QA
identifier override. All three scheduler unit tests pass after updating the macOS
fallback bundle path; scheduler registration identities are retained. Rust
formatting and `git diff --check` pass. See the product evolution plan for remaining implementation
and release gates.

Native testing uses `--debug-day` to suppress kiosk and OS scheduler changes.
This pass does not certify hard enforcement, live provider generation, signing,
notarization, or Windows/Linux runtime behavior. It verifies the packaged macOS
UI, native persistence and the changed controls.
