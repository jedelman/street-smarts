//! P9 Scattered Work -- every floor gets a use, and work is scattered
//! through the homes rather than gathered apart from them.
//!
//! From Alexander, *A Pattern Language*, Pattern 9 (p. 51), via
//! patternlanguage.cc/Patterns/Scattered-Work-(9):
//! > **Problem:** The artificial separation of houses and work creates
//! > intolerable rifts in people's inner lives.
//! > **Solution:** Use zoning laws, neighborhood planning, tax incentives,
//! > and any other means available to scatter workplaces throughout the
//! > city. Prohibit large concentrations of work without family life around
//! > them. Prohibit large concentrations of family life without workplaces
//! > around them.
//!
//! # What this generator does
//!
//! Writes `Building.floor_uses`, one entry per floor, ground floor first:
//! - **Ground floors:** "play" where P87 put shop doors (shops, cafes,
//!   things to do), "work" where it didn't (a building with no wall on a
//!   street or square gets workshops at grade, not blank homes).
//! - **Upper floors:** "live", except that work is added floor by floor
//!   until it reaches `work_share` of all the floor area: every building's
//!   first upper floor before any building's second, and so on, in a
//!   scattered order across the site. That order is P9's two prohibitions
//!   built in: work spreads as a thin layer through every building before
//!   it thickens anywhere, so no part of the site gets work without homes
//!   around it or homes without work. Every building of three floors or
//!   more keeps its top floor as homes.
//!
//! The default `work_share`, 0.30, is inside the range Cushman & Wakefield
//! and Places Platform found works for walkable urban places (their
//! "Reimagining Cities: Disrupting the Urban Doom Loop", optimal work share
//! averaging 42%, with a 90% band of 23.8% to 61.7% of floor area); it's
//! toward the low end because this site also has to be a neighborhood.
//!
//! # Caveats
//!
//! - Uses are per floor, not per room or per unit; a floor is one use.
//! - What kind of work (offices, studios, clinics, workshops) isn't chosen
//!   here; nor whether a ground floor's shops are shops or services.
//! - P9 is a town-scale pattern applied here to one site; the scatter is
//!   checked within 100 m (`scatter_radius_m`), Alexander gives no radius.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Building, Neighborhood, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

/// `floor_uses` values.
pub const LIVE: &str = "live";
pub const WORK: &str = "work";
pub const PLAY: &str = "play";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P9Params {
    /// Share of all floor area given to work (ground floors included).
    pub work_share: f64,
    /// Radius within which every building should find both homes and work.
    pub scatter_radius_m: f64,
}

impl Parameters for P9Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "work_share",
                "Share of all floor area given to work (C&W's walkable-place band: 0.24 to 0.62).",
                0.0, 0.8, 0.30,
            ),
            ParamSpec::float(
                "scatter_radius_m",
                "Radius within which every building should find both homes and work (P9's check).",
                30.0, 400.0, 100.0,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self { work_share: 0.30, scatter_radius_m: 100.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.work_share, self.scatter_radius_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.first(), v.first()) { p.work_share = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.scatter_radius_m = s.clamp(*x); }
        p
    }
}

pub struct P9ScatteredWork;

/// A building's centroid in local metres (equirectangular about `lat0`).
fn centre_m(b: &Building, lat0: f64) -> (f64, f64) {
    let c = b.polygon.centroid();
    (c.lng * 111_320.0 * lat0.to_radians().cos(), c.lat * 110_540.0)
}

/// Bit-interleaves two 16-bit grid coordinates: sorting by this visits a
/// site in a Z-order curve, so consecutive picks land far apart.
fn z_order(x: u32, y: u32) -> u64 {
    let spread = |mut v: u64| {
        v &= 0xffff;
        v = (v | (v << 8)) & 0x00ff_00ff;
        v = (v | (v << 4)) & 0x0f0f_0f0f;
        v = (v | (v << 2)) & 0x3333_3333;
        (v | (v << 1)) & 0x5555_5555
    };
    spread(x as u64) | (spread(y as u64) << 1)
}

impl PatternOperator for P9ScatteredWork {
    type Params = P9Params;

    fn name(&self) -> &'static str {
        "p9_scattered_work"
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p9".into(),
            display: "Alexander et al., A Pattern Language, Pattern 9 (Scattered Work)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Scattered-Work-(9)".into()),
        }
    }

    fn description(&self) -> &'static str {
        "Give every floor a use, and scatter work through the homes: no work without homes around it, no homes without work."
    }

    fn apply(&self, nbhd: &Neighborhood, _parcel_id: &str, params: &Self::Params, seed: u64) -> Result<Subdivision, String> {
        let buildings: Vec<&Building> = nbhd.buildings.iter().filter(|b| b.floors.unwrap_or(0) >= 1).collect();
        if buildings.is_empty() {
            return Err("p9_scattered_work: no buildings with floors to give uses to.".into());
        }
        if buildings.iter().any(|b| !b.floor_uses.is_empty()) {
            return Err("p9_scattered_work: floors already have uses.".into());
        }
        let lat0 = buildings[0].polygon.centroid().lat;
        let area: Vec<f64> = buildings.iter().map(|b| b.polygon.area_m2()).collect();
        let floors: Vec<usize> = buildings.iter().map(|b| b.floors.unwrap_or(1) as usize).collect();
        let total: f64 = area.iter().zip(&floors).map(|(a, f)| a * *f as f64).sum();

        // Ground floors: shops where P87 opened shop doors, workshops where it didn't.
        let mut uses: Vec<Vec<&str>> = buildings
            .iter()
            .zip(&floors)
            .map(|(b, f)| {
                let shops = b.openings.iter().any(|o| o.kind == OpeningKind::ShopDoor);
                let mut u = vec![if shops { PLAY } else { WORK }];
                u.extend(std::iter::repeat_n(LIVE, f - 1));
                u
            })
            .collect();
        let mut work: f64 = uses.iter().zip(&area).filter(|(u, _)| u[0] == WORK).map(|(_, a)| a).sum();
        let target = params.work_share * total;

        // Upper floors: a layer of work through every building before any building gets a second.
        let pts: Vec<(f64, f64)> = buildings.iter().map(|b| centre_m(b, lat0)).collect();
        let (minx, miny) = pts.iter().fold((f64::MAX, f64::MAX), |m, p| (m.0.min(p.0), m.1.min(p.1)));
        let mut order: Vec<usize> = (0..buildings.len()).collect();
        order.sort_by_key(|&i| z_order(((pts[i].0 - minx) / 10.0) as u32, ((pts[i].1 - miny) / 10.0) as u32));
        // Alternate halves of the curve so neighbours on it don't follow each other.
        let (a, b) = order.split_at(order.len().div_ceil(2));
        let order: Vec<usize> = a.iter().zip(b.iter().chain(std::iter::repeat(&usize::MAX))).flat_map(|(x, y)| [*x, *y]).filter(|i| *i != usize::MAX).collect();
        let max_floors = *floors.iter().max().unwrap_or(&1);
        'layers: for level in 1..max_floors {
            for &i in &order {
                if work >= target {
                    break 'layers;
                }
                let f = floors[i];
                // Floor `level` exists, and (in a building of 3+ floors) isn't the top floor.
                if level >= f || (f >= 3 && level == f - 1) {
                    continue;
                }
                uses[i][level] = WORK;
                work += area[i];
            }
        }

        // P9's check: within the radius, does every building find both homes and work?
        let has = |u: &[&str], x: &str| u.contains(&x);
        let mixed = (0..buildings.len())
            .filter(|&i| {
                let near: Vec<usize> = (0..buildings.len())
                    .filter(|&j| ((pts[i].0 - pts[j].0).powi(2) + (pts[i].1 - pts[j].1).powi(2)).sqrt() <= params.scatter_radius_m)
                    .collect();
                near.iter().any(|&j| has(&uses[j], LIVE)) && near.iter().any(|&j| has(&uses[j], WORK))
            })
            .count();
        let share = |x: &str| uses.iter().zip(&area).map(|(u, a)| u.iter().filter(|v| **v == x).count() as f64 * a).sum::<f64>() / total;
        let (live, workf, play) = (share(LIVE), share(WORK), share(PLAY));

        let mut new_buildings = Vec::new();
        let mut replaced_building_ids = Vec::new();
        for (b, u) in buildings.iter().zip(&uses) {
            let mut nb = (*b).clone();
            nb.floor_uses = u.iter().map(|s| s.to_string()).collect();
            replaced_building_ids.push(b.id.clone());
            new_buildings.push(nb);
        }
        let n_work_ground = uses.iter().filter(|u| u[0] == WORK).count();
        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Every floor gets a use: {:.0}% homes, {:.0}% work, {:.0}% shops and things to do, with work scattered through every part of the site.",
                live * 100.0, workf * 100.0, play * 100.0
            ),
            steps: vec![
                format!(
                    "Ground floors: {} with shop doors are shops and things to do; {} with none are workshops.",
                    buildings.len() - n_work_ground, n_work_ground
                ),
                format!(
                    "Upper floors: homes, with work added a floor at a time across all {} buildings (first upper floors first) until work reached {:.0}% of {:.0} m2.",
                    buildings.len(), workf * 100.0, total
                ),
                format!(
                    "{mixed} of {} buildings have both homes and work within {:.0} m.",
                    buildings.len(), params.scatter_radius_m
                ),
            ],
            caveats: vec![
                "A floor is one use; rooms and units aren't divided between uses.".into(),
                "What kind of work, and whether a ground floor holds shops or services, isn't chosen here.".into(),
                "The 30% default is from Cushman & Wakefield's walkable-place band (23.8% to 61.7%), not from Alexander, who gives no share.".into(),
            ],
            seed,
            params: params.as_map(),
        };
        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings,
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids,
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
            new_fields: vec![],
            new_trees: vec![],
            new_ground_cover: vec![],
            new_transit: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subdivision::apply_subdivision;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Opening};

    fn building(id: &str, x_m: f64, y_m: f64, floors: u32, shops: bool) -> Building {
        let d = 20.0 / 111_320.0;
        let (x, y) = (x_m / 111_320.0, y_m / 110_540.0);
        let ring = vec![LngLat::new(x, y), LngLat::new(x + d, y), LngLat::new(x + d, y + d), LngLat::new(x, y + d), LngLat::new(x, y)];
        let openings = if shops {
            vec![Opening { kind: OpeningKind::ShopDoor, ring_index: 0, on_hole: false, t: 0.5, width_m: 1.0, sill_height_m: 0.0, head_height_m: 2.4, floor: 0 }]
        } else {
            vec![]
        };
        Building {
            id: id.into(), polygon: Polygon::from_ring(ring), height_m: Some(floors as f64 * 3.2), typology: None, year_built: None,
            parcel_id: None, floors: Some(floors), openings, interior_cells: vec![], wall_thickness_m: None, roof: None,
            roof_segments: vec![], canopies: vec![], wall_niches: vec![], floor_uses: vec![],
        }
    }

    fn site(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "t".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01], parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta { source: "synthetic".into(), fetched_at: "t".into(), license: "t".into(), layer_provenance: Default::default(), label: "P9 unit fixture".into() },
            pattern_fields: vec![], trees: vec![], ground_cover: vec![], transit: vec![],
        }
    }

    fn grid(n: usize, floors: u32) -> Neighborhood {
        site((0..n * n).map(|k| building(&format!("B{k}"), (k % n) as f64 * 40.0, (k / n) as f64 * 40.0, floors, k % 5 != 0)).collect())
    }

    #[test]
    fn every_floor_gets_a_use_and_ground_floors_follow_the_shop_doors() {
        let n = grid(4, 5);
        let out = apply_subdivision(&n, &P9ScatteredWork.apply(&n, "*", &P9Params::defaults(), 1).unwrap());
        for (b, orig) in out.buildings.iter().zip(&n.buildings) {
            assert_eq!(b.floor_uses.len(), 5, "{}", b.id);
            let shops = orig.openings.iter().any(|o| o.kind == OpeningKind::ShopDoor);
            assert_eq!(b.floor_uses[0], if shops { PLAY } else { WORK });
            assert_eq!(b.floor_uses[4], LIVE, "the top floor stays homes");
        }
    }

    #[test]
    fn work_reaches_its_share_as_a_thin_layer_before_it_thickens() {
        let n = grid(5, 6);
        let p = P9Params { work_share: 0.3, ..P9Params::defaults() };
        let out = apply_subdivision(&n, &P9ScatteredWork.apply(&n, "*", &p, 1).unwrap());
        let floors: usize = out.buildings.iter().map(|b| b.floor_uses.len()).sum();
        let work: usize = out.buildings.iter().map(|b| b.floor_uses.iter().filter(|u| *u == WORK).count()).sum();
        let share = work as f64 / floors as f64;
        assert!((0.3..0.36).contains(&share), "work share {share}");
        // Nobody gets a second upper work floor while another building still has none.
        let upper = |b: &Building| b.floor_uses[1..].iter().filter(|u| *u == WORK).count();
        let (lo, hi) = out.buildings.iter().fold((usize::MAX, 0), |(l, h), b| (l.min(upper(b)), h.max(upper(b))));
        assert!(hi - lo <= 1, "work thickened unevenly: {lo}..{hi}");
    }

    #[test]
    fn no_buildings_or_a_second_run_is_an_error() {
        assert!(P9ScatteredWork.apply(&site(vec![]), "*", &P9Params::defaults(), 1).is_err());
        let n = grid(2, 4);
        let once = apply_subdivision(&n, &P9ScatteredWork.apply(&n, "*", &P9Params::defaults(), 1).unwrap());
        assert!(P9ScatteredWork.apply(&once, "*", &P9Params::defaults(), 1).is_err());
    }
}
