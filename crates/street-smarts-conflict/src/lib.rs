//! # street-smarts-conflict
//!
//! Disagreement detection between evaluated opinions.
//!
//! Per the spec: disagreement is the PRIMARY product of this library.
//! The conflict engine doesn't resolve disagreements. It surfaces them.
//! It also surfaces *agreements* and *abstentions* (opinions that declined
//! to speak), because all three are signal.
//!
//! In v0.1 every opinion is in its own axis (Levels of Scale, Strong Centers,
//! Ownership Pattern) so direct cross-axis "disagreement" doesn't apply in
//! the strict Salingaros-prompt-vs-classical sense. What we surface instead:
//!
//! - The geometric chorus's split — agreement vs internal spread on the
//!   wholeness axes (how alive does this place look?)
//! - The geometric-vs-activist split — the equity guard's separate verdict
//!   (does this place protect the people in it?)
//! - Each opinion's abstentions ("I don't have a view here") with reasons
//!
//! In v0.2, when multiple geometric implementations per property and the
//! VLM family land, cross-axis disagreements become the central feature.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use street_smarts_core::opinion::OpinionFamily;
use street_smarts_opinions::registry::{group_by_family, EvaluatedOpinion};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisagreementReport {
    /// Every evaluated opinion, with full provenance and output.
    pub opinions: Vec<EvaluatedOpinion>,
    /// Summary of the geometric chorus.
    pub geometric_summary: ChorusSummary,
    /// Summary of the activist chorus.
    pub activist_summary: ChorusSummary,
    /// Top human-facing questions the conflict engine surfaces.
    pub questions_for_humans: Vec<HumanPrompt>,
    /// Opinions that declined to speak, with their reasons.
    pub abstentions: Vec<Abstention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChorusSummary {
    pub family: OpinionFamily,
    pub n_voices: usize,
    pub n_abstentions: usize,
    /// Mean of values from voices that spoke. None if all abstained.
    pub mean_value: Option<f64>,
    /// Spread (max − min) of values that spoke. None if <2 voices.
    pub spread: Option<f64>,
    /// One-line summary suitable for a header.
    pub headline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanPrompt {
    pub question: String,
    pub why_it_matters: String,
    pub related_opinions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Abstention {
    pub opinion_name: String,
    pub family: OpinionFamily,
    pub reason: String,
}

pub fn build_report(evaluated: Vec<EvaluatedOpinion>) -> DisagreementReport {
    let groups = group_by_family(&evaluated);

    let geometric = summarize_family(OpinionFamily::Geometric, groups.get(&OpinionFamily::Geometric));
    let activist = summarize_family(OpinionFamily::Activist, groups.get(&OpinionFamily::Activist));

    let mut abstentions = Vec::new();
    for ev in &evaluated {
        if let street_smarts_core::opinion::OpinionOutput::NoView { reason, .. } = &ev.output {
            abstentions.push(Abstention {
                opinion_name: ev.opinion.name.clone(),
                family: ev.opinion.family,
                reason: reason.clone(),
            });
        }
    }

    let questions = build_human_prompts(&evaluated, &geometric, &activist);

    DisagreementReport {
        opinions: evaluated,
        geometric_summary: geometric,
        activist_summary: activist,
        questions_for_humans: questions,
        abstentions,
    }
}

fn summarize_family(
    family: OpinionFamily,
    group: Option<&Vec<&EvaluatedOpinion>>,
) -> ChorusSummary {
    let group = match group {
        Some(g) => g,
        None => {
            return ChorusSummary {
                family,
                n_voices: 0,
                n_abstentions: 0,
                mean_value: None,
                spread: None,
                headline: format!("No {:?} opinions registered.", family),
            };
        }
    };

    let mut values = Vec::new();
    let mut abstentions = 0;
    for ev in group.iter() {
        match &ev.output {
            street_smarts_core::opinion::OpinionOutput::Value { value, .. } => values.push(*value),
            street_smarts_core::opinion::OpinionOutput::NoView { .. } => abstentions += 1,
        }
    }

    let mean = if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    };
    let spread = if values.len() >= 2 {
        let mn = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        Some(mx - mn)
    } else {
        None
    };

    let headline = match (mean, spread) {
        (Some(m), Some(s)) if s > 0.3 => {
            format!("The {:?} chorus disagrees with itself: mean {:.2}, spread {:.2}. That's the conversation.", family, m, s)
        }
        (Some(m), _) if values.len() == group.len() => {
            format!("The {:?} chorus is in rough agreement around {:.2}.", family, m)
        }
        (Some(m), _) => {
            format!("The {:?} chorus has {} voice(s) speaking around {:.2} and {} abstention(s).", family, values.len(), m, abstentions)
        }
        (None, _) => {
            format!("All {} {:?} opinion(s) abstained.", abstentions, family)
        }
    };

    ChorusSummary {
        family,
        n_voices: values.len(),
        n_abstentions: abstentions,
        mean_value: mean,
        spread,
        headline,
    }
}

fn build_human_prompts(
    evaluated: &[EvaluatedOpinion],
    geometric: &ChorusSummary,
    activist: &ChorusSummary,
) -> Vec<HumanPrompt> {
    let mut prompts = Vec::new();

    // Geometric vs activist split is the canonical disagreement to surface.
    if let (Some(g), Some(a)) = (geometric.mean_value, activist.mean_value) {
        let delta = (g - a).abs();
        if delta > 0.15 {
            let prompt = if g > a {
                HumanPrompt {
                    question: format!(
                        "The geometric chorus says this place is alive (≈ {:.2}). The activist chorus says it's not protecting the people in it (≈ {:.2}). Which matters more for what you're trying to do?",
                        g, a
                    ),
                    why_it_matters: "Greenwich Village 2024 scores high on geometry and low on equity. Alexandrian beauty without protection from displacement is a movie set.".into(),
                    related_opinions: vec!["geometric_chorus".into(), "activist_chorus".into()],
                }
            } else {
                HumanPrompt {
                    question: format!(
                        "The activist chorus likes this place's ownership pattern ({:.2}). The geometric chorus is less convinced about its form ({:.2}). What's the gap?",
                        a, g
                    ),
                    why_it_matters: "Strong commons ownership can survive weak geometry — and weak geometry can be improved. The reverse is much harder.".into(),
                    related_opinions: vec!["geometric_chorus".into(), "activist_chorus".into()],
                }
            };
            prompts.push(prompt);
        }
    }

    // Direct cross-opinion disagreements on the geometric axis (Levels-of-Scale vs Strong-Centers).
    let levels = evaluated.iter().find(|e| e.opinion.name == "levels_of_scale");
    let centers = evaluated.iter().find(|e| e.opinion.name == "strong_centers");
    if let (Some(l), Some(c)) = (levels, centers) {
        if let (Some(lv), Some(cv)) = (l.output.value(), c.output.value()) {
            if (lv - cv).abs() > 0.3 {
                prompts.push(HumanPrompt {
                    question: format!(
                        "Levels of Scale says {:.2} but Strong Centers says {:.2}. The place may have variety without focus (or focus without variety). Which describes it from where you live?",
                        lv, cv
                    ),
                    why_it_matters: "A neighborhood with many sizes of building but no centers feels scattered. A neighborhood with one strong center but no scale variety feels monotonous. Alexander wanted both.".into(),
                    related_opinions: vec!["levels_of_scale".into(), "strong_centers".into()],
                });
            }
        } else if l.output.value().is_some() && c.output.value().is_none() {
            prompts.push(HumanPrompt {
                question: "Levels of Scale has a view here; Strong Centers couldn't see enough data to speak. Where would you say the centers are?".into(),
                why_it_matters: "Centers are often invisible to an algorithm. Markets, gathering corners, named landmarks — only humans know where these actually are.".into(),
                related_opinions: vec!["strong_centers".into()],
            });
        }
    }

    if prompts.is_empty() {
        prompts.push(HumanPrompt {
            question: "All voices roughly agree. What do you see that the algorithms might be missing?".into(),
            why_it_matters: "Agreement among algorithms is suspicious — they share assumptions. The interesting disagreement may be between the chorus and you.".into(),
            related_opinions: vec![],
        });
    }

    prompts
}
