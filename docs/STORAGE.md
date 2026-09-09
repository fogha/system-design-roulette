# Storage and enrollment contracts

Principia Desk keeps the existing application data location and SQLite database. Display-name changes must not select a new database. `db::open` is the only production connection factory: it enables and verifies foreign keys, applies pending migrations, then enables WAL journaling.

## Numbered migrations

`src-tauri/src/storage/migrations/mod.rs` owns the ordered migration list. Each ledger row contains a version, name, SHA-256 checksum, application time and optional pre-upgrade backup path. `PRAGMA user_version` must agree with the ledger. Unknown newer versions, gaps and changed checksums stop the open before migration writes.

- v1 brings original-main, PR-head and intermediate schemas to the legacy baseline, preserving their IDs and values. Missing columns are detected explicitly. Course-source and exercise-owner rebuilds copy records before replacing a table.
- v2 adds immutable curriculum snapshots, enrollment drafts, classes, accepted path revisions and namespaced legacy crosswalks. It does not enroll a learner, alter an existing program, create schedules or manufacture assessment evidence.

Migration SQL and the frozen `legacy_v1.rs` implementation contribute to their checksums. Once committed, do not edit these sources, even for formatting. Add a new migration for later changes. The runner itself may evolve without changing an already-applied migration's meaning.

An immediate transaction reserves the writer before backup and rechecks the ledger after acquiring the lock. A separate read connection uses SQLite's backup API to include all committed WAL pages. The checked, flushed backup is published under `backups/<database>.before-v<target>.<timestamp>-<nonce>.db`, alongside the database. Unix backup files use mode `0600`. Empty new databases and normal reopens do not create backups.

The entire pending batch runs in one transaction. Every migration must pass SQLite integrity and foreign-key checks before its ledger row is recorded. SQL, backup or integrity failures abort the upgrade; foreign-key enforcement is restored on the connection. Errors after a successful backup include its path. Two concurrent openers share the completed upgrade rather than applying it twice.

Backups are retained; automatic retention/deletion is not implemented. Restore only while the application is closed, and preserve the current database and its WAL files before recovery. A pre-upgrade backup contains no later learning records: restoring it is not a lossless rollback. Once new-schema writes exist, use a compatible build or a forward repair. Do not delete migration history to force an older build to open a newer database.

## Enrollment drafts

`domain/enrollment.rs` is authoritative; thin native commands expose options, fetch the pending draft for a course and save a draft. `src/lib/contracts/enrollment.ts` describes the wire format. Browser previews use the same catalog and fingerprint generator, with separate local storage; they never read or write the native learner database.

A draft records the course reference, goal, entry route, pace, provider-specific model ID and focus preference. The three entry routes are foundations, diagnostic and manual. Manual entry has a course-specific starting stage/band and declared familiarity. These declarations do not award scores, completed lessons or mastery. New draft preferences default to advisory focus; saving one never changes existing enforcement preferences.

The course reference contains a version and a SHA-256 fingerprint of the canonical catalog definition, full course curriculum, prompt and bundled reference lessons. Native and browser fingerprints are checked together. Snapshots are immutable and retain their authored array order. Curriculum changes require an explicit save against refreshed options; updating a draft does not delete its older snapshot.

There is one pending draft per course. Opaque draft IDs and expected revisions reject stale changes. Retrying the same desired state is idempotent, including when the first save succeeded but its response was lost. A failed draft write rolls back a newly inserted snapshot. Choosing a diagnostic currently records intent only; diagnostic rounds, path recommendations, acceptance and runtime selection are subsequent implementation work. The class/path tables are not yet the writer for existing learning engines.

## Regression fixtures

`tests/fixtures/upgrades/` includes SQL schemas extracted from the original main, PR head and intermediate exercise revision, with source commits recorded in their headers. Upgrade tests compare every original column and record, including multiple documents on one date, colliding IDs in different owner namespaces, settings, active work, language bands, mastery aggregates and audio paths. Further tests cover committed WAL data, backup failure, injected late migration failures, integrity failures, concurrent opens and incompatible ledgers. Enrollment tests cover all nine courses, restart, retries, stale edits and rollback without learning credit.
