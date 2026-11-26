//! P34 Interchange -- a light rail station that is a center of the place,
//! continuous with its streets, ringed by homes and workplaces.
//!
//! From Alexander, *A Pattern Language*, Pattern 34 (p. 183), via
//! patternlanguage.cc/Patterns/Interchange-(34). Its solution: "At every
//! interchange in the web of transportation follow these principles:"
//! "Surround the interchange with workplaces and housing types which
//! specially need public transportation." "Keep the interior of the
//! interchange continuous with the exterior pedestrian network," keeping
//! parking to one side. "Keep the transfer distance between different modes
//! of transport down to 300 feet wherever possible, with an absolute maximum
//! of 600 feet."
//!
//! # Why here
//!
//! Hampton Roads Transit's Naval Station Norfolk Transit Corridor, Phase 1,
//! would extend the Tide about two miles from Newtown Road, past Sentara
//! Leigh Hospital, to an end-of-line station at the Military Circle site.
//! HRT paused it on August 25, 2023, waiting on where the city would put that
//! station (gohrt.com/2020/01/naval-station-norfolk-transit-corridor-project).
//! HRT gives no location on the site, so this places one.
//!
//! # What this generator does
//!
//! 1. The line comes from `approach` (default: the Tide's Newtown Road
//!    station, 36.8411 N, 76.1902 W, from its Wikipedia infobox). It enters
//!    the site's ring on a straight bearing from the site's middle.
//! 2. Candidate stations step inward from where the line enters, and sideways
//!    in parallel lines: an end-of-line platform `platform_length_m` long
//!    beside a stub of track that ends just past it. A candidate is kept when
//!    its track and platform stay clear of every building and its platform is
//!    within `max_transfer_m` (300 ft) of a street, with clear ground for a
//!    path to it: the pattern's transfer rule, read as "to the pedestrian
//!    network". Where no candidate is that close, the pattern's absolute
//!    maximum, `absolute_max_m` (600 ft), is tried instead.
//! 3. Of those, the one with the most buildings around it wins (the
//!    pattern's first principle), then the one with the least track inside
//!    the site.
//! 4. It adds the `TransitLine` (from a point `stub_m` outside the site,
//!    through the edge, to the bumper), its `TransitStation`, an
//!    `ActivityNode` of kind `Transit` on the platform, a square beside the
//!    platform where one fits, and a pedestrian path from the platform to
//!    the nearest street when it isn't already on one.
//!
//! # Caveats
//!
//! - The track is a straight sketch from the direction of Newtown Road, not
//!   HRT's alignment (which would pass Sentara Leigh), and the station's
//!   place is this generator's choice, not HRT's or the city's.
//! - No buses, so the transfer rule is measured to the nearest street.
//! - Platform length (60 m, about two coupled cars) and width are plausible
//!   figures, not HRT's.

use crate::p172_garden_growing_wild::site_ring;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{self, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{
    ActivityKind, ActivityNode, Neighborhood, OpenSpace, OpenSpaceKind, Street, TransitLine, TransitStation,
};
use street_smarts_core::opinion::SourceCitation;

/// Clear ground kept between the track's centre and any wall.
const TRACK_CLEARANCE_M: f64 = 4.0;
/// From the track's centre to the platform's near edge.
const PLATFORM_OFFSET_M: f64 = 1.8;
/// Track past the platform's end, to the bumper.
const OVERRUN_M: f64 = 8.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P34Params {
    pub approach_lng: f64,
    pub approach_lat: f64,
    pub platform_length_m: f64,
    pub platform_width_m: f64,
    pub max_transfer_m: f64,
    pub absolute_max_m: f64,
    pub stub_m: f64,
}

impl Parameters for P34Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("approach_lng", "Longitude the line comes from (default: the Tide's Newtown Road station).", -180.0, 180.0, -76.1902),
            ParamSpec::float("approach_lat", "Latitude the line comes from.", -90.0, 90.0, 36.8411),
            ParamSpec::float("platform_length_m", "Platform length (about two coupled light rail cars; not HRT's figure).", 30.0, 120.0, 60.0),
            ParamSpec::float("platform_width_m", "Platform width.", 3.0, 8.0, 4.0),
            ParamSpec::float("max_transfer_m", "Farthest the platform should be from a street (P34: 300 ft wherever possible).", 30.0, 183.0, 91.44),
            ParamSpec::float("absolute_max_m", "Farthest it may ever be (P34: an absolute maximum of 600 ft).", 30.0, 300.0, 182.88),
            ParamSpec::float("stub_m", "Track drawn outside the site, toward where the line comes from.", 50.0, 600.0, 250.0),
        ]
    }
    fn defaults() -> Self {
        Self { approach_lng: -76.1902, approach_lat: 36.8411, platform_length_m: 60.0, platform_width_m: 4.0, max_transfer_m: 91.44, absolute_max_m: 182.88, stub_m: 250.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.approach_lng, self.approach_lat, self.platform_length_m, self.platform_width_m, self.max_transfer_m, self.absolute_max_m, self.stub_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        let mut slots = [&mut p.approach_lng, &mut p.approach_lat, &mut p.platform_length_m, &mut p.platform_width_m, &mut p.max_transfer_m, &mut p.absolute_max_m, &mut p.stub_m];
        for (i, slot) in slots.iter_mut().enumerate() {
            if let (Some(s), Some(x)) = (schema.get(i), v.get(i)) {
                **slot = s.clamp(*x);
            }
        }
        p
    }
}

fn add(a: Pt2, b: Pt2, s: f64) -> Pt2 {
    Pt2::new(a.x + b.x * s, a.y + b.y * s)
}

fn ring_distance(p: Pt2, ring: &[Pt2]) -> f64 {
    let n = ring.len();
    (0..n).map(|i| planar::point_segment_distance(p, ring[i], ring[(i + 1) % n])).fold(f64::INFINITY, f64::min)
}

/// Where the ray from `from` along `dir` last leaves `ring`, as a distance along the ray.
fn exit_distance(from: Pt2, dir: Pt2, ring: &[Pt2]) -> Option<f64> {
    let n = ring.len();
    let mut best: Option<f64> = None;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let e = Pt2::new(b.x - a.x, b.y - a.y);
        let den = dir.x * e.y - dir.y * e.x;
        if den.abs() < 1e-12 {
            continue;
        }
        let w = Pt2::new(a.x - from.x, a.y - from.y);
        let t = (w.x * e.y - w.y * e.x) / den;
        let s = (w.x * dir.y - w.y * dir.x) / den;
        if t > 0.0 && (0.0..=1.0).contains(&s) {
            best = Some(best.map_or(t, |b| b.max(t)));
        }
    }
    best
}

struct Footprint {
    outer: Vec<Pt2>,
    holes: Vec<Vec<Pt2>>,
    centre: Pt2,
}

impl Footprint {
    fn inside(&self, p: Pt2) -> bool {
        planar::point_in_polygon(p, &self.outer) && !self.holes.iter().any(|h| planar::point_in_polygon(p, h))
    }
    fn distance(&self, p: Pt2) -> f64 {
        if self.inside(p) {
            return 0.0;
        }
        std::iter::once(&self.outer).chain(self.holes.iter()).map(|r| ring_distance(p, r)).fold(f64::INFINITY, f64::min)
    }
}

/// True when every point every `step` metres along a→b keeps `clear` metres from every footprint.
fn corridor_clear(a: Pt2, b: Pt2, clear: f64, step: f64, footprints: &[Footprint]) -> bool {
    let len = a.dist(b);
    let n = (len / step).ceil().max(1.0) as usize;
    (0..=n).all(|k| {
        let t = k as f64 / n as f64;
        let p = Pt2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        footprints.iter().all(|f| f.distance(p) >= clear)
    })
}

fn polygon_clear(poly: &[Pt2], clear: f64, footprints: &[Footprint]) -> bool {
    let n = poly.len();
    (0..n).all(|i| corridor_clear(poly[i], poly[(i + 1) % n], clear, 2.0, footprints))
        && footprints.iter().all(|f| !f.outer.iter().any(|v| planar::point_in_polygon(*v, poly)))
}

pub struct P34Interchange;

impl PatternOperator for P34Interchange {
    type Params = P34Params;

    fn name(&self) -> &'static str {
        "p34_interchange"
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p34".into(),
            display: "Alexander et al., A Pattern Language, Pattern 34 (Interchange)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Interchange-(34)".into()),
        }
    }

    fn description(&self) -> &'static str {
        "Bring a light rail line onto the site and make its station a center: clear of buildings, within 300 ft of the streets, ringed by buildings, with a square beside it."
    }

    fn apply(&self, nbhd: &Neighborhood, _parcel_id: &str, params: &Self::Params, seed: u64) -> Result<Subdivision, String> {
        if !nbhd.transit.is_empty() {
            return Err("p34_interchange: this site already has a transit line.".into());
        }
        let ring_ll = site_ring(nbhd).ok_or("p34_interchange: no site ring to bring the line into.")?;
        let origin = planar::average_centroid(&ring_ll);
        let ring = planar::ring_to_local(&ring_ll, &origin);
        let centre = planar::centroid(&ring);
        let approach = planar::lnglat_to_local(&LngLat::new(params.approach_lng, params.approach_lat), &origin);
        let to = Pt2::new(approach.x - centre.x, approach.y - centre.y);
        let dist = to.len();
        if dist < 1.0 {
            return Err("p34_interchange: the approach point is the site's middle.".into());
        }
        let u = Pt2::new(to.x / dist, to.y / dist); // outward, toward where the line comes from
        let n = Pt2::new(-u.y, u.x); // sideways
        let local = |r: &Vec<LngLat>| planar::ring_to_local(r, &origin);
        let footprints: Vec<Footprint> = nbhd
            .buildings
            .iter()
            .flat_map(|b| b.polygon.parts_view())
            .map(|p| {
                let outer = local(&p.outer);
                let centre = planar::centroid(&outer);
                Footprint { outer, holes: p.holes.iter().map(local).collect(), centre }
            })
            .collect();
        let streets: Vec<Vec<Pt2>> = nbhd
            .streets
            .iter()
            .filter(|s| s.centerline.len() >= 2)
            .map(|s| s.centerline.iter().map(|p| planar::lnglat_to_local(p, &origin)).collect())
            .collect();
        // Every 4 m along every street: where a path from the platform could join the network.
        let street_points: Vec<Pt2> = streets
            .iter()
            .flat_map(|line| {
                line.windows(2).flat_map(|w| {
                    let (a, b) = (w[0], w[1]);
                    let k = (a.dist(b) / 4.0).ceil().max(1.0) as usize;
                    (0..=k).map(move |i| add(a, Pt2::new(b.x - a.x, b.y - a.y), i as f64 / k as f64))
                })
            })
            .collect();
        // The nearest street point a clear, straight path can reach, within `limit`.
        let reachable = |p: Pt2, limit: f64| -> Option<(f64, Pt2)> {
            let mut near: Vec<(f64, Pt2)> = street_points.iter().map(|q| (p.dist(*q), *q)).filter(|(d, _)| *d <= limit).collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            near.into_iter().find(|(d, q)| *d <= 8.0 || corridor_clear(p, *q, 2.0, 2.0, &footprints))
        };

        let (pl, pw) = (params.platform_length_m, params.platform_width_m);
        // Where a parallel line `lat` metres sideways enters the site, as a point on the ring.
        let entry = |lat: f64| -> Option<Pt2> {
            let start = add(centre, n, lat);
            exit_distance(start, u, &ring).map(|t| add(start, u, t))
        };
        struct Pick {
            score: f64,
            enter: Pt2,
            end: Pt2,
            mid: Pt2,
            side: f64,
            neighbours: usize,
            inside_m: f64,
            transfer_m: f64,
        }
        let mut best: Option<Pick> = None;
        let mut limit_used = params.max_transfer_m;
        for limit in [params.max_transfer_m, params.absolute_max_m] {
        limit_used = limit;
        for lat in [0.0, 15.0, -15.0, 30.0, -30.0, 45.0, -45.0, 60.0, -60.0, 80.0, -80.0, 100.0, -100.0] {
            let Some(enter) = entry(lat) else { continue };
            let mut d = pl / 2.0 + 10.0;
            while d <= 240.0 {
                let mid = add(enter, u, -d);
                let end = add(mid, u, -(pl / 2.0 + OVERRUN_M));
                d += 5.0;
                if !planar::point_in_polygon(end, &ring) || !corridor_clear(enter, end, TRACK_CLEARANCE_M, 2.0, &footprints) {
                    continue;
                }
                for side in [1.0, -1.0] {
                    let near = PLATFORM_OFFSET_M * side;
                    let far = (PLATFORM_OFFSET_M + pw) * side;
                    let (p0, p1) = (add(mid, u, pl / 2.0), add(mid, u, -pl / 2.0));
                    let platform = vec![add(p0, n, near), add(p1, n, near), add(p1, n, far), add(p0, n, far)];
                    if !polygon_clear(&platform, 1.0, &footprints) {
                        continue;
                    }
                    let centre_pf = add(mid, n, (PLATFORM_OFFSET_M + pw / 2.0) * side);
                    let Some((transfer_m, _)) = reachable(centre_pf, limit) else { continue };
                    let neighbours = footprints.iter().filter(|f| f.centre.dist(centre_pf) <= 150.0).count();
                    let inside_m = enter.dist(end);
                    let score = -(neighbours as f64) * 20.0 + inside_m * 0.3 + lat.abs() * 0.2 + transfer_m * 0.1;
                    if best.as_ref().is_none_or(|b| score < b.score) {
                        best = Some(Pick { score, enter, end, mid, side, neighbours, inside_m, transfer_m });
                    }
                }
            }
        }
        if best.is_some() {
            break;
        }
        }
        let pick = best.ok_or("p34_interchange: no clear place for a platform within 600 ft of a street on the line's approach.")?;
        let Pick { enter, end, mid, side, neighbours, inside_m, transfer_m, .. } = pick;

        let line_id = "p34_light_rail".to_string();
        let station_id = format!("{line_id}_p34_station");
        let outside = add(enter, u, params.stub_m);
        let (p0, p1) = (add(mid, u, pl / 2.0), add(mid, u, -pl / 2.0));
        let near = PLATFORM_OFFSET_M * side;
        let far = (PLATFORM_OFFSET_M + pw) * side;
        let platform = vec![add(p0, n, near), add(p1, n, near), add(p1, n, far), add(p0, n, far)];
        let station_pt = add(mid, n, (PLATFORM_OFFSET_M + pw / 2.0) * side);
        let ll = |p: Pt2| planar::local_to_lnglat(p, &origin);
        let mut steps = vec![
            format!(
                "The line comes from ({:.4}, {:.4}), {:.0} m from the site's middle, and enters the site's edge; its track runs {inside_m:.0} m inside, to a bumper past the platform.",
                params.approach_lat, params.approach_lng, dist
            ),
            format!(
                "{station_id}: a {pl:.0} m platform with {neighbours} building(s) within 150 m, {transfer_m:.0} m from the nearest street ({}).",
                if limit_used <= params.max_transfer_m { "within P34's 300 ft" } else { "past P34's 300 ft, within its 600 ft absolute maximum: nothing closer was clear" }
            ),
        ];

        // A square beside the platform, as deep as fits.
        let mut new_open_space = vec![];
        let back = far + side * 0.5;
        for depth in [24.0, 16.0, 10.0] {
            let sq = vec![add(add(mid, u, pl / 2.0 + 5.0), n, back), add(add(mid, u, -pl / 2.0 - 5.0), n, back), add(add(mid, u, -pl / 2.0 - 5.0), n, back + side * depth), add(add(mid, u, pl / 2.0 + 5.0), n, back + side * depth)];
            if polygon_clear(&sq, 1.0, &footprints) && sq.iter().all(|p| planar::point_in_polygon(*p, &ring)) {
                new_open_space.push(OpenSpace { id: format!("{line_id}_p34_square"), polygon: Polygon::from_ring(planar::local_to_ring(&sq, &origin)), kind: OpenSpaceKind::Plaza });
                steps.push(format!("A square {depth:.0} m deep beside the platform."));
                break;
            }
        }
        // A path from the platform to the street, unless it's already on one.
        let mut new_streets = vec![];
        if let Some((d, q)) = reachable(station_pt, limit_used) {
            if d > 8.0 {
                // P121 Path Shape, as path_network bulges its own segments: the
                // path's middle swings 1.5 widths off the straight line, to
                // whichever side stays clear of buildings (straight if neither does).
                let width = 4.0;
                let (dx, dy) = ((q.x - station_pt.x) / d, (q.y - station_pt.y) / d);
                let half = add(station_pt, Pt2::new(q.x - station_pt.x, q.y - station_pt.y), 0.5);
                let bulge = [1.0, -1.0].into_iter().map(|s| add(half, Pt2::new(-dy, dx), s * width * 1.5)).find(|m| {
                    corridor_clear(station_pt, *m, 2.0, 2.0, &footprints) && corridor_clear(*m, q, 2.0, 2.0, &footprints)
                });
                let centerline = match bulge {
                    Some(m) => vec![ll(station_pt), ll(m), ll(q)],
                    None => vec![ll(station_pt), ll(q)],
                };
                new_streets.push(Street {
                    id: format!("{line_id}_p34_link"),
                    centerline,
                    classification: Some("pedestrian".into()),
                    row_width_m: Some(width),
                    surface: Some("grass_pavers".into()),
                });
                steps.push(format!("A {d:.0} m path from the platform to the nearest street, so the station is continuous with the pedestrian network."));
            }
        }

        let line = TransitLine {
            id: line_id.clone(),
            kind: "light_rail".into(),
            centerline: vec![ll(outside), ll(enter), ll(end)],
            stations: vec![TransitStation { id: station_id.clone(), location: ll(station_pt), platform: Polygon::from_ring(planar::local_to_ring(&platform, &origin)) }],
        };
        let node = ActivityNode {
            id: station_id,
            location: ll(station_pt),
            kind: ActivityKind::Transit,
            intensity: None,
            label: Some("Light rail".into()),
            activity_fit: Default::default(),
            publicness: None,
        };
        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("A light rail station {transfer_m:.0} m from the streets, {neighbours} buildings around it, at the end of a line from Newtown Road."),
            steps,
            caveats: vec![
                "The track is a straight sketch from the direction of Newtown Road, not HRT's alignment, and the station's place is this generator's choice, not HRT's or the city's.".into(),
                "No buses: the transfer rule is measured to the nearest street.".into(),
                "Platform length and width are plausible figures, not HRT's.".into(),
            ],
            seed,
            params: params.as_map(),
        };
        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space,
            new_buildings: vec![],
            new_streets,
            new_activity_nodes: vec![node],
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
            new_fields: vec![],
            new_trees: vec![],
            new_ground_cover: vec![],
            new_transit: vec![line],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::haversine_m;
    use street_smarts_core::nir::{Boundary, BoundaryKind, Building, NeighborhoodMeta};

    const M: f64 = 1.0 / 111_320.0;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<LngLat> {
        vec![
            LngLat::new(x0 * M, y0 * M),
            LngLat::new(x1 * M, y0 * M),
            LngLat::new(x1 * M, y1 * M),
            LngLat::new(x0 * M, y1 * M),
            LngLat::new(x0 * M, y0 * M),
        ]
    }

    fn building(id: &str, r: Vec<LngLat>) -> Building {
        let mut b: Building = serde_json::from_value(serde_json::json!({"id": id, "polygon": {"outer": [], "holes": []}})).unwrap();
        b.polygon = Polygon::from_ring(r);
        b
    }

    /// A 300 m site with a row of buildings across its middle and a street along its south side;
    /// the line comes from far to the east.
    fn site() -> (Neighborhood, P34Params) {
        let n = Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 300.0 * M, 300.0 * M],
            parcels: vec![],
            buildings: vec![building("B1", rect(40.0, 120.0, 140.0, 180.0)), building("B2", rect(160.0, 120.0, 260.0, 180.0))],
            streets: vec![Street {
                id: "S1".into(),
                centerline: vec![LngLat::new(0.0, 60.0 * M), LngLat::new(300.0 * M, 60.0 * M)],
                classification: Some("local".into()),
                row_width_m: Some(6.0),
                surface: None,
            }],
            open_space: vec![],
            boundaries: vec![Boundary { id: "site_perimeter".into(), centerline: rect(0.0, 0.0, 300.0, 300.0), kind: BoundaryKind::Jurisdictional }],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P34 unit fixture".into(),
            },
            pattern_fields: vec![],
            trees: vec![],
            ground_cover: vec![],
            transit: vec![],
        };
        let params = P34Params { approach_lng: 2000.0 * M, approach_lat: 150.0 * M, ..P34Params::defaults() };
        (n, params)
    }

    #[test]
    fn places_a_station_with_its_line_square_and_marker() {
        let (n, p) = site();
        let sub = P34Interchange.apply(&n, "*", &p, 1).unwrap();
        assert_eq!(sub.new_transit.len(), 1);
        let line = &sub.new_transit[0];
        assert_eq!(line.kind, "light_rail");
        assert_eq!(line.stations.len(), 1);
        assert_eq!(sub.new_activity_nodes.len(), 1);
        assert_eq!(sub.new_activity_nodes[0].kind, ActivityKind::Transit);
        assert!(sub.new_activity_nodes[0].id.ends_with("_p34_station"));
        // The line starts outside the site, toward the east, and ends inside it.
        assert!(line.centerline[0].lng > 300.0 * M);
        let end = line.centerline.last().unwrap();
        assert!(end.lng > 0.0 && end.lng < 300.0 * M && end.lat > 0.0 && end.lat < 300.0 * M);
    }

    #[test]
    fn the_platform_is_within_300_feet_of_a_street_and_clear_of_buildings() {
        let (n, p) = site();
        let sub = P34Interchange.apply(&n, "*", &p, 1).unwrap();
        let st = &sub.new_transit[0].stations[0];
        let street = &n.streets[0].centerline;
        let d = street_smarts_core::geometry::point_to_polyline_m(&st.location, street);
        let linked = !sub.new_streets.is_empty();
        assert!(d <= 91.5 || linked, "platform {d:.0} m from the street and no link");
        for v in &st.platform.outer {
            let (x, y) = (v.lng / M, v.lat / M);
            for (x0, y0, x1, y1) in [(40.0, 120.0, 140.0, 180.0), (160.0, 120.0, 260.0, 180.0)] {
                assert!(!(x > x0 && x < x1 && y > y0 && y < y1), "platform corner inside a building");
            }
        }
        assert!(haversine_m(&st.location, sub.new_transit[0].centerline.last().unwrap()) < 60.0);
    }

    #[test]
    fn no_clear_place_is_an_error_and_a_second_run_too() {
        let (mut n, p) = site();
        // Fill the site with one building: nowhere for a platform.
        n.buildings = vec![building("ALL", rect(1.0, 1.0, 299.0, 299.0))];
        assert!(P34Interchange.apply(&n, "*", &p, 1).is_err());
        let (n, p) = site();
        let once = crate::subdivision::apply_subdivision(&n, &P34Interchange.apply(&n, "*", &p, 1).unwrap());
        assert!(P34Interchange.apply(&once, "*", &p, 1).is_err(), "one line is enough");
    }
}
