//! Ordered schema migrations with immutable checksums, consistent pre-upgrade
//! snapshots and one transaction for the whole pending batch.
mod legacy_v1;

use crate::db::{DbError, Result};
use rusqlite::{params, Connection, DatabaseName, OpenFlags, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Migration {
    version: u32,
    name: &'static str,
    sources: &'static [&'static str],
    apply: fn(&Connection) -> Result<()>,
}
impl Migration {
    fn checksum(&self) -> String {
        let mut hash = Sha256::new();
        for source in self.sources {
            hash.update((source.len() as u64).to_be_bytes());
            hash.update(source.as_bytes());
        }
        format!("{:x}", hash.finalize())
    }
}
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "legacy_baseline",
        sources: &[
            legacy_v1::BASELINE,
            legacy_v1::COLUMNS,
            legacy_v1::COURSES,
            legacy_v1::EXERCISES,
            include_str!("legacy_v1.rs"),
        ],
        apply: legacy_v1::apply,
    },
    Migration {
        version: 2,
        name: "enrollment_paths",
        sources: &[include_str!("002_enrollment_paths.sql")],
        apply: |conn| {
            conn.execute_batch(include_str!("002_enrollment_paths.sql"))?;
            Ok(())
        },
    },
    Migration {
        version: 3,
        name: "shared_assessments",
        sources: &[include_str!("003_assessments.sql")],
        apply: |conn| {
            conn.execute_batch(include_str!("003_assessments.sql"))?;
            Ok(())
        },
    },
    Migration {
        version: 4,
        name: "agent_calls",
        sources: &[include_str!("004_agent_calls.sql")],
        apply: |conn| {
            conn.execute_batch(include_str!("004_agent_calls.sql"))?;
            Ok(())
        },
    },
    Migration {
        version: 5,
        name: "class_tutors",
        sources: &[include_str!("005_class_tutors.sql")],
        apply: |conn| {
            conn.execute_batch(include_str!("005_class_tutors.sql"))?;
            Ok(())
        },
    },
];

pub fn enable_foreign_keys(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    let enabled: bool = conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if !enabled {
        return Err(DbError::Invalid(
            "could not enable SQLite foreign keys".into(),
        ));
    }
    Ok(())
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get(0),
    )?)
}

fn applied_count(conn: &Connection, migrations: &[Migration]) -> Result<usize> {
    let version: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if !table_exists(conn, "schema_migrations")? {
        if version != 0 {
            return Err(DbError::Invalid(format!(
                "database schema version {version} has no migration ledger"
            )));
        }
        return Ok(0);
    }
    let mut statement =
        conn.prepare("SELECT version, name, checksum FROM schema_migrations ORDER BY version")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, u32>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut count = 0;
    for row in rows {
        let (recorded_version, name, checksum) = row?;
        let Some(migration) = migrations.get(count) else {
            return Err(DbError::Invalid(format!(
                "database schema v{recorded_version} requires a newer Principia Desk build"
            )));
        };
        if recorded_version != migration.version
            || name != migration.name
            || checksum != migration.checksum()
        {
            return Err(DbError::Invalid(format!("migration history mismatch at v{recorded_version}; this build cannot safely upgrade the database")));
        }
        count += 1;
    }
    let expected = count
        .checked_sub(1)
        .map_or(0, |index| migrations[index].version);
    if version != expected {
        return Err(DbError::Invalid(format!(
            "schema version {version} disagrees with migration ledger v{expected}"
        )));
    }
    Ok(count)
}

fn integrity_check(conn: &Connection, foreign_keys: bool) -> Result<()> {
    let mut statement = conn.prepare("PRAGMA integrity_check")?;
    let messages = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if messages != ["ok"] {
        return Err(DbError::Invalid(format!(
            "SQLite integrity check failed: {}",
            messages.join("; ")
        )));
    }
    if foreign_keys {
        let mut statement = conn.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        if let Some(row) = rows.next()? {
            let table: String = row.get(0)?;
            let rowid: Option<i64> = row.get(1)?;
            return Err(DbError::Invalid(format!(
                "foreign-key violation in {table}, row {rowid:?}"
            )));
        }
    }
    Ok(())
}

/// The migration connection already holds the write reservation. A separate
/// read connection sees the committed pre-migration snapshot, including WAL
/// pages, while no concurrent writer can race between backup and migration.
/// https://www.sqlite.org/backup.html
fn backup_before_upgrade(database: &Path, target_version: u32) -> Result<PathBuf> {
    let parent = database
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let directory = parent.join("backups");
    fs::create_dir_all(&directory)?;
    let filename = database.file_name().unwrap_or_default().to_string_lossy();
    let nonce: u64 = rand::random();
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.9fZ");
    let destination = directory.join(format!(
        "{filename}.before-v{target_version}.{stamp}-{nonce:016x}.db"
    ));
    let partial = destination.with_extension("partial");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(&partial)?);
    let result = (|| -> Result<()> {
        let source = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        source.busy_timeout(std::time::Duration::from_secs(5))?;
        source.backup(DatabaseName::Main, &partial, None)?;
        let snapshot = Connection::open(&partial)?;
        snapshot.pragma_update(None, "journal_mode", "DELETE")?;
        integrity_check(&snapshot, false)?;
        drop(snapshot);
        fs::File::open(&partial)?.sync_all()?;
        fs::rename(&partial, &destination)?;
        #[cfg(unix)]
        fs::File::open(&directory)?.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(DbError::Invalid(format!(
            "pre-migration backup failed: {error}"
        )));
    }
    Ok(destination)
}

pub fn migrate(conn: &Connection, database: &Path) -> Result<()> {
    run(conn, database, MIGRATIONS)
}

fn run(conn: &Connection, database: &Path, migrations: &[Migration]) -> Result<()> {
    let count = applied_count(conn, migrations)?;
    if count == migrations.len() {
        return Ok(());
    }
    // Foreign keys are disabled only for the schema-rebuild transaction, as in
    // SQLite's documented create/copy/drop/rename procedure. Restore on errors.
    conn.pragma_update(None, "foreign_keys", false)?;
    let mut backup = None;
    let result = (|| -> Result<()> {
        let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        let count = applied_count(&tx, migrations)?;
        if count == migrations.len() {
            return Ok(());
        }
        let has_data_tables: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%')", [], |row| row.get(0))?;
        if has_data_tables {
            backup = Some(backup_before_upgrade(
                database,
                migrations.last().unwrap().version,
            )?);
        }
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL,
            applied_at TEXT NOT NULL, backup_path TEXT
        );",
        )?;
        for migration in &migrations[count..] {
            (migration.apply)(&tx).map_err(|error| {
                DbError::Invalid(format!(
                    "migration v{} ({}) failed: {error}",
                    migration.version, migration.name
                ))
            })?;
            integrity_check(&tx, true)?;
            tx.execute("INSERT INTO schema_migrations (version, name, checksum, applied_at, backup_path) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![migration.version, migration.name, migration.checksum(), chrono::Utc::now().to_rfc3339(), backup.as_ref().map(|path| path.to_string_lossy().into_owned())])?;
            tx.pragma_update(None, "user_version", migration.version)?;
        }
        tx.commit()?;
        Ok(())
    })();
    let restored = enable_foreign_keys(conn);
    result.map_err(|error| {
        DbError::Invalid(format!(
            "{error}{}",
            backup
                .as_ref()
                .map(|path| format!(". Pre-upgrade backup: {}", path.display()))
                .unwrap_or_default()
        ))
    })?;
    restored?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v4_class_tutor_upgrade_retains_dependents_and_accepts_every_runner() {
        let directory = std::env::temp_dir().join(format!(
            "principia-tutor-upgrade-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("learner.db");
        let conn = Connection::open(&path).unwrap();
        run(&conn, &path, &MIGRATIONS[..4]).unwrap();
        crate::language::initialize(&conn, "2026-09-09").unwrap();
        crate::classroom::initialize(&conn).unwrap();
        conn.execute_batch("INSERT INTO classroom_schedule_slots(id,subject_id,hour,minute,created_at) VALUES(42,'linux-bash',18,30,'original');
            INSERT INTO classroom_sessions(id,subject_id,slot_id,session_date,title,payload_json,prompt_version,started_at,exercise_draft) VALUES(51,'linux-bash',42,'2026-09-09','Saved work','{\"original\":true}','v1','original','learner script');").unwrap();
        let original: String = conn.query_row("SELECT json_group_array(json_object('id',subject_id,'agent',agent,'model',model,'updated',updated_at)) FROM classroom_programs", [], |r| r.get(0)).unwrap();
        run(&conn, &path, MIGRATIONS).unwrap();
        let current: String = conn.query_row("SELECT json_group_array(json_object('id',subject_id,'agent',agent,'model',model,'updated',updated_at)) FROM classroom_programs", [], |r| r.get(0)).unwrap();
        assert_eq!(original, current);
        assert_eq!(
            conn.query_row(
                "SELECT slot_id,exercise_draft,payload_json FROM classroom_sessions WHERE id=51",
                [],
                |r| Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?
                ))
            )
            .unwrap(),
            (42, "learner script".into(), "{\"original\":true}".into())
        );
        for runner in crate::agents::RunnerId::ALL {
            for id in [runner.id(), runner.legacy_id()] {
                conn.execute(
                    "UPDATE classroom_programs SET agent=?1,model=?2 WHERE subject_id='linux-bash'",
                    params![id, crate::agents::default_model(runner)],
                )
                .unwrap();
            }
        }
        integrity_check(&conn, true).unwrap();
        let backup: String = conn
            .query_row(
                "SELECT backup_path FROM schema_migrations WHERE version=5",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let old = Connection::open(backup).unwrap();
        assert_eq!(
            old.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            4
        );
        assert_eq!(
            old.query_row(
                "SELECT COUNT(*) FROM classroom_sessions WHERE id=51",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }

    fn fail_after_writes(conn: &Connection) -> Result<()> {
        conn.execute_batch("CREATE TABLE partial_upgrade(id INTEGER PRIMARY KEY); INSERT INTO config VALUES ('partial-write', 'must roll back'); INSERT INTO absent_table VALUES (1);")?;
        Ok(())
    }
    fn introduce_orphan(conn: &Connection) -> Result<()> {
        conn.execute_batch("CREATE TABLE orphan(parent_key TEXT REFERENCES config(key)); INSERT INTO orphan VALUES ('missing-parent');")?;
        Ok(())
    }

    #[test]
    fn failing_sql_and_failed_integrity_checks_roll_back_the_whole_pending_batch() {
        for apply in [
            fail_after_writes as fn(&Connection) -> Result<()>,
            introduce_orphan,
        ] {
            let directory = std::env::temp_dir().join(format!(
                "principia-migration-failure-{}-{:016x}",
                std::process::id(),
                rand::random::<u64>()
            ));
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join("learner.db");
            let conn = Connection::open(&path).unwrap();
            enable_foreign_keys(&conn).unwrap();
            conn.execute_batch("CREATE TABLE config(key TEXT PRIMARY KEY, value TEXT NOT NULL); INSERT INTO config VALUES ('original', 'retained');").unwrap();
            let migrations = [
                Migration {
                    version: 1,
                    name: MIGRATIONS[0].name,
                    sources: MIGRATIONS[0].sources,
                    apply: MIGRATIONS[0].apply,
                },
                Migration {
                    version: 2,
                    name: "injected_failure",
                    sources: &["test fixture"],
                    apply,
                },
            ];
            let error = run(&conn, &path, &migrations).unwrap_err().to_string();
            assert!(error.contains("Pre-upgrade backup:"));
            assert!(conn.is_autocommit());
            assert!(!table_exists(&conn, "schema_migrations").unwrap());
            assert!(!table_exists(&conn, "partial_upgrade").unwrap());
            assert!(!table_exists(&conn, "orphan").unwrap());
            assert!(
                !table_exists(&conn, "concepts").unwrap(),
                "the earlier migration must also roll back"
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM config", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                    .unwrap(),
                0
            );
            assert!(conn
                .pragma_query_value(None, "foreign_keys", |row| row.get::<_, bool>(0))
                .unwrap());
        }
    }
}
