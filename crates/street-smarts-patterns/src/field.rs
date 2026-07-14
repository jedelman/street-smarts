//! Rasterized pressure fields for seed placement.
//!
//! A narrow port of eastside-commons' `EC_FieldSolver`
//! (`ec-solver-worker.js` in the `jason-edelman.org` repo) into
//! street-smarts' `Pt2`/local-meter conventions. EC paints Gaussian and
//! segment "pressure" from pattern anchors onto a rasterized grid,
//! combines multiple patterns' fields with a weighted sum, and reads
//! seed/hot-node locations back off the combined field. That's a real,
//! independently-useful idea for seed placement that street-smarts'
//! existing `stratified_seeds` (blind jittered-grid, no awareness of
//! surrounding context) doesn't attempt.
//!
//! # What this is NOT
//! EC's real pipeline goes much further than this module does: after
//! combining fields it traces walkable lines through the field
//! (`traceFieldLine`), finds local maxima ALONG those traced lines
//! (`buildHotNodes`), then clusters hot nodes into axis-aligned bounding
//! rectangles (`buildFootprints`) with no shape/dimension verification.
//! street-smarts already has a real, verified footprint step
//! (`planar::voronoi_cell` + `clip_to_polygon` + `inset_convex`) that's
//! more rigorous than EC's bounding-rect approach -- there's no reason to
//! port that part. This module stops at producing SEED POINTS
//! (`Field::find_seeds`, a simplified stand-in for `buildHotNodes`: local
//! maxima above a threshold, deduped by minimum separation, no line
//! tracing) and hands them to the existing Voronoi machinery from there.

use crate::planar::{point_in_polygon, point_segment_distance, Pt2};

/// A rasterized scalar field over an axis-aligned region in local meters.
pub struct Field {
    pub min: Pt2,
    pub cell_size: f64,
    pub cols: usize,
    pub rows: usize,
    data: Vec<f64>,
}

impl Field {
    pub fn new(min: Pt2, max: Pt2, cell_size: f64) -> Self {
        let cell_size = cell_size.max(0.5);
        let cols = (((max.x - min.x) / cell_size).ceil() as usize).max(1) + 1;
        let rows = (((max.y - min.y) / cell_size).ceil() as usize).max(1) + 1;
        Self { min, cell_size, cols, rows, data: vec![0.0; cols * rows] }
    }

    fn idx(&self, cx: usize, cy: usize) -> usize {
        cy * self.cols + cx
    }

    fn cell_center(&self, cx: usize, cy: usize) -> Pt2 {
        Pt2::new(
            self.min.x + (cx as f64 + 0.5) * self.cell_size,
            self.min.y + (cy as f64 + 0.5) * self.cell_size,
        )
    }

    fn add_at(&mut self, cx: usize, cy: usize, v: f64) {
        if cx < self.cols && cy < self.rows {
            let i = self.idx(cx, cy);
            self.data[i] += v;
        }
    }

    /// EC's `paintGaussian`: add a Gaussian bump of the given `weight`
    /// centered at `center`, falling off over `sigma` meters. Positive
    /// weight attracts seeds toward `center`; negative weight repels them.
    pub fn paint_gaussian(&mut self, center: Pt2, sigma: f64, weight: f64) {
        if sigma <= 0.0 || weight == 0.0 {
            return;
        }
        let radius_cells = ((sigma * 3.0) / self.cell_size).ceil() as isize;
        let ccx = ((center.x - self.min.x) / self.cell_size).round() as isize;
        let ccy = ((center.y - self.min.y) / self.cell_size).round() as isize;
        let two_sigma_sq = 2.0 * sigma * sigma;
        for dy in -radius_cells..=radius_cells {
            for dx in -radius_cells..=radius_cells {
                let cx = ccx + dx;
                let cy = ccy + dy;
                if cx < 0 || cy < 0 {
                    continue;
                }
                let (cx, cy) = (cx as usize, cy as usize);
                if cx >= self.cols || cy >= self.rows {
                    continue;
                }
                let p = self.cell_center(cx, cy);
                let d2 = (p.x - center.x).powi(2) + (p.y - center.y).powi(2);
                let v = weight * (-d2 / two_sigma_sq).exp();
                self.add_at(cx, cy, v);
            }
        }
    }

    /// EC's `paintSegment`: same Gaussian falloff, but distance-to-segment
    /// instead of distance-to-point -- pressure along a line (a street
    /// centerline, say) rather than radiating from one spot.
    pub fn paint_segment(&mut self, a: Pt2, b: Pt2, sigma: f64, weight: f64) {
        if sigma <= 0.0 || weight == 0.0 {
            return;
        }
        let two_sigma_sq = 2.0 * sigma * sigma;
        for cy in 0..self.rows {
            for cx in 0..self.cols {
                let p = self.cell_center(cx, cy);
                let d = point_segment_distance(p, a, b);
                if d > sigma * 3.0 {
                    continue;
                }
                let v = weight * (-(d * d) / two_sigma_sq).exp();
                self.add_at(cx, cy, v);
            }
        }
    }

    /// Scale all cell values so the maximum is 1.0 (no-op on an all-zero
    /// field). EC's `normalizeField`.
    pub fn normalize(&mut self) {
        let max = self.data.iter().cloned().fold(0.0_f64, f64::max);
        if max <= 0.0 {
            return;
        }
        for v in self.data.iter_mut() {
            *v /= max;
        }
    }

    pub fn sample(&self, p: Pt2) -> f64 {
        let cx = ((p.x - self.min.x) / self.cell_size).round();
        let cy = ((p.y - self.min.y) / self.cell_size).round();
        if cx < 0.0 || cy < 0.0 {
            return 0.0;
        }
        let (cx, cy) = (cx as usize, cy as usize);
        if cx >= self.cols || cy >= self.rows {
            return 0.0;
        }
        self.data[self.idx(cx, cy)]
    }

    /// EC's `combineFields`: weighted sum of same-shaped fields. Fields
    /// with a different grid than the first are skipped (callers should
    /// build every field over the same `min`/`cell_size`/extent).
    pub fn combine(min: Pt2, max: Pt2, cell_size: f64, fields: &[(&Field, f64)]) -> Field {
        let mut out = Field::new(min, max, cell_size);
        for (f, weight) in fields {
            if f.cols != out.cols || f.rows != out.rows || f.min != out.min {
                continue;
            }
            for i in 0..out.data.len() {
                out.data[i] += f.data[i] * weight;
            }
        }
        out
    }

    /// A simplified adaptation of EC's `buildHotNodes`: cells that are
    /// local maxima (value >= all 8 neighbors) above `threshold`, restricted
    /// to points inside `poly`, taken strongest-first, deduped by requiring
    /// at least `min_separation_m` from every seed already chosen. Returns
    /// up to `count` points -- fewer if the field doesn't support that many
    /// well-separated peaks above threshold.
    pub fn find_seeds(
        &self,
        poly: &[Pt2],
        count: usize,
        threshold: f64,
        min_separation_m: f64,
    ) -> Vec<Pt2> {
        let mut candidates: Vec<(f64, Pt2)> = Vec::new();
        for cy in 0..self.rows {
            for cx in 0..self.cols {
                let v = self.data[self.idx(cx, cy)];
                if v < threshold {
                    continue;
                }
                let mut is_max = true;
                'neighbors: for dy in -1isize..=1 {
                    for dx in -1isize..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = cx as isize + dx;
                        let ny = cy as isize + dy;
                        if nx < 0 || ny < 0 {
                            continue;
                        }
                        let (nx, ny) = (nx as usize, ny as usize);
                        if nx >= self.cols || ny >= self.rows {
                            continue;
                        }
                        if self.data[self.idx(nx, ny)] > v {
                            is_max = false;
                            break 'neighbors;
                        }
                    }
                }
                if !is_max {
                    continue;
                }
                let p = self.cell_center(cx, cy);
                if !point_in_polygon(p, poly) {
                    continue;
                }
                candidates.push((v, p));
            }
        }
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let mut chosen: Vec<Pt2> = Vec::new();
        for (_, p) in candidates {
            if chosen.len() >= count {
                break;
            }
            if chosen.iter().all(|c| c.dist(p) >= min_separation_m) {
                chosen.push(p);
            }
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_gaussian_peaks_at_its_center() {
        let mut f = Field::new(Pt2::new(0.0, 0.0), Pt2::new(100.0, 100.0), 2.0);
        f.paint_gaussian(Pt2::new(50.0, 50.0), 15.0, 1.0);
        let peak = f.sample(Pt2::new(50.0, 50.0));
        let off_center = f.sample(Pt2::new(10.0, 10.0));
        assert!(peak > off_center);
        assert!(peak > 0.9);
    }

    #[test]
    fn normalize_scales_max_to_one() {
        let mut f = Field::new(Pt2::new(0.0, 0.0), Pt2::new(50.0, 50.0), 2.0);
        f.paint_gaussian(Pt2::new(25.0, 25.0), 10.0, 3.0);
        f.normalize();
        let max = f.data.iter().cloned().fold(0.0_f64, f64::max);
        assert!((max - 1.0).abs() < 1e-9);
    }

    #[test]
    fn find_seeds_locates_two_separated_peaks() {
        let poly = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(100.0, 0.0),
            Pt2::new(100.0, 100.0),
            Pt2::new(0.0, 100.0),
        ];
        let mut f = Field::new(Pt2::new(0.0, 0.0), Pt2::new(100.0, 100.0), 2.0);
        f.paint_gaussian(Pt2::new(20.0, 20.0), 8.0, 1.0);
        f.paint_gaussian(Pt2::new(80.0, 80.0), 8.0, 1.0);
        let seeds = f.find_seeds(&poly, 5, 0.3, 10.0);
        assert_eq!(seeds.len(), 2);
        let near = |p: Pt2, target: Pt2| p.dist(target) < 5.0;
        assert!(seeds.iter().any(|&s| near(s, Pt2::new(20.0, 20.0))));
        assert!(seeds.iter().any(|&s| near(s, Pt2::new(80.0, 80.0))));
    }

    #[test]
    fn find_seeds_respects_min_separation() {
        let poly = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(100.0, 0.0),
            Pt2::new(100.0, 100.0),
            Pt2::new(0.0, 100.0),
        ];
        let mut f = Field::new(Pt2::new(0.0, 0.0), Pt2::new(100.0, 100.0), 2.0);
        // Two close-together peaks -- with a large min_separation, only one
        // should survive dedup.
        f.paint_gaussian(Pt2::new(48.0, 50.0), 8.0, 1.0);
        f.paint_gaussian(Pt2::new(52.0, 50.0), 8.0, 0.9);
        let seeds = f.find_seeds(&poly, 5, 0.3, 40.0);
        assert_eq!(seeds.len(), 1);
    }

    #[test]
    fn find_seeds_ignores_points_outside_the_polygon() {
        // A small triangle in the corner of a bigger field -- a peak
        // outside it should never be returned.
        let poly = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(20.0, 0.0),
            Pt2::new(0.0, 20.0),
        ];
        let mut f = Field::new(Pt2::new(0.0, 0.0), Pt2::new(100.0, 100.0), 2.0);
        f.paint_gaussian(Pt2::new(80.0, 80.0), 8.0, 1.0);
        let seeds = f.find_seeds(&poly, 5, 0.3, 5.0);
        assert!(seeds.is_empty());
    }

    #[test]
    fn paint_segment_is_strong_along_the_line_and_weak_far_from_it() {
        let mut f = Field::new(Pt2::new(0.0, 0.0), Pt2::new(100.0, 100.0), 2.0);
        f.paint_segment(Pt2::new(10.0, 50.0), Pt2::new(90.0, 50.0), 10.0, 1.0);
        let on_line = f.sample(Pt2::new(50.0, 50.0));
        let far = f.sample(Pt2::new(50.0, 5.0));
        assert!(on_line > far);
    }

    #[test]
    fn combine_sums_weighted_fields() {
        let min = Pt2::new(0.0, 0.0);
        let max = Pt2::new(40.0, 40.0);
        let mut a = Field::new(min, max, 2.0);
        a.paint_gaussian(Pt2::new(20.0, 20.0), 8.0, 1.0);
        let mut b = Field::new(min, max, 2.0);
        b.paint_gaussian(Pt2::new(20.0, 20.0), 8.0, 1.0);
        let combined = Field::combine(min, max, 2.0, &[(&a, 1.0), (&b, 2.0)]);
        let expected = a.sample(Pt2::new(20.0, 20.0)) + 2.0 * b.sample(Pt2::new(20.0, 20.0));
        assert!((combined.sample(Pt2::new(20.0, 20.0)) - expected).abs() < 1e-9);
    }
}
