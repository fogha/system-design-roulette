//! The bundled catalog is authored once in seed/catalog.json. The build script
//! compiles it into static definitions; native IPC and the browser preview use
//! that same manifest rather than maintaining separate lists of subjects.

use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SubjectKind {
    Engineering,
    Language,
}

impl SubjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Engineering => "engineering",
            Self::Language => "language",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct EntryPoint {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CourseDefinition {
    pub id: &'static str,
    pub course_id: &'static str,
    pub kind: SubjectKind,
    pub label: &'static str,
    pub native_label: &'static str,
    pub short_code: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub version: &'static str,
    pub context: &'static str,
    pub outcome: &'static str,
    pub environment: &'static str,
    pub prompt_profile: &'static str,
    #[serde(skip_serializing)]
    pub prompt: &'static str,
    pub prerequisite_courses: &'static [&'static str],
    pub source_hosts: &'static [&'static str],
    pub reference_lessons: &'static [&'static str],
    #[serde(skip_serializing)]
    pub bundled_lessons: &'static [&'static str],
    pub capabilities: &'static [&'static str],
    pub entry_points: &'static [EntryPoint],
}

include!(concat!(env!("OUT_DIR"), "/catalog.rs"));

pub fn course(id: &str) -> Option<&'static CourseDefinition> {
    COURSES.iter().find(|course| course.id == id)
}

pub fn validate() -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut course_ids = HashSet::new();
    for item in COURSES {
        if item.id.is_empty()
            || !item
                .id
                .bytes()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-')
            || !ids.insert(item.id)
            || !course_ids.insert(item.course_id)
        {
            return Err(format!(
                "invalid or duplicate catalog identity: {}",
                item.id
            ));
        }
        if item.version.is_empty()
            || item.summary.is_empty()
            || item.outcome.is_empty()
            || item.source_hosts.is_empty()
        {
            return Err(format!("incomplete catalog definition: {}", item.id));
        }
        let mut entries = HashSet::new();
        if item.entry_points.is_empty()
            || item
                .entry_points
                .iter()
                .any(|entry| entry.label.is_empty() || !entries.insert(entry.id))
        {
            return Err(format!("invalid entry points: {}", item.id));
        }
        if !item.prompt.contains(&format!(
            "PROMPT PROFILE: {}.{}",
            item.prompt_profile, item.version
        )) {
            return Err(format!("prompt version mismatch: {}", item.id));
        }
        fn visit(id: &str, path: &mut HashSet<String>) -> Result<(), String> {
            if !path.insert(id.to_string()) {
                return Err(format!("course prerequisite cycle: {id}"));
            }
            let item = course(id).ok_or_else(|| format!("unknown prerequisite course: {id}"))?;
            for prerequisite in item.prerequisite_courses {
                visit(prerequisite, path)?;
            }
            path.remove(id);
            Ok(())
        }
        visit(item.id, &mut HashSet::new())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_the_nine_initial_courses_with_independent_shell_offerings() {
        validate().unwrap();
        assert_eq!(COURSES.len(), 9);
        assert_eq!(ENGINEERING_IDS.len(), 7);
        assert_eq!(
            course("bash-scripting").unwrap().prerequisite_courses,
            &["linux-bash"]
        );
        assert!(course("linux-bash")
            .unwrap()
            .prerequisite_courses
            .is_empty());
        assert!(course("system-design").is_some());
    }

    #[test]
    fn compiled_catalog_matches_the_authoring_manifest() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../seed/catalog.json")).unwrap();
        for (item, source) in COURSES.iter().zip(manifest["courses"].as_array().unwrap()) {
            let compiled = serde_json::to_value(item).unwrap();
            for (key, value) in source.as_object().unwrap() {
                if key != "prompt_path" {
                    assert_eq!(&compiled[key], value, "{} field {key}", item.id);
                }
            }
        }
    }
}
