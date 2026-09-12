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
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// The builder's calls in flight, by course id: `draft`, `review`, `sources`
/// or `bank`. The interface asks for it when it opens a class, so leaving
/// the page and coming back still shows what the tutor is doing.
#[derive(Default)]
pub struct BuilderJobs(pub Mutex<HashMap<String, String>>);

impl BuilderJobs {
    pub fn working(&self, id: &str) -> Option<String> {
        self.0.lock().unwrap().get(id).cloned()
    }

    /// Mark a job in flight until the guard drops, however the call ends.
    pub fn begin(&self, id: &str, kind: &str) -> JobGuard<'_> {
        self.0
            .lock()
            .unwrap()
            .insert(id.to_string(), kind.to_string());
        JobGuard {
            jobs: self,
            id: id.to_string(),
        }
    }
}

pub struct JobGuard<'a> {
    jobs: &'a BuilderJobs,
    id: String,
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        self.jobs.0.lock().unwrap().remove(&self.id);
    }
}

/// A long call says so while it runs. The CLI runners answer only when they
/// finish, and a whole curriculum takes minutes, so without this the feed
/// falls silent and looks stuck.
async fn with_heartbeat<T>(
    feed: &crate::execution_log::Feed,
    what: &str,
    call: impl std::future::Future<Output = T>,
) -> T {
    let started = std::time::Instant::now();
    let mut tick = tokio::time::interval(Duration::from_secs(45));
    tick.tick().await; // the first tick fires at once
    tokio::pin!(call);
    loop {
        tokio::select! {
            result = &mut call => return result,
            _ = tick.tick() => {
                feed.say(format!(
                    "class builder: still {what} ({}s); a whole course takes a few minutes",
                    started.elapsed().as_secs()
                ));
            }
        }
    }
}

/// How long a drafting call may take. A whole curriculum is a long answer.
const DRAFT_TIMEOUT: Duration = Duration::from_secs(420);
const REVIEW_TIMEOUT: Duration = Duration::from_secs(240);
const BANK_TIMEOUT: Duration = Duration::from_secs(360);
const FIX_TIMEOUT: Duration = Duration::from_secs(300);

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

/// What the learner already settled, for the review prompt: a dismissed
/// finding is not raised again; a fixed one only if the fix did not take.
fn settled_text(settled: &[ReviewFinding]) -> String {
    let lines: Vec<String> = settled
        .iter()
        .filter(|f| f.status != "open")
        .take(24)
        .map(|f| {
            format!(
                "- [{}{}] {}",
                f.status,
                if f.topic.is_empty() {
                    String::new()
                } else {
                    format!(", {}", f.topic)
                },
                f.message.trim()
            )
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "\n\nALREADY SETTLED by the learner in an earlier read. Do not raise a dismissed one \
         again; raise a fixed one only if the draft still shows the problem:\n{}",
        lines.join("\n")
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
        let (raw, source) = with_heartbeat(
            &scoped.feed,
            "drafting",
            scoped.run_exact_for::<CourseDraft>(
                &agent,
                &custom_bin,
                &prompt,
                false,
                DRAFT_TIMEOUT,
                &model,
            ),
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
            match with_heartbeat(
                &scoped.feed,
                "correcting the draft",
                scoped.run_exact_for::<CourseDraft>(
                    &agent,
                    &custom_bin,
                    &correction,
                    false,
                    DRAFT_TIMEOUT,
                    &model,
                ),
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
    /// `settled` are the findings the learner already fixed or dismissed;
    /// the tutor is told not to raise them again.
    pub async fn review_custom_course(
        &self,
        brief: &CourseBrief,
        draft: &CourseDraft,
        settled: &[ReviewFinding],
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
             At most twelve findings, highest severity first.{settled}\n\n\
             DRAFT:\n{draft}",
            outcome = brief.outcome.trim(),
            background = if brief.background.trim().is_empty() {
                "not stated"
            } else {
                brief.background.trim()
            },
            settled = settled_text(settled),
            draft = serde_json::to_string(draft)
                .map_err(|error| GenError::Parse(format!("could not serialize draft: {error}")))?
        );
        let (review, source) = with_heartbeat(
            &scoped.feed,
            "reviewing",
            scoped.run_exact_for::<Review>(
                &agent,
                &custom_bin,
                &prompt,
                false,
                REVIEW_TIMEOUT,
                &model,
            ),
        )
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

/// The change the tutor proposes for one finding: header fields to set,
/// topics to add or replace (by slug), topics to remove, and the order the
/// topics should take. The desk applies it; the tutor never rewrites the
/// whole course for one finding.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct DraftPatch {
    /// One sentence on what was changed, kept with the finding.
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub header: HeaderPatch,
    #[serde(default)]
    pub topics: Vec<crate::domain::custom::DraftTopic>,
    #[serde(default)]
    pub remove: Vec<String>,
    /// Every slug in the order the topics should take; missing slugs keep
    /// their place after the listed ones, unknown ones are ignored.
    #[serde(default)]
    pub order: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct HeaderPatch {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub native_label: Option<String>,
    #[serde(default)]
    pub short_code: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub source_hosts: Option<Vec<String>>,
    #[serde(default)]
    pub entry_points: Option<Vec<crate::catalog::OwnedEntryPoint>>,
}

fn patch_schema() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "note": "one sentence saying what you changed",
        "header": {"summary": "only the header fields you change: label, native_label, short_code, title, summary, context, outcome, environment, source_hosts, entry_points; leave the rest out"},
        "topics": [{"slug": "a topic to add, or an existing slug to replace in full", "title": "...", "category": "...", "prereqs": ["..."], "curriculum": {"phase": "foundations | mechanisms | production | synthesis | elective", "core": true, "learner_outcome": "...", "mechanisms": ["..."], "production_scenario": "...", "misconceptions": ["..."], "evidence": "...", "artifact": "...", "primary_sources": ["https://..."]}}],
        "remove": ["slugs of topics to remove"],
        "order": ["every slug in the order the topics should take, when the order changes; otherwise empty"]
    }))
    .unwrap()
}

/// Apply the tutor's change to the draft. A replaced topic keeps its
/// place; a new one goes after the last topic it builds on, else after the
/// last topic of its stage, else at the end; removed slugs leave every
/// prerequisite list. The result is normalized, not yet validated.
pub fn apply_patch(mut draft: CourseDraft, patch: DraftPatch) -> CourseDraft {
    let h = patch.header;
    let set = |field: &mut String, value: Option<String>| {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            *field = value;
        }
    };
    set(&mut draft.label, h.label);
    set(&mut draft.native_label, h.native_label);
    set(&mut draft.short_code, h.short_code);
    set(&mut draft.title, h.title);
    set(&mut draft.summary, h.summary);
    set(&mut draft.context, h.context);
    set(&mut draft.outcome, h.outcome);
    set(&mut draft.environment, h.environment);
    if let Some(hosts) = h.source_hosts.filter(|hosts| !hosts.is_empty()) {
        draft.source_hosts = hosts;
    }
    if let Some(entries) = h.entry_points.filter(|entries| !entries.is_empty()) {
        for entry in entries {
            if let Some(existing) = draft.entry_points.iter_mut().find(|e| e.id == entry.id) {
                if !entry.label.trim().is_empty() {
                    existing.label = entry.label;
                }
            }
        }
    }
    let removed: Vec<String> = patch
        .remove
        .iter()
        .map(|slug| custom::slugify(slug))
        .collect();
    draft.topics.retain(|topic| !removed.contains(&topic.slug));
    for mut topic in patch.topics {
        topic.slug = custom::slugify(if topic.slug.trim().is_empty() {
            &topic.title
        } else {
            &topic.slug
        });
        if topic.slug.is_empty() {
            continue;
        }
        if let Some(index) = draft.topics.iter().position(|t| t.slug == topic.slug) {
            draft.topics[index] = topic;
            continue;
        }
        let after_prereq = draft
            .topics
            .iter()
            .rposition(|t| topic.prereqs.iter().any(|p| custom::slugify(p) == t.slug));
        let after_stage = draft
            .topics
            .iter()
            .rposition(|t| t.curriculum.phase == topic.curriculum.phase);
        let at = after_prereq
            .or(after_stage)
            .map(|i| i + 1)
            .unwrap_or(draft.topics.len());
        draft.topics.insert(at, topic);
    }
    for topic in &mut draft.topics {
        topic
            .prereqs
            .retain(|p| !removed.contains(&custom::slugify(p)));
    }
    if !patch.order.is_empty() {
        let wanted: Vec<String> = patch
            .order
            .iter()
            .map(|slug| custom::slugify(slug))
            .collect();
        let mut ordered: Vec<crate::domain::custom::DraftTopic> = Vec::new();
        for slug in &wanted {
            if let Some(index) = draft.topics.iter().position(|t| &t.slug == slug) {
                ordered.push(draft.topics.remove(index));
            }
        }
        ordered.append(&mut draft.topics);
        draft.topics = ordered;
    }
    custom::normalize(draft)
}

fn fix_prompt(brief: &CourseBrief, draft: &CourseDraft, finding: &ReviewFinding) -> String {
    format!(
        "You reviewed this self-study curriculum and raised the finding below. Now make the \
         change. The learner wants to be able to: {outcome}.\n\n\
         FINDING ({severity}{topic}): {message}\n\
         PROPOSED FIX: {fix}\n\n\
         Return ONLY a JSON patch with exactly this shape, no markdown fences, no commentary. \
         Include only what changes: a replaced topic is given in full, a new topic in full with \
         its prerequisites among existing slugs, header fields only when they change. Every \
         topic keeps the shape of the ones in the draft: a learner outcome of at least eight \
         words, at least two named mechanisms, a production scenario, at least one \
         misconception, evidence, an artifact, and at least two primary-source URLs on the \
         course's hosts ({hosts}).\n{schema}\n\nDRAFT:\n{draft}",
        outcome = brief.outcome.trim(),
        severity = finding.severity,
        topic = if finding.topic.is_empty() {
            String::new()
        } else {
            format!(", topic {}", finding.topic)
        },
        message = finding.message.trim(),
        fix = if finding.fix.trim().is_empty() {
            "none given; decide the smallest change that settles the finding"
        } else {
            finding.fix.trim()
        },
        hosts = draft.source_hosts.join(", "),
        schema = patch_schema(),
        draft = serde_json::to_string(draft).unwrap_or_default(),
    )
}

impl Generator {
    /// Ask the tutor to make the change one finding asks for. The patch is
    /// applied here; one that leaves the draft worse against the validator
    /// goes back once with the reasons, and is dropped if still worse.
    pub async fn fix_custom_course_finding(
        &self,
        brief: &CourseBrief,
        draft: &CourseDraft,
        finding: &ReviewFinding,
    ) -> Result<(CourseDraft, String, String)> {
        let scoped = self.scoped("course-fix");
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
            "class builder: asking {agent} ({model}) to settle a finding on \"{}\": {}",
            draft.label,
            finding.message.trim()
        ));
        let prompt = fix_prompt(brief, draft, finding);
        let before = custom::validate(draft).len();
        let (patch, source) = with_heartbeat(
            &scoped.feed,
            "changing the draft",
            scoped.run_exact_for::<DraftPatch>(
                &agent,
                &custom_bin,
                &prompt,
                false,
                FIX_TIMEOUT,
                &model,
            ),
        )
        .await?;
        let note = patch.note.trim().to_string();
        let changed = apply_patch(draft.clone(), patch);
        let issues = custom::validate(&changed);
        scoped.log(format!(
            "class builder: the change applied; {} issue(s) against the validator (was {before})",
            issues.len()
        ));
        if issues.len() <= before {
            return Ok((changed, note, source));
        }
        let correction = format!(
            "Your patch left the draft failing these deterministic checks:\n{}\n\n\
             Return ONLY a corrected patch in the same shape that settles the finding without \
             breaking them.\n\nORIGINAL_REQUEST:\n{prompt}",
            issues_text(&issues)
        );
        let (again, _) = with_heartbeat(
            &scoped.feed,
            "correcting the change",
            scoped.run_exact_for::<DraftPatch>(
                &agent,
                &custom_bin,
                &correction,
                false,
                FIX_TIMEOUT,
                &model,
            ),
        )
        .await?;
        let note = if again.note.trim().is_empty() {
            note
        } else {
            again.note.trim().to_string()
        };
        let changed = apply_patch(draft.clone(), again);
        let remaining = custom::validate(&changed);
        if remaining.len() > before {
            return Err(GenError::Quality(format!(
                "the tutor's change left the draft worse ({} issue(s) where there were {before}); nothing was applied",
                remaining.len()
            )));
        }
        Ok((changed, note, source))
    }
}

/// One question as the tutor writes it; the desk assigns ids and criteria.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WrittenQuestion {
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
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
                "- {} [{}]: {}; outcome: {}; sources: {}",
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
        "Write the placement check for a self-study course: exactly {per} four-choice \
         questions for EACH of the four stages (foundations, mechanisms, production, \
         synthesis), {total} in all. Every question names a core topic from the list below, \
         and the topic's stage is the question's stage: count them per stage before you \
         answer. Spread a stage's questions over its core topics; when a stage has fewer \
         core topics than {per}, write more than one question on the same topic rather than \
         fewer questions. A question tests whether the learner already has the topic's \
         outcome; it is short, concrete, answerable in under a minute, and its key is \
         verifiable from the topic's primary sources. The three wrong choices are plausible \
         mistakes, not jokes. Cite one of the topic's sources per question.\n\n\
         COURSE: {title}\nOUTCOME: {outcome}\nWHAT THE LEARNER ALREADY KNOWS: {background}\n\
         SOURCE HOSTS: {hosts}\n\nCORE TOPICS:\n{topics}\n\n\
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
        let (written, source) = with_heartbeat(
            &scoped.feed,
            "writing the question bank",
            scoped.run_exact_for::<WrittenBank>(
                &agent,
                &custom_bin,
                &prompt,
                false,
                BANK_TIMEOUT,
                &model,
            ),
        )
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
                accepted: false,
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
    fn a_patch_replaces_adds_removes_and_reorders_topics() {
        let draft = draft_with_topics();
        let mut added = draft.topics[1].clone();
        added.slug = "b2".into();
        added.title = "Topic b2".into();
        added.prereqs = vec!["b".into()];
        let mut replaced = draft.topics[0].clone();
        replaced.title = "Topic a, renamed".into();
        let patch = DraftPatch {
            note: "split b".into(),
            header: HeaderPatch {
                summary: Some("A new summary.".into()),
                label: Some("   ".into()),
                ..Default::default()
            },
            topics: vec![added, replaced],
            remove: vec!["d".into()],
            order: vec![],
        };
        let changed = apply_patch(draft.clone(), patch);
        let slugs: Vec<&str> = changed.topics.iter().map(|t| t.slug.as_str()).collect();
        assert_eq!(slugs, vec!["a", "b", "b2", "c"]);
        assert_eq!(changed.topics[0].title, "Topic a, renamed");
        assert_eq!(changed.summary, "A new summary.");
        assert_eq!(
            changed.label, "Rust",
            "a blank header value changes nothing"
        );
        // A removed topic leaves every prerequisite list; an order is honoured.
        let mut needs_d = draft.clone();
        needs_d.topics[2].prereqs = vec!["d".into()];
        let patch = DraftPatch {
            remove: vec!["d".into()],
            order: vec!["c".into(), "a".into()],
            ..Default::default()
        };
        let changed = apply_patch(needs_d, patch);
        let slugs: Vec<&str> = changed.topics.iter().map(|t| t.slug.as_str()).collect();
        assert_eq!(slugs, vec!["c", "a", "b"]);
        assert!(changed.topics[0].prereqs.is_empty());
        // A new topic without prerequisites lands after its stage.
        let mut fresh = draft_with_topics().topics[0].clone();
        fresh.slug = "a2".into();
        fresh.title = "Topic a2".into();
        let changed = apply_patch(
            draft_with_topics(),
            DraftPatch {
                topics: vec![fresh],
                ..Default::default()
            },
        );
        let slugs: Vec<&str> = changed.topics.iter().map(|t| t.slug.as_str()).collect();
        assert_eq!(slugs, vec!["a", "a2", "b", "c", "d"]);
        let prompt = fix_prompt(
            &CourseBrief::default(),
            &draft,
            &ReviewFinding {
                severity: "high".into(),
                topic: "b".into(),
                message: "b is two topics".into(),
                fix: String::new(),
                status: "open".into(),
                note: String::new(),
                carried: false,
            },
        );
        assert!(prompt.contains("FINDING (high, topic b): b is two topics"));
        assert!(prompt.contains("\"remove\""));
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
