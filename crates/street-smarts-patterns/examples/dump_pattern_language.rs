//! Prints `language_graph::LANGUAGE` as human-readable ordering
//! documentation -- the generated-docs half of
//! PATTERN_LANGUAGE_SIMULATION.md §4.1 (b): registry.rs's own doc comment
//! points here instead of duplicating this prose, so the explanation and
//! the `validate_order`-enforced rule can't drift apart.
//!
//! Usage:
//!   cargo run -p street-smarts-patterns --example dump_pattern_language

use street_smarts_patterns::language_graph::render_language_doc;

fn main() {
    print!("{}", render_language_doc());
}
