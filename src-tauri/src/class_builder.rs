//! The tutor's part in a learner's own class: drafting the curriculum from
//! the brief, reading a draft back, and the source check that fetches
//! every primary source. Each call is a bare-JSON exchange with the
//! configured runner through the generator's typed path, announced in the
//! execution feed under its own run so the Logs page shows it.

use crate::domain::custom::{
    self, CourseBrief, CourseDraft, DraftIssue, ReviewFinding, SourceCheck, MAX_TOPICS, MIN_TOPICS,
    STAGES,
};
use crate::domain::placement::{Bank, Choice, Question, CRITERIA_PER_ENTRY_POINT};
use crate::generator::{GenError, Generator, Result};
use serde::Deserialize;
use std::time::Duration;

/// How long a drafting call may take. A whole curriculum is a long answer.
const DRAFT_TIMEOUT: Duration = Duration::from_secs(420);
const REVIEW_TIMEOUT: Duration = Duration::from_secs(240);
const BANK_TIMEOUT: Duration = Duration::from_secs(360);

/// The shape the tutor is asked for, as a schema the prompt shows. It is the
/// draft itself minus the id, which the desk assigns.
fn draft_schema() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "label": "short class name, as it appears in a list (2-4 words)",
        "native_label": "two or three words naming the field, lowercase",
        "short_code": "two or three capital letters",
        "title": "the full course title",
        "summary": "one sentence, what the course is for",
        "context": "how this subject should be taught, 2-4 sentences: the reasoning to build, what to compare, what to show and measure",
        "outcome": "the bounded thing the learner will have made and can defend at the end, 2-3 sentences",
        "environment": "the tools on the learner's machine the course assumes, one sentence",
        "source_hosts": ["hostnames of primary documentation, e.g. doc.rust-lang.org"],
        "entry_points": [
            {"id": "foundations", "label": "stage name in this course's words"},
            {"id": "mechanisms", "label": "..."},
            {"id": "production", "label": "..."},
            {"id": "synthesis", "label": "..."}
        ],
        "topics": [{
            "slug": "lowercase-hyphenated, unique in the course",
            "title": "the topic, as a lesson title",
            "category": "one or two lowercase words grouping related topics",
            "prereqs": ["slugs of topics this one builds on; only earlier or same-stage topics"],
            "curriculum": {
                "phase": "foundations | mechanisms | production | synthesis | elective",
                "core": true,
                "learner_outcome": "what the learner can do after the lesson, one sentence of at least eight words, observable",
                "mechanisms": ["at least two named mechanisms the lesson explains"],
                "production_scenario": "a concrete situation at work where this matters, at least eight words",
                "misconceptions": ["at least one common wrong belief, stated"],
                "evidence": "what the learner shows to prove it, at least six words",
                "artifact": "what the lesson leaves behind, at least six words",
                "primary_sources": ["at least two absolute URLs on the source hosts, real pages you are confident exist"]
            }
        }]
    }))
    .unwrap()
}

fn draft_prompt(brief: &CourseBrief) -> String {
    let hosts = if brief.trusted_hosts.is_empty() {
        "none named; choose the primary documentation hosts for the subject".to_string()
    } else {
        brief.trusted_hosts.join(", ")
    };
    format!(
        "You are designing a self-study course for one learner who will study it in \
         thirty-minute to two-hour sessions, one topic per session, with a tutor writing \
         each lesson from primary documentation. Design the whole curriculum now.\n\n\
         WHAT THE LEARNER WANTS TO BE ABLE TO DO:\n{outcome}\n\n\
         WORKING TITLE: {title}\n\
         WHAT THEY ALREADY KNOW: {background}\n\
         DOCUMENTATION HOSTS THEY TRUST: {hosts}\n\n\
         Rules:\n\
         - Between {min} and {max} topics, most of them core, ordered so that each builds \
         on what came before. Four stages in this order: foundations (the ideas and the \
         first observations), mechanisms (how it works underneath), production (using it \
         under real constraints), synthesis (the capstone that produces the outcome). \
         Every stage has at least one core topic. Give the stages names in this \
         course's own words.\n\
         - A topic is one lesson: narrow enough to teach and check in one session, with \
         a mechanism the learner can observe on their own machine.\n\
         - Prerequisites point only at topics in the same or an earlier stage. No cycles.\n\
         - Every topic cites at least two primary-source URLs on the source hosts: \
         reference pages, specifications, manuals, official guides. Only cite pages you \
         are confident exist at that exact URL; prefer stable index or chapter pages \
         over deep anchors. Do not invent hosts.\n\
         - The outcome names something the learner will have made, not a feeling.\n\
         - Write for the learner's background: if they know nothing, foundations starts \
         from nothing.\n\n\
         Return ONLY one JSON object with exactly this shape, no markdown fences, no \
         commentary:\n{schema}",
        outcome = brief.outcome.trim(),
        title = brief.title.trim(),
        background = if brief.background.trim().is_empty() {
            "not stated"
        } else {
            brief.background.trim()
        },
        min = MIN_TOPICS.max(12),
        max = MAX_TOPICS.min(36),
        schema = draft_schema(),
    )
}

fn issues_text(issues: &[DraftIssue]) -> String {
    issues
        .iter()
        .take(40)
        .map(|issue| format!("- {}: {}", issue.at, issue.message))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Deserialize)]
struct Review {
    findings: Vec<ReviewFinding>,
}

impl Generator {
    /// Draft the curriculum from the brief with the class's tutor. A draft
    /// that fails the validator goes back once with the reasons; what
    /// comes back is normalized and returned with whatever issues remain,
    /// so the learner sees them in the editor rather than losing the draft.
    pub async fn draft_custom_course(
        &self,
        id: &str,
        brief: &CourseBrief,
    ) -> Result<(CourseDraft, Vec<DraftIssue>, String)> {
        let scoped = self.scoped("course-draft");
        let agent = if brief.agent.is_empty() {
            scoped.current_agent()
        } else {
            brief.agent.clone()
        };
        let model = if brief.model.is_empty() {
            scoped.current_model()
        } else {
            brief.model.clone()
        };
        let custom_bin = if agent == "custom" {
            brief.custom_agent_bin.clone()
        } else {
            scoped.current_custom_bin()
        };
        scoped.log(format!(
            "class builder: asking {agent} ({model}) to draft \"{}\"",
            brief.title.trim()
        ));
        let prompt = draft_prompt(brief);
        let (raw, source) = scoped
            .run_exact_for::<CourseDraft>(
                &agent,
                &custom_bin,
                &prompt,
                false,
                DRAFT_TIMEOUT,
                &model,
            )
            .await?;
        let mut draft = custom::normalize(with_id(raw, id, brief));
        let mut issues = custom::validate(&draft);
        scoped.log(format!(
            "class builder: {} topics drafted, {} issue(s) against the validator",
            draft.topics.len(),
            issues.len()
        ));
        if !issues.is_empty() {
            let correction = format!(
                "Correct this course draft. It failed these deterministic checks:\n{}\n\n\
                 Keep the course, its stages and its topics; change only what the checks \
                 name (add missing fields, fix prerequisites, move sources onto the source \
                 hosts or add the host, split or merge topics as needed). Return ONLY the \
                 complete corrected JSON object in the same shape, no commentary.\n\n\
                 DRAFT:\n{}",
                issues_text(&issues),
                serde_json::to_string(&draft).map_err(|error| GenError::Parse(format!(
                    "could not serialize draft: {error}"
                )))?
            );
            match scoped
                .run_exact_for::<CourseDraft>(
                    &agent,
                    &custom_bin,
                    &correction,
                    false,
                    DRAFT_TIMEOUT,
                    &model,
                )
                .await
            {
                Ok((corrected, _)) => {
                    let corrected = custom::normalize(with_id(corrected, id, brief));
                    let remaining = custom::validate(&corrected);
                    scoped.log(format!(
                        "class builder: after one correction, {} issue(s) remain",
                        remaining.len()
                    ));
                    if remaining.len() <= issues.len() {
                        draft = corrected;
                        issues = remaining;
                    }
                }
                Err(error) => {
                    scoped.log(format!(
                        "class builder: the correction pass failed ({error}); keeping the first draft"
                    ));
                }
            }
        }
        Ok((draft, issues, source))
    }

    /// Ask the tutor to read the draft back and say what is wrong with it.
    pub async fn review_custom_course(
        &self,
        brief: &CourseBrief,
        draft: &CourseDraft,
    ) -> Result<(Vec<ReviewFinding>, String)> {
        let scoped = self.scoped("course-review");
        let agent = if brief.agent.is_empty() {
            scoped.current_agent()
        } else {
            brief.agent.clone()
        };
        let model = if brief.model.is_empty() {
            scoped.current_model()
        } else {
            brief.model.clone()
        };
        let custom_bin = if agent == "custom" {
            brief.custom_agent_bin.clone()
        } else {
            scoped.current_custom_bin()
        };
        scoped.log(format!(
            "class builder: asking {agent} ({model}) to review \"{}\"",
            draft.label
        ));
        let prompt = format!(
            "Review this self-study curriculum as a demanding course designer. The learner \
             wants to be able to: {outcome}. They already know: {background}.\n\n\
             Look for: a topic whose learner outcome is not observable; a prerequisite that \
             should exist and does not, or one that is wrong; two topics that are really \
             one; a topic too broad for one session; a stage that jumps; a primary source \
             that does not support the topic it is attached to; a missing topic without \
             which the outcome cannot be reached; ordering that teaches a mechanism before \
             its foundation. Do not restate the course. Be specific and brief.\n\n\
             Return ONLY a JSON object: {{\"findings\": [{{\"severity\": \"high|medium|low\", \
             \"topic\": \"slug of the topic concerned, or empty for the course as a whole\", \
             \"message\": \"what is wrong, one or two sentences\", \"fix\": \"the concrete \
             change you propose, or empty\"}}]}}. An empty list means the draft is sound. \
             At most twelve findings, highest severity first.\n\n\
             DRAFT:\n{draft}",
            outcome = brief.outcome.trim(),
            background = if brief.background.trim().is_empty() {
                "not stated"
            } else {
                brief.background.trim()
            },
            draft = serde_json::to_string(draft)
                .map_err(|error| GenError::Parse(format!("could not serialize draft: {error}")))?
        );
        let (review, source) = scoped
            .run_exact_for::<Review>(&agent, &custom_bin, &prompt, false, REVIEW_TIMEOUT, &model)
            .await?;
        let slugs: Vec<&str> = draft.topics.iter().map(|t| t.slug.as_str()).collect();
        let mut findings: Vec<ReviewFinding> = review
            .findings
            .into_iter()
            .filter(|finding| !finding.message.trim().is_empty())
            .map(|mut finding| {
                finding.severity = match finding.severity.trim().to_lowercase().as_str() {
                    "high" => "high",
                    "low" => "low",
                    _ => "medium",
                }
                .into();
                if !slugs.contains(&finding.topic.as_str()) {
                    finding.topic = String::new();
                }
                finding
            })
            .take(12)
            .collect();
        let rank = |severity: &str| match severity {
            "high" => 0,
            "medium" => 1,
            _ => 2,
        };
        findings.sort_by_key(|finding| rank(&finding.severity));
        scoped.log(format!(
            "class builder: review returned {} finding(s)",
            findings.len()
        ));
        Ok((findings, source))
    }
}

/// One question as the tutor writes it; the desk assigns ids and criteria.
#[derive(Debug, Clone, Deserialize)]
pub struct WrittenQuestion {
    pub topic: String,
    pub label: String,
    pub prompt: String,
    pub choices: Vec<String>,
    pub answer: String,
    pub explanation: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Deserialize)]
struct WrittenBank {
    questions: Vec<WrittenQuestion>,
}

/// Hold the tutor's questions to the bank's shape: exactly three per stage,
/// each on a core topic of that stage, four distinct choices with the key
/// among them, an explanation, and a source on the course's hosts. The
/// reasons come back for a correction round.
pub fn bank_from_written(
    draft: &CourseDraft,
    written: Vec<WrittenQuestion>,
) -> std::result::Result<Bank, String> {
    let mut reasons = Vec::new();
    let mut questions = Vec::new();
    let mut per_stage = std::collections::HashMap::<String, usize>::new();
    let on_host = |url: &str| {
        crate::research::host_of(url).is_some_and(|host| {
            draft
                .source_hosts
                .iter()
                .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
        })
    };
    for (index, item) in written.into_iter().enumerate() {
        let n = index + 1;
        let Some(topic) = draft.topics.iter().find(|t| t.slug == item.topic) else {
            reasons.push(format!(
                "question {n}: topic {:?} is not in the course",
                item.topic
            ));
            continue;
        };
        let stage = topic.curriculum.phase.clone();
        if stage == "elective" {
            reasons.push(format!(
                "question {n}: {} is an elective; sample a core topic",
                item.topic
            ));
            continue;
        }
        if item.prompt.trim().is_empty()
            || item.label.trim().is_empty()
            || item.explanation.trim().is_empty()
        {
            reasons.push(format!(
                "question {n}: prompt, label and explanation are all required"
            ));
            continue;
        }
        let choices: Vec<String> = item
            .choices
            .iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        let distinct: std::collections::HashSet<&str> =
            choices.iter().map(|c| c.as_str()).collect();
        if choices.len() != 4 || distinct.len() != 4 {
            reasons.push(format!("question {n}: exactly four distinct choices"));
            continue;
        }
        let Some(key) = choices.iter().position(|c| c == item.answer.trim()) else {
            reasons.push(format!(
                "question {n}: the answer must be one of the choices, word for word"
            ));
            continue;
        };
        if !item.source.trim().is_empty() && !on_host(&item.source) {
            reasons.push(format!(
                "question {n}: the source must be on the course's hosts"
            ));
            continue;
        }
        let count = per_stage.entry(stage.clone()).or_default();
        if *count >= CRITERIA_PER_ENTRY_POINT {
            // Extra questions for a stage are kept out rather than failing the bank.
            continue;
        }
        *count += 1;
        let ordinal = questions.len() + 1;
        questions.push(Question {
            id: format!("{}-entry-{ordinal}", draft.id),
            criterion: format!("{}-criterion-{ordinal}", draft.id),
            competency: format!("{}-{}", draft.id, topic.slug),
            entry_point: stage,
            label: item.label.trim().to_string(),
            prompt: item.prompt.trim().to_string(),
            choices: choices
                .iter()
                .enumerate()
                .map(|(i, text)| Choice {
                    id: (i + 1).to_string(),
                    text: text.clone(),
                })
                .collect(),
            answer: (key + 1).to_string(),
            explanation: item.explanation.trim().to_string(),
            followup_for: None,
            source: item.source.trim().to_string(),
            voided: false,
            void_reason: String::new(),
        });
    }
    for (stage, _) in STAGES {
        let have = per_stage.get(stage).copied().unwrap_or(0);
        if have < CRITERIA_PER_ENTRY_POINT {
            reasons.push(format!(
                "the {stage} stage has {have} usable question(s); it needs {CRITERIA_PER_ENTRY_POINT}, each on a core topic of that stage"
            ));
        }
    }
    if !reasons.is_empty() {
        return Err(reasons.join("\n"));
    }
    Ok(Bank {
        course_id: draft.id.clone(),
        version: format!("written-{}", chrono::Utc::now().format("%Y%m%d%H%M%S")),
        estimated_minutes: 14,
        scope_note: "Twelve short questions, three for each stage of this course, written by the tutor from the course's own sources, so the check can place you at any stage rather than guessing past the ones it never sampled.".into(),
        questions,
    })
}

fn bank_prompt(brief: &CourseBrief, draft: &CourseDraft) -> String {
    let topics = draft
        .topics
        .iter()
        .filter(|t| t.curriculum.core && t.curriculum.phase != "elective")
        .map(|t| {
            format!(
                "- {} [{}]: {} — outcome: {} — sources: {}",
                t.slug,
                t.curriculum.phase,
                t.title,
                t.curriculum.learner_outcome,
                t.curriculum.primary_sources.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Write the placement check for a self-study course: {per} four-choice questions for          each of the four stages (foundations, mechanisms, production, synthesis), {total} in          all, each on a different core topic of its stage where the stage has enough topics.          A question tests whether the learner already has the topic's outcome; it is short,          concrete, answerable in under a minute, and its key is verifiable from the topic's          primary sources. The three wrong choices are plausible mistakes, not jokes. Cite one          of the topic's sources per question.

         COURSE: {title}
OUTCOME: {outcome}
WHAT THE LEARNER ALREADY KNOWS: {background}
         SOURCE HOSTS: {hosts}

CORE TOPICS:
{topics}

         Return ONLY a JSON object: {{\"questions\": [{{\"topic\": \"slug from the list\", \
         \"label\": \"3-6 words naming the skill\", \"prompt\": \"the question\", \"choices\": \
         [\"four\", \"distinct\", \"answers\", \"here\"], \"answer\": \"the correct choice, word \
         for word\", \"explanation\": \"why, in one or two sentences\", \"source\": \"one of \
         the topic's source URLs\"}}]}}. No markdown fences, no commentary.",
        per = CRITERIA_PER_ENTRY_POINT,
        total = CRITERIA_PER_ENTRY_POINT * 4,
        title = draft.title,
        outcome = draft.outcome,
        background = if brief.background.trim().is_empty() {
            "not stated"
        } else {
            brief.background.trim()
        },
        hosts = draft.source_hosts.join(", "),
    )
}

impl Generator {
    /// Ask the tutor to write the question bank: three cited questions per
    /// stage. A bank that fails the shape goes back once with the reasons.
    pub async fn write_custom_course_bank(
        &self,
        brief: &CourseBrief,
        draft: &CourseDraft,
    ) -> Result<(Bank, String)> {
        let scoped = self.scoped("course-bank");
        let agent = if brief.agent.is_empty() {
            scoped.current_agent()
        } else {
            brief.agent.clone()
        };
        let model = if brief.model.is_empty() {
            scoped.current_model()
        } else {
            brief.model.clone()
        };
        let custom_bin = if agent == "custom" {
            brief.custom_agent_bin.clone()
        } else {
            scoped.current_custom_bin()
        };
        scoped.log(format!(
            "class builder: asking {agent} ({model}) for the question bank of \"{}\"",
            draft.label
        ));
        let prompt = bank_prompt(brief, draft);
        let (written, source) = scoped
            .run_exact_for::<WrittenBank>(&agent, &custom_bin, &prompt, false, BANK_TIMEOUT, &model)
            .await?;
        let first = bank_from_written(draft, written.questions);
        let bank = match first {
            Ok(bank) => bank,
            Err(reasons) => {
                scoped.log(format!(
                    "class builder: the bank failed its checks; asking for one correction\n{reasons}"
                ));
                let correction = format!(
                    "Correct this question bank. It failed these checks:\n{reasons}\n\n\
                     Keep the good questions; replace or fix the others. Return ONLY the \
                     complete JSON object in the same shape.\n\nORIGINAL_REQUEST:\n{prompt}"
                );
                let (again, _) = scoped
                    .run_exact_for::<WrittenBank>(
                        &agent,
                        &custom_bin,
                        &correction,
                        false,
                        BANK_TIMEOUT,
                        &model,
                    )
                    .await?;
                bank_from_written(draft, again.questions).map_err(|remaining| {
                    GenError::Parse(format!(
                        "the question bank failed its checks after one correction: {remaining}"
                    ))
                })?
            }
        };
        scoped.log(format!(
            "class builder: {} questions written, three per stage",
            bank.questions.len()
        ));
        Ok((bank, source))
    }
}

/// The tutor's draft with the desk's id and the brief's hosts folded in.
fn with_id(mut draft: CourseDraft, id: &str, brief: &CourseBrief) -> CourseDraft {
    draft.id = id.into();
    if draft.label.trim().is_empty() {
        draft.label = brief.title.trim().into();
    }
    for host in &brief.trusted_hosts {
        if !draft.source_hosts.iter().any(|h| h == host) {
            draft.source_hosts.push(host.clone());
        }
    }
    if draft.entry_points.is_empty() {
        draft.entry_points = custom::default_stages();
    }
    draft
}

/// Fetch every primary source of the draft and say which ones answered.
/// Off-host URLs are reported without being fetched.
pub async fn verify_sources(
    researcher: &crate::research::Researcher,
    feed: &crate::execution_log::Feed,
    draft: &CourseDraft,
) -> Vec<SourceCheck> {
    let on_host = |url: &str| {
        crate::research::host_of(url).is_some_and(|host| {
            draft
                .source_hosts
                .iter()
                .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
        })
    };
    let mut checks = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for topic in &draft.topics {
        for url in &topic.curriculum.primary_sources {
            let state = if !on_host(url) {
                "off-host"
            } else if seen.contains(url) {
                checks
                    .iter()
                    .find(|c: &&SourceCheck| &c.url == url)
                    .map(|c| c.state.as_str())
                    .unwrap_or("unreachable")
            } else if researcher.url_resolves(url).await {
                "reachable"
            } else {
                "unreachable"
            };
            seen.insert(url.clone());
            checks.push(SourceCheck {
                topic: topic.slug.clone(),
                url: url.clone(),
                state: state.to_string(),
            });
        }
    }
    let unreachable = checks.iter().filter(|c| c.state == "unreachable").count();
    let off = checks.iter().filter(|c| c.state == "off-host").count();
    feed.say(format!(
        "class builder: {} source(s) checked, {unreachable} unreachable, {off} off the course's hosts",
        checks.len()
    ));
    checks
}

/// The stage labels a draft carries, for prompts and the feed.
pub fn stage_labels(draft: &CourseDraft) -> Vec<(String, String)> {
    STAGES
        .iter()
        .map(|(id, default)| {
            let label = draft
                .entry_points
                .iter()
                .find(|e| e.id == *id)
                .map(|e| e.label.clone())
                .unwrap_or_else(|| (*default).into());
            ((*id).into(), label)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::CurriculumBrief;
    use crate::domain::custom::DraftTopic;

    fn draft_with_topics() -> CourseDraft {
        let topic = |slug: &str, phase: &str| DraftTopic {
            slug: slug.into(),
            title: format!("Topic {slug}"),
            category: "fundamentals".into(),
            prereqs: vec![],
            curriculum: CurriculumBrief {
                phase: phase.into(),
                core: true,
                primary_sources: vec!["https://doc.rust-lang.org/book/".into()],
                ..Default::default()
            },
        };
        CourseDraft {
            id: "custom-rust".into(),
            label: "Rust".into(),
            title: "Rust".into(),
            source_hosts: vec!["doc.rust-lang.org".into()],
            entry_points: custom::default_stages(),
            topics: vec![
                topic("a", "foundations"),
                topic("b", "mechanisms"),
                topic("c", "production"),
                topic("d", "synthesis"),
            ],
            ..Default::default()
        }
    }

    fn written(topic: &str, answer: &str) -> WrittenQuestion {
        WrittenQuestion {
            topic: topic.into(),
            label: "A skill".into(),
            prompt: "Which is it?".into(),
            choices: vec!["one".into(), "two".into(), "three".into(), "four".into()],
            answer: answer.into(),
            explanation: "Because.".into(),
            source: "https://doc.rust-lang.org/book/".into(),
        }
    }

    #[test]
    fn a_written_bank_is_held_to_three_per_stage_on_core_topics_with_the_key_among_the_choices() {
        let draft = draft_with_topics();
        let mut questions = Vec::new();
        for topic in ["a", "b", "c", "d"] {
            for _ in 0..3 {
                questions.push(written(topic, "two"));
            }
        }
        let bank = bank_from_written(&draft, questions.clone()).unwrap();
        assert_eq!(bank.questions.len(), 12);
        assert_eq!(bank.questions[0].competency, "custom-rust-a");
        assert_eq!(bank.questions[0].answer, "2");
        assert_eq!(bank.questions[0].choices.len(), 4);
        assert_eq!(bank.questions[11].entry_point, "synthesis");
        assert!(bank.questions.iter().all(|q| !q.voided));
        // A missing stage, a key not among the choices, an unknown topic.
        let short = questions[..9].to_vec();
        let reasons = bank_from_written(&draft, short).unwrap_err();
        assert!(reasons.contains("synthesis stage has 0"));
        let mut bad_key = questions.clone();
        bad_key[0].answer = "five".into();
        assert!(bank_from_written(&draft, bad_key)
            .unwrap_err()
            .contains("word for word"));
        let mut unknown = questions.clone();
        unknown[0].topic = "zz".into();
        assert!(bank_from_written(&draft, unknown)
            .unwrap_err()
            .contains("not in the course"));
        // A fourth question for a stage is set aside, not a failure.
        let mut extra = questions.clone();
        extra.push(written("a", "one"));
        assert_eq!(
            bank_from_written(&draft, extra).unwrap().questions.len(),
            12
        );
    }

    #[test]
    fn the_drafting_prompt_carries_the_brief_and_the_shape() {
        let brief = CourseBrief {
            title: "Rust for CLI tools".into(),
            outcome: "Build and ship a small tool".into(),
            background: "Python".into(),
            trusted_hosts: vec!["doc.rust-lang.org".into()],
            ..Default::default()
        };
        let prompt = draft_prompt(&brief);
        assert!(prompt.contains("Build and ship a small tool"));
        assert!(prompt.contains("doc.rust-lang.org"));
        assert!(prompt.contains("\"entry_points\""));
        assert!(prompt.contains("Between 12 and 36 topics"));
        let draft = with_id(CourseDraft::default(), "custom-rust", &brief);
        assert_eq!(draft.id, "custom-rust");
        assert_eq!(draft.label, "Rust for CLI tools");
        assert_eq!(draft.source_hosts, vec!["doc.rust-lang.org"]);
        assert_eq!(stage_labels(&draft).len(), 4);
    }
}
