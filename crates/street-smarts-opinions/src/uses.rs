//! What a parcel is used for: its own `use_category` tag, or the floors of
//! the buildings standing on it (`Building.floor_uses`, set by the pattern
//! pipeline's P9 Scattered Work). A parcel whose building has shops or work
//! on its ground floor is commercial; one with any floor of homes is
//! residential; a mixed building's parcel is both, which is what a ground
//! floor of shops under homes is.
use std::collections::HashSet;
use street_smarts_core::nir::{Neighborhood, Parcel};

pub struct Uses {
    commercial: HashSet<String>,
    residential: HashSet<String>,
}

impl Uses {
    pub fn of(n: &Neighborhood) -> Self {
        let mut commercial = HashSet::new();
        let mut residential = HashSet::new();
        for b in &n.buildings {
            let Some(pid) = &b.parcel_id else { continue };
            if matches!(b.floor_uses.first().map(String::as_str), Some("play") | Some("work")) {
                commercial.insert(pid.clone());
            }
            if b.floor_uses.iter().any(|u| u == "live") {
                residential.insert(pid.clone());
            }
        }
        Self { commercial, residential }
    }

    /// Tagged "commercial", or a building on it has shops or work at street level.
    pub fn is_commercial(&self, p: &Parcel) -> bool {
        p.use_category.as_deref() == Some("commercial") || self.commercial.contains(&p.id)
    }

    /// Tagged "residential", or a building on it has a floor of homes.
    pub fn is_residential(&self, p: &Parcel) -> bool {
        p.use_category.as_deref() == Some("residential") || self.residential.contains(&p.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, NeighborhoodMeta};

    fn parcel(id: &str, tag: Option<&str>) -> Parcel {
        Parcel { id: id.into(), polygon: Polygon::from_ring(vec![]), area_acres: 0.0, use_category: tag.map(String::from), ownership: None, is_eda: false, spec: None, density_tier: None, target_stories: None }
    }
    fn building(parcel: &str, uses: &[&str]) -> Building {
        Building {
            id: format!("{parcel}_b"), polygon: Polygon::from_ring(vec![]), height_m: None, typology: None, year_built: None,
            parcel_id: Some(parcel.into()), floors: Some(uses.len() as u32), openings: vec![], interior_cells: vec![], wall_thickness_m: None,
            roof: None, roof_segments: vec![], canopies: vec![], wall_niches: vec![], floor_uses: uses.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn floors_make_a_parcel_commercial_residential_or_both() {
        let n = Neighborhood {
            id: "t".into(), bbox_wgs84: [0.0; 4],
            parcels: vec![parcel("mixed", Some("p95_pad_with_building")), parcel("homes", None), parcel("tagged", Some("commercial")), parcel("bare", None)],
            buildings: vec![building("mixed", &["play", "work", "live"]), building("homes", &["live", "live"])],
            streets: vec![], open_space: vec![], boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta { source: "t".into(), fetched_at: "t".into(), license: "t".into(), layer_provenance: Default::default(), label: "t".into() },
            pattern_fields: vec![], trees: vec![], ground_cover: vec![], transit: vec![],
        };
        let u = Uses::of(&n);
        let p = |i: usize| &n.parcels[i];
        assert!(u.is_commercial(p(0)) && u.is_residential(p(0)), "shops under homes are both");
        assert!(!u.is_commercial(p(1)) && u.is_residential(p(1)));
        assert!(u.is_commercial(p(2)) && !u.is_residential(p(2)), "a tag still counts");
        assert!(!u.is_commercial(p(3)) && !u.is_residential(p(3)), "no floors, no tag: unknown");
    }
}
