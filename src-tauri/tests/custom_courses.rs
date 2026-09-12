//! A learner's own class: from a brief to a published course that the rest
//! of the desk treats like a bundled one.

use principia_desk_lib::{
    catalog, classroom,
    classroom::{ConfigureClassroomInput, UpsertClassroomSlotInput},
    db,
    db::CurriculumBrief,
    domain::{
        classes::{self, AcceptPath},
        custom::{self, CourseBrief, CourseDraft, DraftTopic},
        enrollment::{self, EntryChoice, SaveEnrollmentDraft},
        placement,
    },
    language, mastery,
    subjects::engineering,
};
use rusqlite::Connection;

fn fixture() -> (std::path::PathBuf, Connection) {
    let path = std::env::temp_dir().join(format!(
        "principia-custom-{:032x}.db",
        rand::random::<u128>()
    ));
    let conn = db::open(&path).unwrap();
    db::seed_concepts(&conn, include_str!("../seed/concepts.json")).unwrap();
    language::initialize(&conn, "2026-09-12").unwrap();
    classroom::initialize(&conn).unwrap();
    (path, conn)
}

/// The catalog registry is one per process, so each test names its class
/// differently.
fn brief_titled(title: &str) -> CourseBrief {
    CourseBrief {
        title: title.into(),
        outcome: "Build and ship a small command-line tool with tests and a release binary".into(),
        background: "Comfortable in Python".into(),
        trusted_hosts: vec!["doc.rust-lang.org".into(), "docs.rs".into()],
        agent: "claude".into(),
        model: "opus".into(),
        custom_agent_bin: String::new(),
    }
}

fn topic(slug: &str, phase: &str, core: bool, prereqs: &[&str]) -> DraftTopic {
    DraftTopic {
        slug: slug.into(),
        title: format!("Topic {slug}"),
        category: "fundamentals".into(),
        prereqs: prereqs.iter().map(|p| p.to_string()).collect(),
        curriculum: CurriculumBrief {
            phase: phase.into(),
            core,
            learner_outcome: "Explain the mechanism with a worked example and a measured result of your own making".into(),
            mechanisms: vec!["one mechanism".into(), "another mechanism".into()],
            production_scenario: "A tool in daily use meets this problem on a busy day and someone has to fix it before lunch".into(),
            misconceptions: vec!["it is not magic".into()],
            evidence: "A trace showing the mechanism at work in the tool".into(),
            artifact: "A short note with the trace and the fix applied".into(),
            primary_sources: vec![
                "https://doc.rust-lang.org/book/".into(),
                "https://docs.rs/clap/latest/clap/".into(),
            ],
            related_concepts: vec![],
        },
    }
}

fn complete(mut draft: CourseDraft) -> CourseDraft {
    draft.summary = "Build small, fast command-line tools in Rust with care.".into();
    draft.context = "Teach ownership and error handling through small tools that are run and measured, comparing each choice with what the compiler and the operating system actually do.".into();
    draft.environment = "A terminal with a Rust toolchain and a text editor.".into();
    draft.outcome = "Build and ship a small command-line tool with argument parsing, clean error handling, tests and a release binary, and explain each design choice with evidence.".into();
    draft.topics = vec![
        topic("ownership", "foundations", true, &[]),
        topic("errors", "foundations", true, &["ownership"]),
        topic("clap", "mechanisms", true, &["errors"]),
        topic("io", "mechanisms", false, &["ownership"]),
        topic("testing", "production", true, &["clap"]),
        topic("release", "synthesis", true, &["testing", "io"]),
    ];
    draft
}

#[test]
fn a_brief_becomes_a_draft_that_publishes_into_the_catalog_and_teaches_like_any_course() {
    let (_file, conn) = fixture();
    let created = custom::create(&conn, &brief_titled("Rust for CLI tools"), "manual").unwrap();
    assert_eq!(created.id, "custom-rust-for-cli-tools");
    assert_eq!(created.status, "draft");
    assert_eq!(created.draft.entry_points.len(), 4);
    assert!(
        !created.issues.is_empty(),
        "a blank draft is not ready to publish"
    );
    assert!(
        catalog::course(&created.id).is_none(),
        "a draft is not a course yet"
    );
    assert!(custom::publish(&conn, &created.id)
        .unwrap_err()
        .to_string()
        .contains("not ready"));

    let saved = custom::save_draft(&conn, &created.id, complete(created.draft.clone())).unwrap();
    assert!(saved.issues.is_empty(), "{:?}", saved.issues);
    let published = custom::publish(&conn, &created.id).unwrap();
    assert_eq!(
        (published.status.as_str(), published.version),
        ("published", 1)
    );

    // The course is in the catalog, engineering-kind, with its own prompt.
    let course = catalog::course(&created.id).expect("registered");
    assert_eq!(course.label, "Rust for CLI tools");
    assert_eq!(course.kind, catalog::SubjectKind::Engineering);
    assert!(course
        .prompt
        .contains("PROMPT PROFILE: custom.custom-rust-for-cli-tools.v1"));
    assert!(catalog::engineering_ids().contains(&course.id));
    assert!(catalog::all().iter().any(|c| c.id == course.id));
    assert!(classroom::prompt_contracts_are_isolated().is_ok());

    // Its topics are concepts of the course, with prefixed slugs.
    let concepts = db::all_concepts(&conn, &created.id).unwrap();
    assert_eq!(concepts.len(), 6);
    assert!(concepts
        .iter()
        .any(|c| c.slug == "custom-rust-for-cli-tools-ownership"));
    assert_eq!(mastery::overview(&conn, &created.id).unwrap().len(), 6);

    // It has a program with the brief's tutor, and the desk lists it.
    let program = classroom::program_row(&conn, &created.id).unwrap();
    assert_eq!(
        (program.agent.as_str(), program.model.as_str()),
        ("claude", "opus")
    );
    assert!(classroom::program_views(&conn, "2026-09-12")
        .unwrap()
        .iter()
        .any(|view| view.subject_id == created.id));

    // Enrollment works from the registry: options, a manual entry, an
    // accepted path, and a planned lesson on the first topic.
    let options = enrollment::options(&created.id).unwrap();
    assert_eq!(options.familiarity_options.len(), 6);
    assert_eq!(options.entry_points.len(), 4);
    let mut configuration = options.default_configuration;
    configuration.entry = EntryChoice::Manual {
        entry_point: "foundations".into(),
        familiar_competencies: vec![],
    };
    let draft = enrollment::save_draft(
        &conn,
        &SaveEnrollmentDraft {
            id: None,
            expected_revision: None,
            course: options.course.clone(),
            configuration,
        },
    )
    .unwrap();
    let recommendation = placement::recommend(&conn, &draft.id, draft.revision).unwrap();
    let path = classes::accept(
        &conn,
        &AcceptPath {
            draft_id: draft.id,
            expected_revision: draft.revision,
            recommendation_id: recommendation.id,
        },
        "2026-09-12",
    )
    .unwrap();
    assert_eq!(path.recommendation.course.course_id, created.id);
    // With a study time the class activates like any other, and a lesson
    // can be planned on the first topic.
    classroom::upsert_slot(
        &conn,
        &UpsertClassroomSlotInput {
            id: None,
            subject_id: created.id.clone(),
            hour: 7,
            minute: 30,
            weekdays: vec![1, 2, 3, 4, 5],
            enabled: true,
            durations: Default::default(),
            starts: Default::default(),
        },
    )
    .unwrap();
    classroom::configure_program(
        &conn,
        &ConfigureClassroomInput {
            subject_id: created.id.clone(),
            enabled: true,
            agent: "claude".into(),
            model: "opus".into(),
            custom_agent_bin: String::new(),
            session_minutes: 30,
            start_level: None,
            target_level: None,
            weekly_minutes: None,
        },
        "2026-09-12",
    )
    .unwrap();
    let program = classroom::program_row(&conn, &created.id).unwrap();
    assert!(program.enabled);
    let session = engineering::plan(&conn, &program, None, None, "2026-09-12", false).unwrap();
    let chosen = engineering::selection(&session).unwrap();
    assert!(
        chosen.slug.starts_with("custom-rust-for-cli-tools-"),
        "{}",
        chosen.slug
    );

    // The placement check is not available until a bank exists; the other
    // starting points are.
    assert!(placement::bank(&created.id).is_err());

    // A published course survives a restart: loading registers it again.
    catalog::unregister_custom(&created.id);
    assert!(catalog::course(&created.id).is_none());
    assert_eq!(custom::load_published(&conn).unwrap(), 1);
    assert!(catalog::course(&created.id).is_some());
    assert_eq!(
        enrollment::course_snapshot(&created.id).unwrap().0,
        options.course
    );
}

#[test]
fn republishing_bumps_the_version_keeps_taught_topics_and_drops_untaught_ones() {
    let (_file, conn) = fixture();
    let created = custom::create(&conn, &brief_titled("Rust again"), "tutor").unwrap();
    assert_eq!(created.id, "custom-rust-again");
    custom::save_draft(&conn, &created.id, complete(created.draft.clone())).unwrap();
    custom::publish(&conn, &created.id).unwrap();
    let before = enrollment::course_snapshot(&created.id).unwrap().0;
    // One topic was taught; another is removed in the second version.
    conn.execute(
        "UPDATE concepts SET times_picked=1 WHERE slug='custom-rust-again-io'",
        [],
    )
    .unwrap();
    let mut second = custom::get(&conn, &created.id).unwrap().draft;
    second
        .topics
        .retain(|t| t.slug != "io" && t.slug != "release");
    second
        .topics
        .push(topic("ship", "synthesis", true, &["testing"]));
    second
        .topics
        .push(topic("profiling", "production", false, &["clap"]));
    second.entry_points[0].label = "Getting started".into();
    custom::save_draft(&conn, &created.id, second).unwrap();
    let published = custom::publish(&conn, &created.id).unwrap();
    assert_eq!(published.version, 2);
    let course = catalog::course(&created.id).unwrap();
    assert_eq!(course.version, "v2");
    assert_eq!(course.entry_points[0].label, "Getting started");
    let slugs: Vec<String> = db::all_concepts(&conn, &created.id)
        .unwrap()
        .into_iter()
        .map(|c| c.slug)
        .collect();
    assert!(
        slugs.contains(&"custom-rust-again-io".to_string()),
        "a taught topic stays"
    );
    assert!(
        !slugs.contains(&"custom-rust-again-release".to_string()),
        "an untaught one leaves"
    );
    assert!(slugs.contains(&"custom-rust-again-ship".to_string()));
    assert_ne!(
        enrollment::course_snapshot(&created.id)
            .unwrap()
            .0
            .fingerprint,
        before.fingerprint,
        "a new version is a new snapshot"
    );
}

#[test]
fn a_class_file_round_trips_and_a_draft_can_be_deleted_but_a_published_class_cannot() {
    let (_file, conn) = fixture();
    let created = custom::create(&conn, &brief_titled("Rust to share"), "manual").unwrap();
    custom::save_draft(&conn, &created.id, complete(created.draft.clone())).unwrap();
    let file = custom::export(&conn, &created.id).unwrap();
    assert_eq!(file.format, custom::CLASS_FILE_FORMAT);
    let imported = custom::import(&conn, file).unwrap();
    assert_ne!(imported.id, created.id, "an import is a new class");
    assert_eq!(imported.id, "custom-rust-to-share-2");
    assert_eq!(imported.origin, "import");
    assert_eq!(imported.draft.topics.len(), 6);
    assert!(imported.issues.is_empty());
    assert_eq!(custom::list(&conn).unwrap().len(), 2);

    custom::delete_draft(&conn, &imported.id).unwrap();
    assert_eq!(custom::list(&conn).unwrap().len(), 1);
    custom::publish(&conn, &created.id).unwrap();
    assert!(custom::delete_draft(&conn, &created.id).is_err());
}
