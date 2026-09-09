//! Compatibility helpers for engineering course selection. The catalog owns
//! the subject inventory and teaching profiles, including System Design.

use crate::db::{DbError, Result};
pub const LEGACY_FOCUS: &str = "system-design";
pub use crate::catalog::ENGINEERING_IDS as SELECTABLE;

pub fn label(focus: &str) -> &str {
    crate::catalog::course(focus)
        .map(|course| course.label)
        .unwrap_or(focus)
}

pub fn context(focus: &str) -> &str {
    crate::catalog::course(focus)
        .map(|course| course.context)
        .unwrap_or("First-principles explanations supported by observable practice.")
}

// Legacy DTO/prompt name; this is a bounded project outcome, not a promise
// that an entire course can be mastered in thirty days.
pub fn month_outcome(focus: &str) -> &str {
    crate::catalog::course(focus)
        .map(|course| course.outcome)
        .unwrap_or("Complete a practical artifact and explain the evidence behind its design.")
}

pub fn is_selectable(focus: &str) -> bool {
    SELECTABLE.contains(&focus)
}

pub fn validate_selectable(focus: &str) -> Result<()> {
    if is_selectable(focus) {
        Ok(())
    } else {
        Err(DbError::InvalidFocus(focus.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_selectable_track_has_a_concrete_project_outcome() {
        for focus in SELECTABLE {
            assert!(
                month_outcome(focus).len() > 180,
                "{focus} has no concrete outcome"
            );
            assert_eq!(
                crate::catalog::course(focus).unwrap().kind,
                crate::catalog::SubjectKind::Engineering
            );
        }
    }
}
