//! Ownership pattern — an activist opinion (equity axis).
//!
//! Scores the share of land area held in:
//! - Community land trust (CLT)
//! - Cooperative
//! - Commons / public
//! - vs. private
//!
//! This is one of the non-substitutable equity guards from the spec.
//! A neighborhood proposal can score perfectly on every Alexander property
//! and still be evaluated *worse* than alternatives if its ownership pattern
//! enables displacement.
//!
//! Source: this opinion's politics come from Jason and the Eastside Commons spec.
//! Not Alexander, not Salingaros. Honest about whose voice this is.

use street_smarts_core::nir::{Neighborhood, Ownership};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};

pub struct OwnershipPattern;

impl Opinion for OwnershipPattern {
    fn name(&self) -> &'static str { "ownership_pattern" }
    fn family(&self) -> OpinionFamily { OpinionFamily::Activist }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "edelman_2026_eastside_commons".into(),
            display: "Jason Edelman + Eastside Commons coalition framework (2026)".into(),
            url: Some("https://jason-edelman.org/eastside-commons/".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) { (0.0, 1.0) }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let mut total = 0.0_f64;
        let mut commons_area = 0.0_f64; // CLT + Coop + Commons + Public
        let mut unknown_area = 0.0_f64;
        let mut tagged_count = 0;
        let mut clt_count = 0;
        let mut coop_count = 0;
        let mut public_count = 0;
        let mut eda_area = 0.0_f64;

        for p in &n.parcels {
            let a = if p.area_acres > 0.0 {
                p.area_acres * 4046.86
            } else {
                p.polygon.area_m2()
            };
            if a <= 0.0 { continue; }
            total += a;

            // EDA parcels in the EC data are CLT/civic by intent — bucket them.
            // The `spec` code distinguishes (CLT_* vs CIVIC_* vs MAIN_ST_*).
            if p.is_eda {
                eda_area += a;
                let spec = p.spec.as_deref().unwrap_or("");
                if spec.starts_with("CLT_") {
                    commons_area += a;
                    clt_count += 1;
                    tagged_count += 1;
                } else if spec.starts_with("CIVIC_") || spec.starts_with("MAIN_ST_") || spec.starts_with("HOUSING_") || spec.starts_with("SPONGE_") {
                    commons_area += a;
                    public_count += 1;
                    tagged_count += 1;
                } else if spec.starts_with("MALL_") {
                    // Mall core in the EC proposal is reused as civic/commercial — treat as commons.
                    commons_area += a;
                    public_count += 1;
                    tagged_count += 1;
                } else {
                    // Other EDA-tagged: count as commons-aspirational
                    commons_area += a;
                    tagged_count += 1;
                }
                continue;
            }

            match p.ownership {
                Some(Ownership::Clt) => { commons_area += a; clt_count += 1; tagged_count += 1; }
                Some(Ownership::Cooperative) => { commons_area += a; coop_count += 1; tagged_count += 1; }
                Some(Ownership::Commons) | Some(Ownership::Public) => {
                    commons_area += a; public_count += 1; tagged_count += 1;
                }
                Some(Ownership::Mixed) => { commons_area += a * 0.5; tagged_count += 1; }
                Some(Ownership::Private) => { tagged_count += 1; }
                Some(Ownership::Unknown) | None => { unknown_area += a; }
            }
        }

        if total <= 0.0 {
            return OpinionOutput::NoView {
                reason: "No parcels with positive area in this neighborhood.".into(),
                runtime_ms: 0,
            };
        }

        let known = total - unknown_area;
        if known <= 0.0 {
            return OpinionOutput::NoView {
                reason: "All parcels have unknown ownership; this opinion needs at least some tagged data.".into(),
                runtime_ms: 0,
            };
        }

        let commons_share = commons_area / known;

        // Score on the share of land in commons / CLT / cooperative / public:
        //   0.0    → 0.0 (fully private)
        //   0.25   → 0.4
        //   0.50   → 0.75
        //   ≥0.75  → 1.0
        let value = if commons_share >= 0.75 {
            1.0
        } else if commons_share >= 0.5 {
            0.75 + (commons_share - 0.5) * 1.0
        } else if commons_share >= 0.25 {
            0.4 + (commons_share - 0.25) * 1.4
        } else {
            commons_share * 1.6
        }
        .clamp(0.0, 1.0);

        let unknown_pct = (unknown_area / total) * 100.0;

        let summary = format!(
            "Of {} parcels tagged with ownership ({:.1}% of area), {:.0}% is in commons / CLT / cooperative / public hands ({} CLT, {} cooperative, {} civic/public). {:.1}% of total area is untagged.",
            tagged_count,
            (known / total) * 100.0,
            commons_share * 100.0,
            clt_count,
            coop_count,
            public_count,
            unknown_pct,
        );

        let mut caveats = vec![
            "This opinion treats EDA-tagged parcels (`is_eda: true`) as commons-aspirational — \
             that's an interpretation of the Eastside Commons proposal's intent, not a fact of ownership today.".into(),
            "Sees parcel-level ownership only. Does NOT see actual deed transfers, displacement events, \
             rent burden, or who lives there now.".into(),
        ];
        if unknown_pct > 30.0 {
            caveats.push(format!(
                "{:.0}% of land area is untagged; the share above is computed over what is tagged. \
                 This number could move significantly with better data.",
                unknown_pct
            ));
        }
        let _ = eda_area; // Reserved for a future EDA-share output.

        OpinionOutput::Value {
            value,
            method_summary: summary,
            caveats,
            contributing_features: vec![],
            runtime_ms: 0,
        }
    }
}
