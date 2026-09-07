//! Learning-track focus: JavaScript, TypeScript, frontend architecture, or
//! developer tooling.
//! Legacy `system-design` concepts remain in the DB for compatibility but
//! never enter the selectable pools.

use crate::db::{DbError, Result};

pub const LEGACY_FOCUS: &str = "system-design";

pub const SELECTABLE: &[&str] = &[
    "javascript",
    "typescript",
    "frontend-architecture",
    "developer-tooling",
];

/// User-facing label for prompt substitution.
pub fn label(focus: &str) -> &str {
    match focus {
        "javascript" => "JavaScript",
        "typescript" => "TypeScript",
        "frontend-architecture" => "frontend architecture",
        "developer-tooling" => "developer tooling",
        LEGACY_FOCUS => "system design",
        other => other,
    }
}

/// Pedagogy context injected into generation prompts. Every track is
/// framed around the same goal: the judgment of a world-class frontend
/// engineer who understands browsers deeply, debugs from first principles,
/// writes code other engineers want to read, and builds tools the rest of
/// the team actually adopts — not trivia disconnected from shipping UI.
pub fn context(focus: &str) -> &str {
    match focus {
        "javascript" => "Modern JavaScript as the browser's execution language and the web platform's control plane. Build first-principles judgment about the event loop and rendering opportunities, engine optimization and memory, DOM/CSSOM/layout/paint/composite, scheduling and responsiveness, Workers and transferable data, Streams and networking, storage and offline behavior, navigation and View Transitions, Speculation Rules, security boundaries, accessibility runtime behavior, and production debugging through DevTools and RUM. Prefer browser experiments and production failure investigations over Node-only trivia. Treat browser support as evidence: distinguish standardized and broadly available features from experimental or engine-specific behavior as of 2026.",
        "typescript" => "Modern TypeScript as an architectural constraint system for large frontend codebases. Go from structural typing, inference, variance, narrowing, generics, and compiler internals to real boundaries: component APIs, route and form contracts, async UI state machines, runtime validation of untrusted data, generated API clients, package exports, project references, typed linting, migration strategy, and keeping editor/build performance healthy in monorepos. Teach what the type system cannot prove, how unsoundness enters, and when simpler public types beat clever metaprogramming. Use current compiler and module-resolution behavior as of 2026 and name version-sensitive details explicitly.",
        "frontend-architecture" => "Staff-level frontend systems architecture for products and organizations that must scale. Cover domain-oriented code organization; component, route, package, and ownership boundaries; server/client and rendering decisions per route; state placement and data-cache consistency; BFF and API composition; design-system governance; accessibility as a cross-cutting contract; performance budgets and RUM; reliability, observability, security, testing, release, migration, and rollback; monorepos and micro-frontends; offline and realtime systems; and multi-team platform strategy. Compare React, Svelte, Vue, Angular, Astro, and standards-based approaches when the mechanism is framework-independent. Prefer a modular monolith until independent deployment or ownership pressure justifies more distribution, and make every recommendation explicit about 10x bottlenecks, failure modes, operability, DX, cost, and reversibility.",
        "developer-tooling" => "Building and operating the 2026 toolchain a world-class frontend organization relies on: parsers and AST transforms, LSP/DAP and source maps, Rust-powered bundlers and dev servers, HMR and incremental graphs, package-manager and monorepo orchestration, CSS and design-token compilers, typed linting and codemods, browser/component/E2E/visual testing, accessibility and performance gates, supply-chain controls, release automation, observability pipelines, and safe AI/MCP-assisted developer workflows. Every lesson should culminate in a practical tool or policy that a team could adopt, with plugin contracts, cache invalidation, determinism, diagnostics, security, and maintenance cost made explicit.",
        LEGACY_FOCUS => "distributed systems and staff-level system design judgment.",
        _ => "deep software engineering mechanics with first-principles explanations and hands-on experiments.",
    }
}

/// A concrete 30-day proof of skill for each track. Courses use this as a
/// cumulative spine so daily exercises build toward one coherent artifact
/// instead of becoming thirty disconnected demos.
pub fn month_outcome(focus: &str) -> &str {
    match focus {
        "javascript" => "By day 30, diagnose and improve a production browser experience from evidence: explain its scheduling and rendering behavior, instrument Core Web Vitals and long tasks, move appropriate work off the main thread, stream or cache data safely, apply browser security boundaries, and present before/after DevTools or RUM evidence in a compact performance case study.",
        "typescript" => "By day 30, design and publish a small type-safe frontend package: model async UI states without impossible combinations, expose readable component and route APIs, validate untrusted runtime data, generate or consume an external contract, ship correct declarations and package exports, and document the type-safety limits and migration path.",
        "frontend-architecture" => "By day 30, produce and defend a reference architecture for a realistic multi-team frontend: domain and route boundaries, rendering decisions per route, state and cache ownership, BFF contracts, design-system and accessibility governance, performance and reliability budgets, observability, test and release strategy, security boundaries, ADRs, and an incremental migration/rollback plan.",
        "developer-tooling" => "By day 30, ship a usable frontend engineering toolchain slice: a parser, codemod, lint rule, bundler/dev-server plugin, or quality gate with a stable CLI/plugin contract, deterministic tests, actionable diagnostics, caching/invalidation behavior, CI integration, security constraints, documentation, and evidence that another engineer can adopt it.",
        LEGACY_FOCUS => "By day 30, defend a production system design with explicit scale assumptions, failure modes, observability, and migration trade-offs.",
        _ => "By day 30, ship and defend one coherent production artifact that demonstrates the track's core mechanisms and trade-offs.",
    }
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
    use super::{month_outcome, SELECTABLE};

    #[test]
    fn every_selectable_track_has_a_concrete_month_outcome() {
        for focus in SELECTABLE {
            let outcome = month_outcome(focus);
            assert!(outcome.starts_with("By day 30"), "{focus}: {outcome}");
            assert!(outcome.len() > 180, "{focus} outcome is too vague");
        }
    }
}
