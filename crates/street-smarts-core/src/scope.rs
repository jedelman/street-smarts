//! `Scope`: a typed name for the string-tag conventions pattern operators
//! already use to find their targets (`Parcel.spec`, `Parcel.use_category`).
//!
//! Nothing about the underlying data changes -- `Scope::matches_parcel`
//! wraps the exact same string comparisons every operator already performs
//! by hand (`spec.as_deref().unwrap_or("").starts_with("BLOCK_")`,
//! `use_category.as_deref() == Some("p95_building_pad")`). What changes is
//! that the convention is named and centralized once, here, instead of
//! re-implemented per operator -- see PATTERN_LANGUAGE_SIMULATION.md §3.1.
//!
//! This is deliberately the cheap version. It does not require or wait for
//! an ECS migration (PRIMITIVES_SPEC.md §1) -- it names the *existing*
//! `Vec<Parcel>` + string-field model's own conventions, so it's usable
//! immediately and every existing operator's filter can be rewritten to
//! call `Neighborhood::select` without any change to the underlying data.

use crate::nir::{Neighborhood, Parcel};

/// What subset of a `Neighborhood`'s parcels an operator reads or writes.
/// Named after the string-tag conventions already in use, not new taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Every parcel, unfiltered -- site-scale operators (P37, PathNetwork).
    Site,
    /// Parcels whose `spec` starts with `"BLOCK_"` -- P37's own output.
    Block,
    /// Parcels whose `spec` starts with `"CIVIC"` -- reserved/non-buildable land.
    Civic,
    /// Parcels tagged with a specific `use_category` value -- e.g.
    /// `Scope::Category("p95_building_pad")`.
    Category(&'static str),
    /// Parcels matching any one of several `use_category` values -- e.g.
    /// the `p95_building_pad` / `p95_pad_with_building` pair several
    /// operators (P96, P107, P108, `block_grouping`, `building_shape`)
    /// currently re-check by hand.
    AnyCategory(&'static [&'static str]),
}

impl Scope {
    /// The building-pad scope every P96/P107/P108/`block_grouping`/
    /// `building_shape` filter above independently re-derives today.
    pub const BUILDING_PAD: Scope = Scope::AnyCategory(&["p95_building_pad", "p95_pad_with_building"]);

    pub fn matches_parcel(&self, p: &Parcel) -> bool {
        match self {
            Scope::Site => true,
            Scope::Block => p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"),
            Scope::Civic => p.spec.as_deref().unwrap_or("").starts_with("CIVIC"),
            Scope::Category(cat) => p.use_category.as_deref() == Some(*cat),
            Scope::AnyCategory(cats) => {
                p.use_category.as_deref().map(|c| cats.contains(&c)).unwrap_or(false)
            }
        }
    }
}

impl Neighborhood {
    /// Parcels matching `scope`, in the neighborhood's own order. Replaces
    /// the `nbhd.parcels.iter().filter(|p| <hand-written predicate>)` line
    /// every operator currently opens with.
    pub fn select<'a>(&'a self, scope: &'a Scope) -> impl Iterator<Item = &'a Parcel> + 'a {
        self.parcels.iter().filter(move |p| scope.matches_parcel(p))
    }

    /// IDs of parcels matching `scope` -- what most callers actually want
    /// (block IDs to loop over, pad IDs to pass to `apply_subdivision`).
    pub fn select_ids(&self, scope: &Scope) -> Vec<String> {
        self.select(scope).map(|p| p.id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Polygon;
    use crate::nir::{NeighborhoodMeta, Parcel};

    fn parcel(id: &str, spec: Option<&str>, use_category: Option<&str>) -> Parcel {
        Parcel {
            id: id.to_string(),
            polygon: Polygon::from_ring(vec![]),
            area_acres: 0.0,
            use_category: use_category.map(String::from),
            ownership: None,
            is_eda: false,
            spec: spec.map(String::from),
            density_tier: None,
            target_stories: None,
        }
    }

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.0, 0.0],
            parcels,
            buildings: vec![],
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            pattern_fields: vec![],
            metadata: NeighborhoodMeta {
                source: "test".into(),
                fetched_at: "".into(),
                license: "".into(),
                layer_provenance: Default::default(),
                label: "test".into(),
            },
        }
    }

    #[test]
    fn site_scope_matches_everything() {
        let n = nbhd(vec![parcel("a", None, None), parcel("b", Some("BLOCK_0"), None)]);
        assert_eq!(n.select_ids(&Scope::Site), vec!["a", "b"]);
    }

    #[test]
    fn block_scope_matches_only_block_prefixed_spec() {
        let n = nbhd(vec![
            parcel("a", Some("BLOCK_0"), None),
            parcel("b", Some("CIVIC_PARK"), None),
            parcel("c", None, None),
        ]);
        assert_eq!(n.select_ids(&Scope::Block), vec!["a"]);
    }

    #[test]
    fn building_pad_scope_matches_either_pad_category() {
        let n = nbhd(vec![
            parcel("a", None, Some("p95_building_pad")),
            parcel("b", None, Some("p95_pad_with_building")),
            parcel("c", None, Some("vacant")),
        ]);
        assert_eq!(n.select_ids(&Scope::BUILDING_PAD), vec!["a", "b"]);
    }
}
