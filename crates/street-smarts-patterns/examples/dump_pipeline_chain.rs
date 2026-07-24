//! Prints `language_graph::render_arrow_chain()` -- nothing else -- so a
//! build step can capture clean stdout and substitute it into a static
//! caption (`public/index.html`'s own "run through the full corrected
//! pipeline (...)" text) instead of a hand-typed stage list that silently
//! stops matching the real pipeline the moment a stage is added, removed,
//! or reordered. See `scripts/vibe-render.sh` for the real substitution
//! step, and `language_graph.rs`'s own `render_arrow_chain` doc for why
//! this exists.
//!
//! Usage:
//!   cargo run -p street-smarts-patterns --example dump_pipeline_chain

use street_smarts_patterns::language_graph::render_arrow_chain;

fn main() {
    print!("{}", render_arrow_chain());
}
