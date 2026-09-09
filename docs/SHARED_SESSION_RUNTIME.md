# Shared study runtime

The native foundation lives in `src-tauri/src/domain/sessions.rs`, with schema v6 in `006_study_sessions.sql`. Existing lesson IPC and the three legacy engines still own their current records. The primary engine must switch its writer, due/consumed decisions, active-session reads and history together before this runtime becomes the user-facing study engine. Engineering classroom and language adapters follow that cutover. This document records an implementation boundary, not completion of P4–P6.

## Identity and planning

A session receives a stable `study-…` ID before provider work begins. A caller-supplied request key makes an identical planning request idempotent, including after completion or a change in class settings. Reusing that key for a different selection fails. A distinct request cannot replace an owner's resumable session.

Class sessions reference the exact accepted path and curriculum snapshot. Planning snapshots the current tutor, focus preference, goal and pace alongside the subject adapter's selected lesson/unit, reason, prerequisite advice and stage sequence. Revising the path or tutor does not rewrite existing sessions. Planning against a stale path or curriculum fails before new work is created.

The unassigned daily routine has an explicit compatibility owner and a service date in its provenance. It does not create a class or infer a permanent subject assignment. Its date is not session identity. Legacy import and routine-assignment UI are still pending.

## Lifecycle and preparation

The persisted lifecycle is `planned → preparing → ready → active ↔ paused → completed/skipped`. SQLite constrains transitions, ownership, one resumable session per owner and one logically active session. Activating advisory work pauses the previous foreground session without changing its checkpoint. Focused/strict activation remains unavailable through this foundation until the OS focus coordinator and recovery protocol are integrated.

Planning creates a durable preparation job. Workers claim it with a bounded lease; a second worker receives no lease while the first is live. Heartbeats extend live leases. After expiry, another worker can recover the same request and invalidate the old token. Failed requests retain their identity and error until an explicit retry. Skipping cancels outstanding preparation.

Publication checks the current lease and freezes one immutable lesson version, including the adapter's content, assessment material and provenance. The lesson and ready state commit together. An identical publication retry returns the original version; a different payload or obsolete token cannot replace it. Provider calls happen outside SQLite transactions.

## Saved work and results

Checkpoints contain the active stage, reading anchor/offset and subject-specific editor/activity state. Work is keyed by session ID and uses its own revision. A delayed save after a class switch still targets its captured session. Competing revisions are rejected; a lost-response retry of the same save is safe. Lifecycle changes and checkpoint saves have separate revisions.

The existing assessment runtime now accepts a `study_session` owner. It retains its frozen rounds, answer drafts, submissions and rubric versions. Class-owned placement remains independent of a lesson's assessment and session slot.

Completion calls a native subject adapter inside one transaction. That adapter validates required work and writes evidence and schedule projections. Unfinished session assessments block completion. Any failed validation or storage write rolls back the result, grade, projection and session transition. An identical completion retry returns the original result without repeating the projection. Skipping retains unanswered work and abandons that session's active assessments; it does not abandon an independent class diagnostic.

Terminal results freeze the submitted checkpoint. Original lesson content, ownership, submitted work and result rows are immutable. Follow-up practice must create a later session/artifact revision rather than changing the original evidence.

## Verification and next integration

`tests/study_sessions.rs` exercises real SQLite connections and process-style reopenings: concurrent planning, competing preparation claims, expiry/recovery, stale worker rejection, save conflicts, foreground handoff, midnight resume, revised-path/tutor isolation, atomic grade/progress failure, result retries, terminal immutability and independent placement. Migration tests retain original assessment records and verify the v5 backup and unchanged v1–v5 ledger entries.

The next integration needs primary session import/crosswalks, compatibility IPC keyed to the stable session, native preparation worker calls, captured timer ownership and simultaneous due/consumed/history reader cutover. The same runtime then supports the classroom and CEFR adapters. Occurrences/timezones, OS enforcement coordination, shared lesson UI and evidence-based curriculum completion remain separate unfinished gates in `PRODUCT_EVOLUTION_PLAN.md`.
