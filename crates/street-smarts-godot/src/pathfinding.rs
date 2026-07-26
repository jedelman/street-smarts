//! Grid-based A* pathfinding over the real per-building footprint colliders
//! walk-mode collision already uses (see `building_mesh::FootprintCollider`).
//!
//! Before this existed, tap-to-walk was straight-line-plus-slide only:
//! `orbit_camera.gd`'s own doc says so plainly -- "tapping a point behind
//! a building walks into its near wall, slides along it, and gives up
//! rather than routing around." This is the "real navmesh" that doc called
//! "now actually buildable, since the footprint polygons collision uses
//! are exactly what a navmesh would be baked from."
//!
//! # Why a hand-rolled grid instead of Godot's own NavigationServer3D
//! Godot 4.3 does have a full navigation/pathfinding subsystem
//! (`NavigationServer3D`, `NavigationRegion3D`, `NavigationObstacle3D`) --
//! confirmed reachable from this GDExtension (it's gated behind gdext's
//! `experimental-godot-api` Cargo feature, since Godot's own docs still
//! mark these classes experimental). It was deliberately NOT used here:
//! its static path queries (`map_get_path`) only see whatever's actually
//! baked into a `NavigationMesh` resource, and baking correctly around
//! real 3D building geometry needs either a real 2D polygon boolean
//! subtraction (site minus every building footprint -- a real, nontrivial
//! op this codebase doesn't have) or Recast's automatic mesh-based bake
//! (untested here, and a real risk: a shed roof's ~1 degree slope might
//! register as walkable "floor" on the rooftop, same misreading Recast
//! would make of any near-flat surface). A grid over the SAME
//! `FootprintCollider` SDF that `resolve_move` already trusts for
//! real-time collision has no such ambiguity, is directly unit-testable,
//! and reuses code already proven correct rather than a new, unfamiliar
//! subsystem. `experimental-godot-api` is not enabled in this project's
//! `Cargo.toml` -- nothing here depends on it.

use crate::building_mesh::FootprintCollider;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Real clearance to the nearest real building footprint at (x, z) --
/// negative inside a solid, positive on open ground (courtyard interiors
/// included, same as `FootprintCollider::distance`'s own convention),
/// folded across every real building on the site. Shared by
/// `NeighborhoodNode3D::resolve_move`'s per-step collision and this
/// module's per-cell walkability test, so the two never disagree about
/// what counts as clear.
pub fn clearance(colliders: &[FootprintCollider], x: f64, z: f64) -> f64 {
    colliders.iter().map(|c| c.distance(x, z)).fold(f64::MAX, f64::min)
}

const NEIGHBOR_OFFSETS: [(isize, isize); 8] =
    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

#[derive(Copy, Clone)]
struct HeapItem {
    f: f64,
    col: usize,
    row: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.f == other.f
    }
}
impl Eq for HeapItem {}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapItem {
    // Reversed so `BinaryHeap` (a max-heap) pops the SMALLEST f-score
    // first -- standard A* open-set behavior.
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

/// A real walkability grid over one neighborhood's real footprint
/// colliders, baked once per rebuild and queried per tap-to-walk.
pub struct NavGrid {
    min_x: f64,
    min_z: f64,
    cell_size: f64,
    cols: usize,
    rows: usize,
    walkable: Vec<bool>,
}

impl NavGrid {
    /// Real body radius a walker needs clear at a cell's own center for
    /// that cell to count as walkable -- matches `orbit_camera.gd`'s own
    /// `body_radius_m` default. Baked into the grid rather than accepted
    /// per-query: `resolve_move` still does the exact, continuous
    /// real-time collision check every step regardless of what this grid
    /// says, so this only needs to be a reasonable, not pixel-exact,
    /// match to the real walker's own radius.
    pub const BODY_RADIUS_M: f64 = 0.35;
    /// Real grid resolution. Coarse enough that even a full ~600m+ real
    /// site (see `orbit_camera.gd`'s own doc on `max_distance`) stays a
    /// small grid (a few hundred cells per side), fine enough to route
    /// around a real building corner rather than cutting through it.
    pub const CELL_SIZE_M: f64 = 2.0;
    /// Real margin beyond the tightest real bounding box of every
    /// collider, so a query starting or ending near the site's own edge
    /// still has real grid coverage around it instead of falling off it.
    pub const PAD_M: f64 = 30.0;

    /// Builds a walkability grid covering `bounds` (min_x, min_z, max_x,
    /// max_z in local meters), padded by `Self::PAD_M`. A cell is walkable
    /// if its own center clears every real building by at least
    /// `Self::BODY_RADIUS_M` -- the same real test `resolve_move` already
    /// uses per step, sampled once per cell instead of continuously.
    pub fn build(colliders: &[FootprintCollider], bounds: (f64, f64, f64, f64)) -> Self {
        let (bmin_x, bmin_z, bmax_x, bmax_z) = bounds;
        let min_x = bmin_x - Self::PAD_M;
        let min_z = bmin_z - Self::PAD_M;
        let max_x = bmax_x + Self::PAD_M;
        let max_z = bmax_z + Self::PAD_M;
        let cols = (((max_x - min_x) / Self::CELL_SIZE_M).ceil().max(1.0)) as usize;
        let rows = (((max_z - min_z) / Self::CELL_SIZE_M).ceil().max(1.0)) as usize;
        let mut walkable = vec![false; cols * rows];
        for row in 0..rows {
            for col in 0..cols {
                let (x, z) = Self::center_at(min_x, min_z, col, row);
                walkable[row * cols + col] = clearance(colliders, x, z) >= Self::BODY_RADIUS_M;
            }
        }
        Self { min_x, min_z, cell_size: Self::CELL_SIZE_M, cols, rows, walkable }
    }

    fn center_at(min_x: f64, min_z: f64, col: usize, row: usize) -> (f64, f64) {
        (min_x + (col as f64 + 0.5) * Self::CELL_SIZE_M, min_z + (row as f64 + 0.5) * Self::CELL_SIZE_M)
    }

    fn center_of(&self, col: usize, row: usize) -> (f64, f64) {
        Self::center_at(self.min_x, self.min_z, col, row)
    }

    fn cell_of(&self, x: f64, z: f64) -> Option<(usize, usize)> {
        let col = ((x - self.min_x) / self.cell_size).floor();
        let row = ((z - self.min_z) / self.cell_size).floor();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some((col, row))
    }

    fn is_walkable(&self, col: usize, row: usize) -> bool {
        self.walkable[row * self.cols + col]
    }

    /// Nearest walkable cell to (col, row), by expanding ring search --
    /// so a start/goal point right at a building's own wall (inside its
    /// own `BODY_RADIUS_M` margin, not literally inside the solid) still
    /// has somewhere real to path from/to instead of failing outright.
    /// `None` if nothing walkable exists within `max_rings` cells (a
    /// genuinely enclosed point, e.g. a fully sealed interior).
    fn nearest_walkable(&self, col: usize, row: usize, max_rings: isize) -> Option<(usize, usize)> {
        if self.is_walkable(col, row) {
            return Some((col, row));
        }
        for ring in 1..=max_rings {
            for dc in -ring..=ring {
                for dr in -ring..=ring {
                    if dc.abs() != ring && dr.abs() != ring {
                        continue; // interior of the ring, already checked at a smaller ring
                    }
                    let (c, r) = (col as isize + dc, row as isize + dr);
                    if c < 0 || r < 0 {
                        continue;
                    }
                    let (c, r) = (c as usize, r as usize);
                    if c >= self.cols || r >= self.rows {
                        continue;
                    }
                    if self.is_walkable(c, r) {
                        return Some((c, r));
                    }
                }
            }
        }
        None
    }

    /// Real straight-line clearance check between two points, sampled
    /// every `step` meters -- used by `simplify` to confirm a shortcut
    /// between two non-adjacent waypoints is actually real, not just
    /// grid-adjacent-looking.
    fn line_clear(&self, colliders: &[FootprintCollider], a: (f64, f64), b: (f64, f64), step: f64) -> bool {
        let dx = b.0 - a.0;
        let dz = b.1 - a.1;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-9 {
            return true;
        }
        let steps = (len / step).ceil().max(1.0) as usize;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            if clearance(colliders, a.0 + dx * t, a.1 + dz * t) < Self::BODY_RADIUS_M {
                return false;
            }
        }
        true
    }

    /// Greedy string-pulling: from each kept waypoint, jump ahead to the
    /// farthest later waypoint still in real line-of-sight clearance,
    /// dropping everything in between -- turns the raw grid-cell
    /// staircase into a real, straighter route without ever cutting
    /// through a building's own real clearance margin to do it.
    fn simplify(&self, colliders: &[FootprintCollider], path: &[(f64, f64)]) -> Vec<(f64, f64)> {
        if path.len() <= 2 {
            return path.to_vec();
        }
        let mut out = vec![path[0]];
        let mut i = 0;
        while i < path.len() - 1 {
            let mut j = path.len() - 1;
            // Sampled at half the real body radius, not the (much
            // coarser) cell size -- a step wider than the clearance
            // margin itself can step clean over a real corner clip
            // without ever landing a sample inside it. Caught by this
            // module's own test: a cell-size-scaled step let a shortcut
            // graze a building corner that a finer external check flagged.
            while j > i + 1 && !self.line_clear(colliders, path[i], path[j], Self::BODY_RADIUS_M * 0.5) {
                j -= 1;
            }
            out.push(path[j]);
            i = j;
        }
        out
    }

    /// Real A* search over the walkable grid, 8-connected (diagonal moves
    /// blocked when both flanking orthogonal cells are unwalkable, so a
    /// path can't cut across a building's own corner), from `from` to `to`
    /// (real world/local-meter ground points, snapped to their own
    /// nearest walkable cell first). `None` if no real route exists.
    /// Returns a simplified real waypoint list, not the raw per-cell
    /// staircase.
    pub fn find_path(&self, colliders: &[FootprintCollider], from: (f64, f64), to: (f64, f64)) -> Option<Vec<(f64, f64)>> {
        let start = self.cell_of(from.0, from.1).and_then(|(c, r)| self.nearest_walkable(c, r, 5))?;
        let goal = self.cell_of(to.0, to.1).and_then(|(c, r)| self.nearest_walkable(c, r, 5))?;

        let heuristic = |col: usize, row: usize| -> f64 {
            let dc = col as f64 - goal.0 as f64;
            let dr = row as f64 - goal.1 as f64;
            (dc * dc + dr * dr).sqrt() * self.cell_size
        };

        let mut open: BinaryHeap<HeapItem> = BinaryHeap::new();
        open.push(HeapItem { f: heuristic(start.0, start.1), col: start.0, row: start.1 });
        let mut g_score: HashMap<(usize, usize), f64> = HashMap::new();
        g_score.insert(start, 0.0);
        let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();

        while let Some(HeapItem { col, row, .. }) = open.pop() {
            if (col, row) == goal {
                let mut cells = vec![(col, row)];
                let mut cur = (col, row);
                while let Some(&prev) = came_from.get(&cur) {
                    cells.push(prev);
                    cur = prev;
                }
                cells.reverse();
                let points: Vec<(f64, f64)> = cells.iter().map(|&(c, r)| self.center_of(c, r)).collect();
                return Some(self.simplify(colliders, &points));
            }

            let current_g = *g_score.get(&(col, row)).unwrap_or(&f64::MAX);
            for &(dc, dr) in &NEIGHBOR_OFFSETS {
                let (nc_i, nr_i) = (col as isize + dc, row as isize + dr);
                if nc_i < 0 || nr_i < 0 {
                    continue;
                }
                let (nc, nr) = (nc_i as usize, nr_i as usize);
                if nc >= self.cols || nr >= self.rows || !self.is_walkable(nc, nr) {
                    continue;
                }
                if dc != 0 && dr != 0 {
                    // Block cutting diagonally across a corner: both the
                    // orthogonal cells flanking this diagonal move must
                    // also be walkable, or a path could clip a building's
                    // own corner even though neither grid cell it passes
                    // through is individually blocked.
                    let flank_a_walkable = self.is_walkable((col as isize + dc) as usize, row);
                    let flank_b_walkable = self.is_walkable(col, (row as isize + dr) as usize);
                    if !flank_a_walkable || !flank_b_walkable {
                        continue;
                    }
                }
                let step_cost = if dc != 0 && dr != 0 { self.cell_size * std::f64::consts::SQRT_2 } else { self.cell_size };
                let tentative_g = current_g + step_cost;
                if tentative_g < *g_score.get(&(nc, nr)).unwrap_or(&f64::MAX) {
                    g_score.insert((nc, nr), tentative_g);
                    came_from.insert((nc, nr), (col, row));
                    open.push(HeapItem { f: tentative_g + heuristic(nc, nr), col: nc, row: nr });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building_mesh::FootprintCollider;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::Building;
    use street_smarts_patterns::planar::local_to_ring;

    fn rect_collider(cx: f64, cz: f64, half_w: f64, half_d: f64, origin: &LngLat) -> FootprintCollider {
        let local = [
            street_smarts_patterns::planar::Pt2::new(cx - half_w, cz - half_d),
            street_smarts_patterns::planar::Pt2::new(cx + half_w, cz - half_d),
            street_smarts_patterns::planar::Pt2::new(cx + half_w, cz + half_d),
            street_smarts_patterns::planar::Pt2::new(cx - half_w, cz + half_d),
        ];
        let building = Building {
            id: "B".into(),
            polygon: Polygon::from_ring(local_to_ring(&local, origin)),
            height_m: Some(5.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            roof_segments: vec![],
            canopies: vec![],
            wall_niches: vec![],
        };
        FootprintCollider::from_building(&building, origin).unwrap()
    }

    #[test]
    fn routes_around_a_real_building_instead_of_a_straight_line_through_it() {
        let origin = LngLat::new(-76.1, 36.8);
        // A wide building spanning most of the site's width, centered
        // between the start and goal -- a straight line from one side to
        // the other MUST cross it.
        let collider = rect_collider(0.0, 0.0, 30.0, 8.0, &origin);
        let colliders = vec![collider];
        let grid = NavGrid::build(&colliders, (-50.0, -50.0, 50.0, 50.0));

        let path = grid.find_path(&colliders, (-40.0, 0.0), (40.0, 0.0)).expect("a real route around the building must exist");
        assert!(path.len() >= 2, "expected a real multi-point route, got {path:?}");

        // Every consecutive segment of the real returned path must stay
        // clear of the building -- not just the waypoints themselves.
        for pair in path.windows(2) {
            assert!(
                grid.line_clear(&colliders, pair[0], pair[1], 0.5),
                "segment {:?} -> {:?} should stay clear of the real building, but doesn't",
                pair[0], pair[1]
            );
        }

        // And the path must actually go around, not through: at least one
        // real waypoint should sit outside the building's own Z span.
        assert!(
            path.iter().any(|&(_, z)| z.abs() > 8.0),
            "expected the route to detour around the building's Z extent, got {path:?}"
        );
    }

    #[test]
    fn a_fully_enclosed_point_has_no_real_route() {
        let origin = LngLat::new(-76.1, 36.8);
        // A big square with a small courtyard hole would be walkable
        // inside; here we just use a big solid block with no interior
        // void, so its own center is fully enclosed.
        let collider = rect_collider(0.0, 0.0, 20.0, 20.0, &origin);
        let colliders = vec![collider];
        let grid = NavGrid::build(&colliders, (-50.0, -50.0, 50.0, 50.0));
        assert!(grid.find_path(&colliders, (0.0, 0.0), (40.0, 0.0)).is_none());
    }

    #[test]
    fn open_ground_with_no_obstacles_gives_a_direct_route() {
        let origin = LngLat::new(-76.1, 36.8);
        // A building far away from both endpoints -- shouldn't affect a
        // direct path between two clearly open points.
        let collider = rect_collider(500.0, 500.0, 5.0, 5.0, &origin);
        let colliders = vec![collider];
        let grid = NavGrid::build(&colliders, (-50.0, -50.0, 50.0, 50.0));
        let path = grid.find_path(&colliders, (-40.0, 0.0), (40.0, 0.0)).expect("open ground should have a route");
        // With nothing in the way, string-pulling should collapse it to
        // just the two endpoints.
        assert_eq!(path.len(), 2, "expected a direct 2-point route on open ground, got {path:?}");
    }
}
