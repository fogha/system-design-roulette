# Storage and enrollment contracts

Principia Desk keeps the existing application data location and SQLite database. Display-name changes must not select a new database. `db::open` is the only production connection factory: it enables and verifies foreign keys, applies pending migrations, then enables WAL journaling.

## Numbered migrations

`src-tauri/src/storage/migrations/mod.rs` owns the ordered migration list. Each ledger row contains a version, name, SHA-256 checksum, application time and optional pre-upgrade backup path. `PRAGMA user_version` must agree with the ledger. Unknown newer versions, gaps and changed checksums stop the open before migration writes.

- v1 brings original-main, PR-head and intermediate schemas to the legacy baseline, preserving their IDs and values. Missing columns are detected explicitly. Course-source and exercise-owner rebuilds copy records before replacing a table.
- v2 adds immutable curriculum snapshots, enrollment drafts, classes, accepted path revisions and namespaced legacy crosswalks. It does not enroll a learner, alter an existing program, create schedules or manufacture assessment evidence.
- v3 adds shared assessment attempts, frozen rounds, revisioned answer drafts and immutable submissions. It preserves the original quiz config bytes and prevents further writes through the retired quiz keys.
- v4 records runner call metadata; v5 widens class tutor identities; v6 adds the shared study runtime tables; v7 assigns stable primary session identities.
- v8 rebuilds classroom check evidence so a row belongs to either a legacy classroom session or a shared-runtime study session.
- v9 adds rule revisions and `schedule_occurrences`: one durable appointment per rule per local service date with its resolved instant and a disposition consumed exactly once.

Migration SQL and the frozen `legacy_v1.rs` implementation contribute to their checksums. Once committed, do not edit these sources, even for formatting. Add a new migration for later changes. The runner itself may evolve without changing an already-applied migration's meaning.

An immediate transaction reserves the writer before backup and rechecks the ledger after acquiring the lock. A separate read connection uses SQLite's backup API to include all committed WAL pages. The checked, flushed backup is published under `backups/<database>.before-v<target>.<timestamp>-<nonce>.db`, alongside the database. Unix backup files use mode `0600`. The flush opens the file for writing, because Windows refuses to flush a handle opened without write access. Empty new databases and normal reopens do not create backups.

The entire pending batch runs in one transaction. Every migration must pass SQLite integrity and foreign-key checks before its ledger row is recorded. SQL, backup or integrity failures abort the upgrade; foreign-key enforcement is restored on the connection. Errors after a successful backup include its path. Two concurrent openers share the completed upgrade rather than applying it twice.

Backups are retained; automatic retention/deletion is not implemented. Restore only while the application is closed, and preserve the current database and its WAL files before recovery. A pre-upgrade backup contains no later learning records: restoring it is not a lossless rollback. Once new-schema writes exist, use a compatible build or a forward repair. Do not delete migration history to force an older build to open a newer database.

## Enrollment drafts

`domain/enrollment.rs` is authoritative; thin native commands expose options, fetch the pending draft for a course and save a draft. `src/lib/contracts/enrollment.ts` describes the wire format. Browser previews use the same catalog and fingerprint generator, with separate local storage; they never read or write the native learner database.

A draft records the course reference, goal, entry route, pace, provider-specific model ID and focus preference. The three entry routes are foundations, diagnostic and manual. Manual entry has a course-specific starting stage/band and declared familiarity. These declarations do not award scores, completed lessons or mastery. New draft preferences default to advisory focus; saving one never changes existing enforcement preferences.

The course reference contains a version and a SHA-256 fingerprint of the canonical catalog definition, full course curriculum, prompt and bundled reference lessons. Native and browser fingerprints are checked together. Snapshots are immutable and retain their authored array order. Curriculum changes require an explicit save against refreshed options; updating a draft does not delete its older snapshot.

There is one pending draft per course. Opaque draft IDs and expected revisions reject stale changes. Retrying the same desired state is idempotent, including when the first save succeeded but its response was lost. A failed draft write rolls back a newly inserted snapshot. Choosing a diagnostic currently records intent only; diagnostic rounds, path recommendations, acceptance and runtime selection are subsequent implementation work. The class/path tables are not yet the writer for existing learning engines.

## Regression fixtures

`tests/fixtures/upgrades/` includes SQL schemas extracted from the original main, PR head and intermediate exercise revision, with source commits recorded in their headers. Upgrade tests compare every original column and record, including multiple documents on one date, colliding IDs in different owner namespaces, settings, active work, language bands, mastery aggregates and audio paths. Further tests cover committed WAL data, backup failure, injected late migration failures, integrity failures, concurrent opens and incompatible ledgers. Enrollment tests cover all nine courses, restart, retries, stale edits and rollback without learning credit.

## Shared assessment runtime

`domain/assessments.rs` owns frozen item definitions and rubrics, saved responses, revisions and submissions. A round has an opaque ID independent of its question IDs. A response distinguishes an unfinished draft, a confirmed answer and an explicit skip. Adapters supply trusted question bodies and interpret results; learner commands supply only a round ID, expected revision and answer. Saving an assessment never activates a lesson, changes focus, consumes a schedule occurrence or awards mastery.

One active attempt is permitted per owner. Follow-up rounds require the previous round's submission. Identical lost-response retries return the saved work; a different answer with a stale revision is rejected. Submission joins the adapter's transaction, checks the round and answer revision captured before grading, and stores the first immutable result. Submitted answers cannot be edited. The enrollment owner and diagnostic purpose are supported by the runtime and tested in isolation; a user-facing diagnostic bank, commands and path recommendation flow are still pending.

The retired daily quiz was the first adapter to use this runtime; its screens, commands and engine have since been removed. Rounds, drafts and results imported from the legacy config rows stay in the assessment tables with a namespaced crosswalk, and nothing writes new primary rounds. Legacy `attempts` rows keep their old boolean correctness field, including the original self-assessment convention, and Progress still reads that history. The canonical assessment result retains an unavailable grader's verdict as `null`; a free-text answer without a verdict does not update mastery.

Migration v3 preserves `quiz_round:*`, `pending_answers:*` and `quiz_result:*` in `legacy_assessment_config`, including malformed or unrecognized data. Known full question snapshots and matching answers import once, with a namespaced crosswalk. An unknown snapshot format stops automatic reconstruction and retains the original bytes. Unmatched legacy answer IDs remain in the raw archive and are counted in the attempt context; an archive recovery UI is pending. Older results can still be read without manufacturing new grading evidence.

`features/assessments/work-editor.ts` serializes native writes per round, flushes on navigation and keeps unsaved work in local recovery storage. A newer saved revision requires an explicit recovery choice. Malformed recovery records remain untouched until the learner chooses the saved answers. Native persistence still works if browser recovery storage is unavailable. The quiz restores partial text and provides an explicit grading action when every answer was confirmed before a restart.
