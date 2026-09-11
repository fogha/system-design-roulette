//! A lesson as a CSV file: the reading, its resources and exercise, and the
//! check questions, one row each, so a lesson can leave the desk for a
//! spreadsheet, a flashcard deck or a printout.
//!
//! One flat table with a `kind` column holds everything, because a lesson is
//! several kinds of record and a reader can filter by kind. The answer key
//! (correct answers and explanations) is included only once the check has
//! been submitted: the check is the point of the lesson, and a key that can
//! be read from a file before it is answered is not a check.

use crate::classroom::{self, StoredEngineeringLesson};
use crate::domain::assessments::{self, Owner, Purpose, ResponseStatus, Round};
use crate::domain::sessions::{self, SessionId};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

pub const COLUMNS: [&str; 14] = [
    "kind",
    "position",
    "section",
    "text",
    "detail",
    "choice_a",
    "choice_b",
    "choice_c",
    "choice_d",
    "correct_answer",
    "explanation",
    "your_answer",
    "result",
    "url",
];

/// One record of the lesson. Every kind uses `text`; questions fill the
/// choice and answer columns, resources the url, the rest `detail`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub kind: &'static str,
    pub position: Option<usize>,
    pub section: String,
    pub text: String,
    pub detail: String,
    pub choices: [String; 4],
    pub correct_answer: String,
    pub explanation: String,
    pub your_answer: String,
    pub result: String,
    pub url: String,
}

impl Row {
    fn new(kind: &'static str, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            ..Self::default()
        }
    }
    fn meta(key: &str, value: impl Into<String>) -> Self {
        Self {
            section: key.into(),
            ..Self::new("meta", value)
        }
    }
    fn fields(&self) -> [String; 14] {
        [
            self.kind.to_string(),
            self.position.map(|p| p.to_string()).unwrap_or_default(),
            self.section.clone(),
            self.text.clone(),
            self.detail.clone(),
            self.choices[0].clone(),
            self.choices[1].clone(),
            self.choices[2].clone(),
            self.choices[3].clone(),
            self.correct_answer.clone(),
            self.explanation.clone(),
            self.your_answer.clone(),
            self.result.clone(),
            self.url.clone(),
        ]
    }
}

/// The finished export: the rows, and what a reader should know about them.
#[derive(Debug, Clone, Serialize)]
pub struct LessonExport {
    /// A file name without extension, safe on every platform.
    pub file_stem: String,
    pub title: String,
    pub questions: usize,
    /// Whether correct answers and explanations are in the file.
    pub answer_key: bool,
    #[serde(skip)]
    pub rows: Vec<Row>,
}

impl LessonExport {
    pub fn csv(&self) -> String {
        csv(&self.rows)
    }
}

/// RFC 4180 text: CRLF line ends, fields quoted when they need it, doubled
/// quotes inside. A UTF-8 byte-order mark opens the file so spreadsheets read
/// accents and arrows correctly. A field that a spreadsheet would run as a
/// formula (`=`, `+`, `-`, `@`, or a control character first) is prefixed
/// with an apostrophe, because lesson text is written by a language model and
/// a cell must never execute.
pub fn csv(rows: &[Row]) -> String {
    let mut out = String::from("\u{feff}");
    out.push_str(&COLUMNS.join(","));
    out.push_str("\r\n");
    for row in rows {
        let line = row
            .fields()
            .iter()
            .map(|value| field(value))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push_str("\r\n");
    }
    out
}

fn field(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let guarded = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if guarded.contains([',', '"', '\n', '\r']) || guarded.starts_with('\'') {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

/// Split Markdown into its `##` sections, code fences respected. Text before
/// the first heading (the title, an intro) becomes a section with no name.
pub fn split_sections(markdown: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = vec![(String::new(), String::new())];
    let mut fence: Option<&str> = None;
    for line in markdown.lines() {
        let trimmed = line.trim();
        let marker = if trimmed.starts_with("```") {
            Some("```")
        } else if trimmed.starts_with("~~~") {
            Some("~~~")
        } else {
            None
        };
        if let Some(marker) = marker {
            if fence == Some(marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
        }
        if fence.is_none() {
            if let Some(heading) = trimmed.strip_prefix("## ") {
                sections.push((
                    heading.trim().trim_end_matches('#').trim().into(),
                    String::new(),
                ));
                continue;
            }
        }
        let body = &mut sections.last_mut().expect("one section").1;
        body.push_str(line);
        body.push('\n');
    }
    sections
        .into_iter()
        .map(|(title, body)| (title, body.trim().to_string()))
        .filter(|(title, body)| !title.is_empty() || !body.is_empty())
        .collect()
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-');
    trimmed
        .chars()
        .take(60)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

fn file_stem(subject: &str, date: &str, title: &str) -> String {
    let date = if date.is_empty() { "undated" } else { date };
    let mut stem = format!("{}-{date}", slug(subject));
    let title = slug(title);
    if !title.is_empty() {
        stem.push('-');
        stem.push_str(&title);
    }
    stem
}

fn percent(score: Option<f64>) -> String {
    score
        .map(|score| format!("{}%", (score * 100.0).round() as i64))
        .unwrap_or_default()
}

/// A question as the reader saw it, with its key, in either subject's shape.
struct Question {
    position: usize,
    prompt: String,
    choices: Vec<String>,
    correct_index: Option<usize>,
    correct_text: String,
    explanation: String,
    section: String,
    objective: String,
}

impl Question {
    fn from_value(position: usize, body: &serde_json::Value) -> Self {
        let text = |key: &str| body[key].as_str().unwrap_or_default().to_string();
        let choices = body["choices"]
            .as_array()
            .map(|choices| {
                choices
                    .iter()
                    .filter_map(|choice| choice.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let correct_index = body["correct_index"].as_u64().map(|index| index as usize);
        Self {
            position,
            prompt: text("prompt"),
            correct_text: text("correct_answer"),
            explanation: text("explanation"),
            section: if body["section"].is_string() {
                text("section")
            } else {
                text("strand")
            },
            objective: text("learning_objective"),
            choices,
            correct_index,
        }
    }

    fn correct_answer(&self) -> String {
        self.correct_index
            .and_then(|index| self.choices.get(index).cloned())
            .unwrap_or_else(|| self.correct_text.clone())
    }

    fn row(&self, key: bool, answer: Option<&str>) -> Row {
        let mut row = Row::new("question", self.prompt.clone());
        row.position = Some(self.position);
        row.section = self.section.clone();
        row.detail = self.objective.clone();
        for (slot, choice) in row.choices.iter_mut().zip(&self.choices) {
            *slot = choice.clone();
        }
        if key {
            row.correct_answer = self.correct_answer();
            row.explanation = self.explanation.clone();
        }
        if let Some(answer) = answer {
            row.your_answer = answer.to_string();
            if key {
                row.result = if answer == self.correct_answer() {
                    "correct".into()
                } else {
                    "incorrect".into()
                };
            }
        }
        row
    }
}

/// The learner's answer to a question, resolved to its text.
fn answer_text(question: &Question, raw: &str) -> String {
    raw.parse::<usize>()
        .ok()
        .and_then(|index| question.choices.get(index).cloned())
        .unwrap_or_else(|| raw.to_string())
}

/// Questions and answers from a frozen check round: the questions as shown,
/// the submitted answers if the round was submitted, else the saved drafts.
fn round_questions(round: &Round) -> (Vec<Question>, Vec<Option<String>>, bool) {
    let questions: Vec<Question> = round
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| Question::from_value(index + 1, &item.body))
        .collect();
    let responses = round
        .submission
        .as_ref()
        .map(|submission| &submission.responses)
        .unwrap_or(&round.responses);
    let answers = round
        .items
        .iter()
        .zip(&questions)
        .map(|(item, question)| {
            responses
                .get(&item.id)
                .filter(|response| response.status == ResponseStatus::Answered)
                .map(|response| answer_text(question, &response.answer))
        })
        .collect();
    (questions, answers, round.submission.is_some())
}

fn exercise_rows(exercise: &crate::generator::Exercise) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut head = Row::new("exercise", exercise.title.clone());
    head.detail = exercise.instructions.clone();
    rows.push(head);
    if let Some(deliverable) = exercise
        .deliverable
        .as_deref()
        .filter(|d| !d.trim().is_empty())
    {
        rows.push(Row::new("exercise_deliverable", deliverable));
    }
    if let Some(starter) = exercise
        .starter_code
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        rows.push(Row::new("exercise_starter_code", starter));
    }
    for (index, hint) in exercise.hints.iter().enumerate() {
        let mut row = Row::new("exercise_hint", hint.clone());
        row.position = Some(index + 1);
        rows.push(row);
    }
    rows
}

fn work_rows(draft: Option<&str>, completed: bool, reflection: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(draft) = draft.filter(|d| !d.trim().is_empty()) {
        rows.push(Row::new("exercise_draft", draft));
    }
    if !reflection.trim().is_empty() {
        rows.push(Row::new("exercise_reflection", reflection.trim()));
    }
    if completed {
        rows.push(Row::new("exercise_status", "completed"));
    }
    rows
}

fn resource_rows(resources: &[crate::generator::Resource]) -> Vec<Row> {
    resources
        .iter()
        .enumerate()
        .map(|(index, resource)| {
            let mut row = Row::new("resource", resource.title.clone());
            row.position = Some(index + 1);
            row.detail = resource.why.clone();
            row.section = resource.kind.clone();
            row.url = resource.url.clone();
            row
        })
        .collect()
}

fn section_rows(markdown: &str) -> Vec<Row> {
    split_sections(markdown)
        .into_iter()
        .enumerate()
        .map(|(index, (title, body))| {
            let mut row = Row::new("section", body);
            row.position = Some(index + 1);
            row.section = title;
            row
        })
        .collect()
}

fn key_note(key: bool) -> Row {
    Row::meta(
        "answer_key",
        if key {
            "included"
        } else {
            "withheld until the check is submitted"
        },
    )
}

/// Export one saved lesson by the same source and owner the Progress page
/// uses to open it.
pub fn export(conn: &Connection, source: &str, owner_id: &str) -> Result<LessonExport, String> {
    match source {
        "study" => study(conn, owner_id),
        "classroom" => legacy_classroom(conn, parse_id(owner_id)?),
        "language" => legacy_language(conn, parse_id(owner_id)?),
        "primary" => primary(conn, owner_id),
        _ => Err("Unknown lesson source".into()),
    }
}

fn parse_id(owner_id: &str) -> Result<i64, String> {
    owner_id
        .parse()
        .map_err(|_| "Invalid lesson identity".to_string())
}

fn study(conn: &Connection, owner_id: &str) -> Result<LessonExport, String> {
    let id = SessionId(owner_id.to_string());
    let session = sessions::get(conn, &id).map_err(|e| e.to_string())?;
    let lesson = sessions::lesson(conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or("This lesson has not been prepared yet, so there is nothing to export.")?;
    let course_id = session.context.course.course_id.clone();
    let spec = classroom::subject(&course_id)?;
    let program = classroom::program_row(conn, &course_id)?;
    let date = session.context.selection["service_date"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let round = assessments::latest(conn, &Owner::StudySession(id.0.clone()), Purpose::ExitCheck)
        .map_err(|e| e.to_string())?
        .and_then(|attempt| attempt.rounds.last().cloned());
    let result = sessions::result(conn, &id).map_err(|e| e.to_string())?;
    let status = if session.status.terminal() {
        format!("{:?}", session.status).to_lowercase()
    } else {
        "in progress".to_string()
    };
    let score = result
        .as_ref()
        .and_then(|result| result.outcome["result"]["score"].as_f64());
    let tutor = lesson.content.provenance["agent"]
        .as_str()
        .or_else(|| lesson.content.provenance["runner"].as_str())
        .unwrap_or(&program.agent)
        .to_string();

    let mut rows = vec![
        Row::meta("title", lesson.content.title.clone()),
        Row::meta("class", program.label.clone()),
        Row::meta("date", date.clone()),
        Row::meta("status", status),
    ];
    if let Some(score) = score {
        rows.push(Row::meta("score", percent(Some(score))));
    }
    rows.push(Row::meta("tutor", tutor));
    rows.push(Row::meta("lesson_id", id.0.clone()));

    let (questions, answers, submitted) = match &round {
        Some(round) => round_questions(round),
        None => (Vec::new(), Vec::new(), false),
    };
    let key = submitted || session.status.terminal();
    rows.push(key_note(key));

    match spec.kind {
        crate::catalog::SubjectKind::Engineering => {
            let stored: StoredEngineeringLesson =
                serde_json::from_value(lesson.content.body.clone()).map_err(|e| e.to_string())?;
            rows.insert(1, Row::meta("topic", stored.concept_title.clone()));
            rows.insert(2, Row::meta("category", stored.category.clone()));
            if let Some(note) = &stored.research_note {
                rows.push(Row::meta("sources", note.clone()));
            }
            rows.extend(section_rows(&stored.markdown));
            rows.extend(resource_rows(&stored.resources));
            if let Some(exercise) = &stored.exercise {
                rows.extend(exercise_rows(exercise));
                let work = crate::subjects::engineering::exercise_view(conn, &id)?;
                if let Some(work) = work {
                    rows.extend(work_rows(
                        work.draft.as_deref(),
                        work.completed,
                        &work.reflection,
                    ));
                }
            }
            let questions = if questions.is_empty() {
                stored
                    .questions
                    .iter()
                    .enumerate()
                    .map(|(index, question)| {
                        Question::from_value(
                            index + 1,
                            &serde_json::to_value(question).unwrap_or_default(),
                        )
                    })
                    .collect()
            } else {
                questions
            };
            let count = questions.len();
            rows.extend(question_rows(&questions, &answers, key));
            Ok(LessonExport {
                file_stem: file_stem(&program.short_code, &date, &lesson.content.title),
                title: lesson.content.title,
                questions: count,
                answer_key: key,
                rows,
            })
        }
        crate::catalog::SubjectKind::Language => {
            let stored: crate::language::StoredLesson =
                serde_json::from_value(lesson.content.body.clone()).map_err(|e| e.to_string())?;
            rows.insert(1, Row::meta("scenario", stored.scenario.clone()));
            rows.insert(2, Row::meta("can_do", stored.can_do.clone()));
            rows.extend(language_rows(&stored));
            let questions = if questions.is_empty() {
                stored
                    .questions
                    .iter()
                    .enumerate()
                    .map(|(index, question)| {
                        Question::from_value(
                            index + 1,
                            &serde_json::to_value(question).unwrap_or_default(),
                        )
                    })
                    .collect()
            } else {
                questions
            };
            let count = questions.len();
            rows.extend(question_rows(&questions, &answers, key));
            Ok(LessonExport {
                file_stem: file_stem(&program.short_code, &date, &lesson.content.title),
                title: lesson.content.title,
                questions: count,
                answer_key: key,
                rows,
            })
        }
    }
}

fn question_rows(questions: &[Question], answers: &[Option<String>], key: bool) -> Vec<Row> {
    questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            question.row(key, answers.get(index).and_then(|answer| answer.as_deref()))
        })
        .collect()
}

fn language_rows(stored: &crate::language::StoredLesson) -> Vec<Row> {
    let mut rows = section_rows(&stored.markdown);
    for (index, phrase) in stored.phrases.iter().enumerate() {
        let mut row = Row::new("phrase", phrase.target.clone());
        row.position = Some(index + 1);
        row.detail = phrase.translation.clone();
        row.section = phrase.note.clone();
        rows.push(row);
    }
    for (index, line) in stored.dialogue.iter().enumerate() {
        let mut row = Row::new("dialogue", line.target.clone());
        row.position = Some(index + 1);
        row.section = line.speaker.clone();
        row.detail = line.translation.clone();
        rows.push(row);
    }
    for (kind, text) in [
        ("speaking_prompt", &stored.speaking_prompt),
        ("writing_prompt", &stored.writing_prompt),
        ("listening_text", &stored.listen_text),
    ] {
        if !text.trim().is_empty() {
            rows.push(Row::new(kind, text.clone()));
        }
    }
    rows
}

/// The columns of a retired classroom session the export reads.
struct LegacyClassroomRow {
    label: String,
    short_code: String,
    date: String,
    status: String,
    score: Option<f64>,
    payload: String,
    response: String,
    exercise_completed: bool,
    reflection: String,
}

fn legacy_classroom(conn: &Connection, session_id: i64) -> Result<LessonExport, String> {
    let row = conn
        .query_row(
            "SELECT p.label, p.short_code, s.session_date, s.status, s.score,
                    s.payload_json, s.response_json, s.exercise_completed, s.exercise_reflection
             FROM classroom_sessions s JOIN classroom_programs p ON p.subject_id = s.subject_id
             WHERE s.id = ?1",
            [session_id],
            |r| {
                Ok(LegacyClassroomRow {
                    label: r.get(0)?,
                    short_code: r.get(1)?,
                    date: r.get(2)?,
                    status: r.get(3)?,
                    score: r.get(4)?,
                    payload: r.get(5)?,
                    response: r.get(6)?,
                    exercise_completed: r.get::<_, i64>(7)? != 0,
                    reflection: r.get(8)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(LegacyClassroomRow {
        label,
        short_code,
        date,
        status,
        score,
        payload,
        response,
        exercise_completed,
        reflection,
    }) = row
    else {
        return Err("That lesson is no longer on this desk.".into());
    };
    let stored: StoredEngineeringLesson =
        serde_json::from_str(&payload).map_err(|e| e.to_string())?;
    let key = status == "completed";
    let mut rows = vec![
        Row::meta("title", stored.title.clone()),
        Row::meta("topic", stored.concept_title.clone()),
        Row::meta("category", stored.category.clone()),
        Row::meta("class", label),
        Row::meta("date", date.clone()),
        Row::meta("status", status.replace('_', " ")),
    ];
    if let Some(score) = score {
        rows.push(Row::meta("score", percent(Some(score))));
    }
    rows.push(Row::meta("tutor", stored.source.clone()));
    rows.push(key_note(key));
    if let Some(note) = &stored.research_note {
        rows.push(Row::meta("sources", note.clone()));
    }
    rows.extend(section_rows(&stored.markdown));
    rows.extend(resource_rows(&stored.resources));
    if let Some(exercise) = &stored.exercise {
        rows.extend(exercise_rows(exercise));
        let draft = crate::db::get_exercise_draft(conn, None, Some(session_id))
            .map_err(|e| e.to_string())?;
        rows.extend(work_rows(draft.as_deref(), exercise_completed, &reflection));
    }
    let answers: Vec<Option<String>> = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|value| value["answers"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|answer| answer.as_u64().map(|index| index.to_string()))
        .collect();
    let questions: Vec<Question> = stored
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            Question::from_value(
                index + 1,
                &serde_json::to_value(question).unwrap_or_default(),
            )
        })
        .collect();
    let answers = questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            answers
                .get(index)
                .cloned()
                .flatten()
                .map(|raw| answer_text(question, &raw))
        })
        .collect::<Vec<_>>();
    let count = questions.len();
    rows.extend(question_rows(&questions, &answers, key));
    Ok(LessonExport {
        file_stem: file_stem(&short_code, &date, &stored.title),
        title: stored.title,
        questions: count,
        answer_key: key,
        rows,
    })
}

fn legacy_language(conn: &Connection, session_id: i64) -> Result<LessonExport, String> {
    let row: Option<(String, String, String, String, Option<f64>, String)> = conn
        .query_row(
            "SELECT s.language, p.label, s.session_date, s.status, s.score, s.lesson_json
             FROM language_sessions s JOIN classroom_programs p ON p.subject_id = s.language
             WHERE s.id = ?1",
            [session_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((language, label, date, status, score, payload)) = row else {
        return Err("That lesson is no longer on this desk.".into());
    };
    let stored: crate::language::StoredLesson =
        serde_json::from_str(&payload).map_err(|e| e.to_string())?;
    let key = status == "completed";
    let mut rows = vec![
        Row::meta("title", stored.title.clone()),
        Row::meta("scenario", stored.scenario.clone()),
        Row::meta("can_do", stored.can_do.clone()),
        Row::meta("class", label),
        Row::meta("date", date.clone()),
        Row::meta("status", status.replace('_', " ")),
    ];
    if let Some(score) = score {
        rows.push(Row::meta("score", percent(Some(score))));
    }
    rows.push(key_note(key));
    rows.extend(language_rows(&stored));
    let questions: Vec<Question> = stored
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            Question::from_value(
                index + 1,
                &serde_json::to_value(question).unwrap_or_default(),
            )
        })
        .collect();
    let count = questions.len();
    rows.extend(question_rows(&questions, &[], key));
    Ok(LessonExport {
        file_stem: file_stem(&language, &date, &stored.title),
        title: stored.title,
        questions: count,
        answer_key: key,
        rows,
    })
}

/// A lesson of the retired daily routine: the course, its questions and the
/// answers given that day. Those sessions are finished, so the key is always
/// included.
fn primary(conn: &Connection, owner_id: &str) -> Result<LessonExport, String> {
    let course_id: Option<i64> = conn
        .query_row(
            "SELECT c.id FROM primary_session_ids i
             JOIN sessions s ON s.date = i.legacy_date
             JOIN courses c ON c.session_date = s.date AND c.concept_id = s.concept_id
             WHERE i.session_id = ?1 ORDER BY c.id DESC LIMIT 1",
            [owner_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let course_id = course_id.ok_or("That lesson is no longer on this desk.")?;
    let course = crate::db::get_course(conn, course_id)
        .map_err(|e| e.to_string())?
        .ok_or("That lesson is no longer on this desk.")?;
    let concept = crate::db::get_concept(conn, course.concept_id).map_err(|e| e.to_string())?;
    let title = concept
        .as_ref()
        .map(|concept| concept.title.clone())
        .unwrap_or_else(|| "Saved study session".into());
    let focus = concept
        .as_ref()
        .map(|concept| concept.focus.clone())
        .unwrap_or_default();
    let resources: Vec<crate::generator::Resource> =
        serde_json::from_str(&course.resources_json).unwrap_or_default();
    let mut rows = vec![
        Row::meta("title", title.clone()),
        Row::meta("topic", title.clone()),
        Row::meta(
            "category",
            concept
                .as_ref()
                .map(|concept| concept.category.clone())
                .unwrap_or_default(),
        ),
        Row::meta("class", crate::focus::label(&focus).to_string()),
        Row::meta("date", course.session_date.clone()),
        Row::meta("status", "completed"),
        Row::meta("tutor", course.source.clone()),
        key_note(true),
    ];
    rows.extend(section_rows(&course.markdown));
    rows.extend(resource_rows(&resources));
    if let Some(exercise) =
        crate::db::get_course_exercise(conn, course_id).map_err(|e| e.to_string())?
    {
        rows.extend(exercise_rows(&crate::generator::Exercise {
            title: exercise.title,
            instructions: exercise.instructions,
            starter_code: exercise.starter_code,
            deliverable: exercise.deliverable,
            hints: exercise.hints,
        }));
        let draft = crate::db::get_exercise_draft(conn, Some(course_id), None)
            .map_err(|e| e.to_string())?;
        let (completed, reflection) =
            crate::db::get_exercise_completion(conn, Some(course_id), None)
                .map_err(|e| e.to_string())?;
        rows.extend(work_rows(draft.as_deref(), completed, &reflection));
    }
    let attempts =
        crate::db::attempts_for_session(conn, &course.session_date).map_err(|e| e.to_string())?;
    let questions = crate::db::questions_for_course(conn, course_id).map_err(|e| e.to_string())?;
    let count = questions.len();
    for (index, question) in questions.iter().enumerate() {
        let choices: Vec<String> = question
            .choices_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        let shaped = Question {
            position: index + 1,
            prompt: question.prompt.clone(),
            correct_index: None,
            correct_text: question.correct_answer.clone(),
            explanation: question.explanation.clone(),
            section: question.kind.clone(),
            objective: String::new(),
            choices,
        };
        let attempt = attempts
            .iter()
            .find(|attempt| attempt.question_id == question.id);
        let mut row = shaped.row(true, attempt.map(|attempt| attempt.user_answer.as_str()));
        if let Some(attempt) = attempt {
            row.result = if attempt.correct {
                "correct".into()
            } else {
                "incorrect".into()
            };
        }
        rows.push(row);
    }
    Ok(LessonExport {
        file_stem: file_stem(&focus, &course.session_date, &title),
        title,
        questions: count,
        answer_key: true,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_quoted_escaped_and_never_formulas() {
        assert_eq!(field("plain"), "plain");
        assert_eq!(field(""), "");
        assert_eq!(field("a, b"), "\"a, b\"");
        assert_eq!(field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(field("two\nlines"), "\"two\nlines\"");
        assert_eq!(field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(field("- a bullet"), "\"'- a bullet\"");
        assert_eq!(field("@mention"), "\"'@mention\"");
    }

    #[test]
    fn the_table_has_a_header_a_bom_and_crlf_line_ends() {
        let mut question = Row::new("question", "Which wins?");
        question.position = Some(1);
        question.choices[0] = "micro".into();
        question.choices[1] = "macro".into();
        question.correct_answer = "micro".into();
        let text = csv(&[Row::meta("title", "Event loop"), question]);
        let mut lines = text.split("\r\n");
        assert_eq!(
            lines.next().unwrap(),
            format!("\u{feff}{}", COLUMNS.join(","))
        );
        assert_eq!(lines.next().unwrap(), "meta,,title,Event loop,,,,,,,,,,");
        assert_eq!(
            lines.next().unwrap(),
            "question,1,,Which wins?,,micro,macro,,,micro,,,,"
        );
        assert_eq!(lines.next(), Some(""));
    }

    #[test]
    fn sections_split_on_second_level_headings_outside_fences() {
        let markdown = "# Title\n\nIntro line.\n\n## First\n\n*~1 min · hint*\n\n```md\n## not a heading\n```\n\n## Second ##\nbody";
        let sections = split_sections(markdown);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].0, "");
        assert_eq!(sections[0].1, "# Title\n\nIntro line.");
        assert_eq!(sections[1].0, "First");
        assert!(sections[1].1.contains("## not a heading"));
        assert_eq!(sections[2], ("Second".to_string(), "body".to_string()));
    }

    #[test]
    fn file_stems_are_plain_and_bounded() {
        assert_eq!(
            file_stem(
                "SD",
                "2026-09-11",
                "CAP in practice: choosing partition behaviour, not a permanent label"
            ),
            "sd-2026-09-11-cap-in-practice-choosing-partition-behaviour-not-a-permanent"
        );
        assert_eq!(
            file_stem("german", "", "Sich vorstellen — Teil 1"),
            "german-undated-sich-vorstellen-teil-1"
        );
    }

    #[test]
    fn a_question_row_holds_the_key_only_when_asked() {
        let question = Question::from_value(
            2,
            &serde_json::json!({
                "prompt": "Which queue drains first?",
                "choices": ["microtasks", "macrotasks", "render", "idle"],
                "correct_index": 0,
                "explanation": "Microtasks run before the next macrotask.",
                "section": "Core mechanics",
                "learning_objective": "order the queues"
            }),
        );
        let hidden = question.row(false, Some("macrotasks"));
        assert_eq!(hidden.correct_answer, "");
        assert_eq!(hidden.explanation, "");
        assert_eq!(hidden.your_answer, "macrotasks");
        assert_eq!(hidden.result, "");
        let shown = question.row(true, Some("macrotasks"));
        assert_eq!(shown.correct_answer, "microtasks");
        assert_eq!(shown.result, "incorrect");
        assert_eq!(shown.section, "Core mechanics");
        assert_eq!(shown.detail, "order the queues");
        assert_eq!(answer_text(&question, "1"), "macrotasks");
        assert_eq!(answer_text(&question, "free text"), "free text");
    }
}
