//! The tutor's part in a learner's own class: drafting the curriculum from
//! the brief, reading a draft back, and the source check that fetches
//! every primary source. Each call is a bare-JSON exchange with the
//! configured runner through the generator's typed path, announced in the
//! execution feed under its own run so the Logs page shows it.

use crate::domain::custom::{
    self, CourseBrief, CourseDraft, DraftIssue, ReviewFinding, SourceCheck, MAX_TOPICS, MIN_TOPICS,
    STAGES,
};
use crate::generator::{GenError, Generator, Result};
use serde::Deserialize;
use std::time::Duration;

/// How long a drafting call may take. A whole curriculum is a long answer.
const DRAFT_TIMEOUT: Duration = Duration::from_secs(420);
const REVIEW_TIMEOUT: Duration = Duration::from_secs(240);

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
