//! Canopy loss detection for Ghent, Norfolk VA.
//!
//! Fetches multi-year NAIP imagery from Planetary Computer, computes
//! per-pixel NDVI, and diffs between years to identify tree removal.
//!
//! Uses per-year adaptive thresholds to handle NAIP radiometric
//! inconsistencies between vintages (different processing chains).
//!
//! Usage: cargo run --release
//!   (from within `nix develop` shell for GDAL)

use gdal::raster::ResampleAlg;
use gdal::spatial_ref::{AxisMappingStrategy, CoordTransform, SpatialRef};
use gdal::Dataset;

use std::collections::BTreeMap;

// Ghent neighborhood bounding box (EPSG:4326)
// Trimmed to area covered by NAIP _ne tile (south edge ~36.871)
const GHENT_BBOX: [f64; 4] = [-76.305, 36.871, -76.285, 36.878];

const PIXEL_SIZE_M: f64 = 1.0;
const NODATA: f32 = -9999.0;

struct NaipScene {
    year: u16,
    date: String,
    asset_url: String,
    gsd: f64,
    bbox: [f64; 4],
}

impl NaipScene {
    fn overlap_fraction(&self, target: &[f64; 4]) -> f64 {
        let ow = (self.bbox[2].min(target[2]) - self.bbox[0].max(target[0])).max(0.0);
        let oh = (self.bbox[3].min(target[3]) - self.bbox[1].max(target[1])).max(0.0);
        let overlap = ow * oh;
        let target_area = (target[2] - target[0]) * (target[3] - target[1]);
        if target_area > 0.0 { overlap / target_area } else { 0.0 }
    }
}

fn percentile(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() { return 0.0; }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Compute an adaptive canopy threshold using Otsu's method.
/// Finds the NDVI threshold that maximizes between-class variance.
fn otsu_threshold(values: &[f32]) -> f32 {
    let n = values.len() as f64;
    if n < 10.0 { return 0.3; }

    // Histogram with 200 bins over NDVI range [-1, 1]
    let bins = 200;
    let mut hist = vec![0u32; bins];
    for &v in values {
        let idx = ((v + 1.0) / 2.0 * (bins as f32 - 1.0)).clamp(0.0, bins as f32 - 1.0) as usize;
        hist[idx] += 1;
    }

    let total: f64 = hist.iter().map(|&h| h as f64).sum();
    let mut sum_total: f64 = hist.iter().enumerate()
        .map(|(i, &h)| i as f64 * h as f64).sum();

    let mut best_thresh = 0.3f32;
    let mut best_var = 0.0f64;
    let mut w0 = 0.0f64;
    let mut sum0 = 0.0f64;

    for i in 0..bins {
        w0 += hist[i] as f64;
        if w0 == 0.0 { continue; }
        let w1 = total - w0;
        if w1 == 0.0 { break; }

        sum0 += i as f64 * hist[i] as f64;
        let mean0 = sum0 / w0;
        let mean1 = (sum_total - sum0) / w1;
        let var = w0 * w1 * (mean0 - mean1).powi(2);

        if var > best_var {
            best_var = var;
            best_thresh = (i as f32 / (bins - 1) as f32) * 2.0 - 1.0;
        }
    }

    best_thresh
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Canopy loss analysis: Ghent, Norfolk VA");
    eprintln!("Bbox: {:.4}, {:.4} → {:.4}, {:.4}\n",
        GHENT_BBOX[0], GHENT_BBOX[1], GHENT_BBOX[2], GHENT_BBOX[3]);

    let scenes = search_naip_scenes(&GHENT_BBOX)?;
    eprintln!("Found {} NAIP scenes\n", scenes.len());

    // Pick best-overlapping scene per year
    let mut by_year: BTreeMap<u16, &NaipScene> = BTreeMap::new();
    for s in &scenes {
        let existing = by_year.get(&s.year);
        if existing.map_or(true, |prev| s.overlap_fraction(&GHENT_BBOX) > prev.overlap_fraction(&GHENT_BBOX)) {
            by_year.insert(s.year, s);
        }
    }

    // Compute grid size
    let [west, south, east, north] = GHENT_BBOX;
    let lat_mid = (south + north) / 2.0;
    let m_per_deg_lon = 111_320.0 * lat_mid.to_radians().cos();
    let m_per_deg_lat = 111_320.0;
    let width_m = (east - west) * m_per_deg_lon;
    let height_m = (north - south) * m_per_deg_lat;
    let nx = (width_m / PIXEL_SIZE_M).ceil() as usize;
    let ny = (height_m / PIXEL_SIZE_M).ceil() as usize;
    let total_pixels = nx * ny;
    let pixel_area_m2 = PIXEL_SIZE_M * PIXEL_SIZE_M;

    eprintln!("Output grid: {}×{} pixels ({:.0}×{:.0}m)\n", nx, ny, width_m, height_m);

    // Fetch NDVI for each year and compute adaptive thresholds
    let mut ndvi_maps: BTreeMap<u16, Vec<f32>> = BTreeMap::new();
    let mut thresholds: BTreeMap<u16, f32> = BTreeMap::new();

    for (&year, scene) in &by_year {
        eprint!("  {} ({:.0}% overlap) ... ", scene.date, scene.overlap_fraction(&GHENT_BBOX) * 100.0);
        match fetch_ndvi(&scene.asset_url, &scene.bbox, &GHENT_BBOX, nx, ny) {
            Ok(ndvi) => {
                let mut valid: Vec<f32> = ndvi.iter().copied().filter(|&v| v != NODATA).collect();
                if valid.is_empty() {
                    eprintln!("SKIPPED — no valid pixels");
                    continue;
                }
                valid.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let valid_frac = valid.len() as f64 / total_pixels as f64 * 100.0;
                let mean: f32 = valid.iter().sum::<f32>() / valid.len() as f32;
                let thresh = otsu_threshold(&valid);
                let canopy_px = valid.iter().filter(|&&v| v > thresh).count();
                let canopy_pct = canopy_px as f64 / valid.len() as f64 * 100.0;

                eprintln!("OK — {:.0}% valid, mean NDVI {:.3}, Otsu threshold {:.3}, canopy: {:.1}% ({} px)",
                    valid_frac, mean, thresh, canopy_pct, canopy_px);

                thresholds.insert(year, thresh);
                ndvi_maps.insert(year, ndvi);
            }
            Err(e) => eprintln!("FAILED: {}", e),
        }
    }

    if ndvi_maps.len() < 2 {
        return Err("Need at least 2 years of data for comparison".into());
    }

    // Classify each year's pixels as canopy/non-canopy using per-year threshold
    let available_years: Vec<u16> = ndvi_maps.keys().copied().collect();
    let mut canopy_masks: BTreeMap<u16, Vec<bool>> = BTreeMap::new();

    for &year in &available_years {
        let ndvi = &ndvi_maps[&year];
        let thresh = thresholds[&year];
        let mask: Vec<bool> = ndvi.iter().map(|&v| v != NODATA && v > thresh).collect();
        canopy_masks.insert(year, mask);
    }

    eprintln!("\n--- Canopy Change (Otsu-classified) ---\n");

    // Per-period analysis
    for pair in available_years.windows(2) {
        let (y0, y1) = (pair[0], pair[1]);
        let ndvi0 = &ndvi_maps[&y0];
        let ndvi1 = &ndvi_maps[&y1];
        let mask0 = &canopy_masks[&y0];
        let mask1 = &canopy_masks[&y1];

        let mut loss = 0usize;
        let mut gain = 0usize;
        let mut stable_canopy = 0usize;
        let mut valid = 0usize;

        for i in 0..total_pixels {
            if ndvi0[i] == NODATA || ndvi1[i] == NODATA { continue; }
            valid += 1;
            match (mask0[i], mask1[i]) {
                (true, false) => loss += 1,
                (false, true) => gain += 1,
                (true, true) => stable_canopy += 1,
                _ => {}
            }
        }

        eprintln!(
            "  {} → {}: loss={:.0}m² ({} px), gain={:.0}m² ({} px), stable={:.0}m², net={:+.0}m² [{}k valid]",
            y0, y1,
            loss as f64 * pixel_area_m2, loss,
            gain as f64 * pixel_area_m2, gain,
            stable_canopy as f64 * pixel_area_m2,
            (gain as i64 - loss as i64) as f64 * pixel_area_m2,
            valid / 1000,
        );
    }

    // Overall comparison: first to last
    let first_year = available_years[0];
    let last_year = *available_years.last().unwrap();

    let mut total_loss = 0usize;
    let mut total_gain = 0usize;
    let mut initial_canopy = 0usize;
    let mut final_canopy = 0usize;
    let mut stable_canopy = 0usize;
    let mut valid_both = 0usize;

    let mask_first = &canopy_masks[&first_year];
    let mask_last = &canopy_masks[&last_year];
    let ndvi_first = &ndvi_maps[&first_year];
    let ndvi_last = &ndvi_maps[&last_year];

    for i in 0..total_pixels {
        if ndvi_first[i] == NODATA || ndvi_last[i] == NODATA { continue; }
        valid_both += 1;
        if mask_first[i] { initial_canopy += 1; }
        if mask_last[i] { final_canopy += 1; }
        match (mask_first[i], mask_last[i]) {
            (true, false) => total_loss += 1,
            (false, true) => total_gain += 1,
            (true, true) => stable_canopy += 1,
            _ => {}
        }
    }

    eprintln!("\n--- Summary: {} → {} ({} pixels valid in both years) ---\n", first_year, last_year, valid_both);
    eprintln!("  Otsu threshold {}: {:.3}", first_year, thresholds[&first_year]);
    eprintln!("  Otsu threshold {}: {:.3}", last_year, thresholds[&last_year]);
    eprintln!();
    eprintln!("  Initial canopy ({}): {:.1}% ({:.0}m²)",
        first_year,
        initial_canopy as f64 / valid_both as f64 * 100.0,
        initial_canopy as f64 * pixel_area_m2);
    eprintln!("  Final canopy ({}):   {:.1}% ({:.0}m²)",
        last_year,
        final_canopy as f64 / valid_both as f64 * 100.0,
        final_canopy as f64 * pixel_area_m2);
    eprintln!("  Stable canopy:  {:.1}% ({:.0}m²)",
        stable_canopy as f64 / valid_both as f64 * 100.0,
        stable_canopy as f64 * pixel_area_m2);
    eprintln!("  Canopy lost:    {:.1}% ({:.0}m²)",
        total_loss as f64 / valid_both as f64 * 100.0,
        total_loss as f64 * pixel_area_m2);
    eprintln!("  Canopy gained:  {:.1}% ({:.0}m²)",
        total_gain as f64 / valid_both as f64 * 100.0,
        total_gain as f64 * pixel_area_m2);
    if initial_canopy > 0 {
        let net = final_canopy as i64 - initial_canopy as i64;
        eprintln!("  Net change:     {:+.0}m² ({:+.1}%)",
            net as f64 * pixel_area_m2,
            net as f64 / initial_canopy as f64 * 100.0);
    }

    // Write outputs
    write_change_map(mask_first, mask_last, ndvi_first, ndvi_last, nx, ny, first_year, last_year)?;
    for (&year, ndvi) in &ndvi_maps {
        write_ndvi_map(ndvi, nx, ny, year)?;
    }
    write_canopy_comparison(&canopy_masks, &ndvi_maps, nx, ny)?;

    // Generate true-color aerial imagery for key years
    eprintln!();
    for &year in &[first_year, last_year] {
        if let Some(scene) = by_year.get(&year) {
            eprint!("Fetching RGB {} ... ", year);
            match fetch_rgba(&scene.asset_url, &scene.bbox, &GHENT_BBOX, nx, ny) {
                Ok(bands) => {
                    write_rgb_image(&bands, nx, ny, year)?;
                    write_cir_image(&bands, nx, ny, year)?;
                    eprintln!("OK");
                }
                Err(e) => eprintln!("FAILED: {}", e),
            }
        }
    }

    eprintln!("\nDone.");
    Ok(())
}

fn search_naip_scenes(bbox: &[f64; 4]) -> Result<Vec<NaipScene>, Box<dyn std::error::Error>> {
    let [west, south, east, north] = *bbox;
    let body = format!(
        r#"{{"collections":["naip"],"bbox":[{},{},{},{}],"limit":50}}"#,
        west, south, east, north
    );

    let resp = ureq::post("https://planetarycomputer.microsoft.com/api/stac/v1/search")
        .set("Content-Type", "application/json")
        .send_string(&body)?
        .into_string()?;

    let mut scenes = Vec::new();
    for feature in resp.split("\"type\":\"Feature\"").skip(1) {
        let date = extract_json_string(feature, "\"datetime\":\"").unwrap_or_default();
        let year: u16 = date.get(..4).and_then(|s| s.parse().ok()).unwrap_or(0);
        if year == 0 { continue; }
        let gsd: f64 = extract_json_number(feature, "\"gsd\":").unwrap_or(1.0);
        let asset_url = extract_naip_href(feature).unwrap_or_default();
        if asset_url.is_empty() { continue; }
        let scene_bbox = extract_bbox(feature).unwrap_or([west, south, east, north]);

        scenes.push(NaipScene {
            year, date: date.chars().take(10).collect(), asset_url, gsd, bbox: scene_bbox,
        });
    }
    scenes.sort_by_key(|s| s.year);
    Ok(scenes)
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let start = json.find(key)? + key.len();
    let rest = &json[start..];
    Some(rest[..rest.find('"')?].to_string())
}

fn extract_json_number(json: &str, key: &str) -> Option<f64> {
    let start = json.find(key)? + key.len();
    let rest = json[start..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-').unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn extract_naip_href(feature: &str) -> Option<String> {
    let assets = &feature[feature.find("\"assets\"")?..];
    let image = &assets[assets.find("\"image\"")?..];
    let start = image.find("\"href\":\"")? + 8;
    let rest = &image[start..];
    Some(rest[..rest.find('"')?].to_string())
}

fn extract_bbox(feature: &str) -> Option<[f64; 4]> {
    let start = feature.find("\"bbox\":[")? + 8;
    let rest = &feature[start..];
    let nums: Vec<f64> = rest[..rest.find(']')?]
        .split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if nums.len() >= 4 { Some([nums[0], nums[1], nums[2], nums[3]]) } else { None }
}

/// Compute the raster read window and output placement for a target bbox.
///
/// Transforms the full target bbox to raster pixel coordinates, clamps to valid
/// raster extent, and computes where the clamped region maps in the output grid.
/// This guarantees pixel-perfect alignment across scenes regardless of their extent.
struct ReadWindow {
    /// Raster pixel offset (x, y) to start reading from
    x_off: isize,
    y_off: isize,
    /// Raster pixels to read (width, height)
    x_size: usize,
    y_size: usize,
    /// Output grid offset and size for the read region
    out_x0: usize,
    out_y0: usize,
    out_nx: usize,
    out_ny: usize,
}

fn compute_read_window(
    ds: &Dataset,
    target_bbox: &[f64; 4],
    nx: usize,
    ny: usize,
) -> Result<ReadWindow, Box<dyn std::error::Error>> {
    let [west, south, east, north] = *target_bbox;
    let gt = ds.geo_transform()?;

    let raster_srs = ds.spatial_ref()?;
    let mut wgs84 = SpatialRef::from_epsg(4326)?;
    wgs84.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let transform = CoordTransform::new(&wgs84, &raster_srs)?;

    // Transform FULL target bbox to raster CRS
    let mut xs = [west, east];
    let mut ys = [south, north];
    let mut zs = [0.0, 0.0];
    transform.transform_coords(&mut xs, &mut ys, &mut zs)?;

    // Target bbox in raster pixel space (may extend outside raster)
    let tgt_px_left = ((xs[0] - gt[0]) / gt[1]).round() as isize;
    let tgt_px_top = ((ys[1] - gt[3]) / gt[5]).round() as isize;
    let tgt_px_right = ((xs[1] - gt[0]) / gt[1]).round() as isize;
    let tgt_px_bottom = ((ys[0] - gt[3]) / gt[5]).round() as isize;

    let tgt_px_w = (tgt_px_right - tgt_px_left).max(1) as usize;
    let tgt_px_h = (tgt_px_bottom - tgt_px_top).max(1) as usize;

    // Clamp to actual raster extent
    let (raster_w, raster_h) = ds.raster_size();
    let x_off = tgt_px_left.max(0).min(raster_w as isize - 1);
    let y_off = tgt_px_top.max(0).min(raster_h as isize - 1);
    let x_end = tgt_px_right.max(0).min(raster_w as isize);
    let y_end = tgt_px_bottom.max(0).min(raster_h as isize);

    let x_size = (x_end - x_off).max(1) as usize;
    let y_size = (y_end - y_off).max(1) as usize;

    // Where in the output grid does the clamped region land?
    // Map raster pixel coords back to output grid coords via the target bbox span.
    let out_x0 = ((x_off - tgt_px_left).max(0) as f64 / tgt_px_w as f64 * nx as f64).round() as usize;
    let out_y0 = ((y_off - tgt_px_top).max(0) as f64 / tgt_px_h as f64 * ny as f64).round() as usize;
    let out_x1 = ((x_end - tgt_px_left) as f64 / tgt_px_w as f64 * nx as f64).round() as usize;
    let out_y1 = ((y_end - tgt_px_top) as f64 / tgt_px_h as f64 * ny as f64).round() as usize;

    Ok(ReadWindow {
        x_off,
        y_off,
        x_size,
        y_size,
        out_x0: out_x0.min(nx),
        out_y0: out_y0.min(ny),
        out_nx: (out_x1.saturating_sub(out_x0)).max(1).min(nx - out_x0.min(nx)),
        out_ny: (out_y1.saturating_sub(out_y0)).max(1).min(ny - out_y0.min(ny)),
    })
}

/// Fetch all 4 NAIP bands (RGBNIR). Returns [4][ny*nx] u8 arrays.
fn fetch_rgba(
    url: &str, _scene_bbox: &[f64; 4], target_bbox: &[f64; 4], nx: usize, ny: usize,
) -> Result<[Vec<u8>; 4], Box<dyn std::error::Error>> {
    let vsicurl = format!("/vsicurl/{}", url);
    let ds = Dataset::open(&vsicurl)?;
    let w = compute_read_window(&ds, target_bbox, nx, ny)?;

    let mut bands = [
        vec![0u8; nx * ny], vec![0u8; nx * ny],
        vec![0u8; nx * ny], vec![0u8; nx * ny],
    ];

    for b in 0..4 {
        let band = ds.rasterband(b + 1)?;
        let buf = band.read_as::<u8>(
            (w.x_off, w.y_off), (w.x_size, w.y_size), (w.out_nx, w.out_ny),
            Some(ResampleAlg::Bilinear),
        )?;
        let data = buf.data();
        for oy in 0..w.out_ny {
            for ox in 0..w.out_nx {
                let dx = w.out_x0 + ox;
                let dy = w.out_y0 + oy;
                if dx < nx && dy < ny {
                    bands[b][dy * nx + dx] = data[oy * w.out_nx + ox];
                }
            }
        }
    }

    Ok(bands)
}

fn fetch_ndvi(
    url: &str, _scene_bbox: &[f64; 4], target_bbox: &[f64; 4], nx: usize, ny: usize,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let vsicurl = format!("/vsicurl/{}", url);
    let ds = Dataset::open(&vsicurl)?;
    let w = compute_read_window(&ds, target_bbox, nx, ny)?;

    let red_band = ds.rasterband(1)?;
    let nir_band = ds.rasterband(4)?;

    let red_buf = red_band.read_as::<u8>(
        (w.x_off, w.y_off), (w.x_size, w.y_size), (w.out_nx, w.out_ny),
        Some(ResampleAlg::Bilinear),
    )?;
    let nir_buf = nir_band.read_as::<u8>(
        (w.x_off, w.y_off), (w.x_size, w.y_size), (w.out_nx, w.out_ny),
        Some(ResampleAlg::Bilinear),
    )?;

    let red = red_buf.data();
    let nir = nir_buf.data();

    let mut ndvi = vec![NODATA; nx * ny];
    for oy in 0..w.out_ny {
        for ox in 0..w.out_nx {
            let si = oy * w.out_nx + ox;
            let dx = w.out_x0 + ox;
            let dy = w.out_y0 + oy;
            if dx >= nx || dy >= ny { continue; }
            let r = red[si] as f32;
            let n = nir[si] as f32;
            let sum = r + n;
            if sum < 10.0 { continue; }
            ndvi[dy * nx + dx] = (n - r) / sum;
        }
    }

    Ok(ndvi)
}

fn write_change_map(
    mask_first: &[bool], mask_last: &[bool],
    ndvi_first: &[f32], ndvi_last: &[f32],
    nx: usize, ny: usize,
    year_first: u16, year_last: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let path = format!("canopy_change_{}_{}.ppm", year_first, year_last);
    let mut f = std::fs::File::create(&path)?;
    writeln!(f, "P6")?;
    writeln!(f, "{} {}", nx, ny)?;
    writeln!(f, "255")?;

    for i in 0..nx * ny {
        let (r, g, b) = if ndvi_first[i] == NODATA || ndvi_last[i] == NODATA {
            (40u8, 40u8, 40u8)
        } else {
            match (mask_first[i], mask_last[i]) {
                (true, false) => (220, 50, 50),   // canopy lost — red
                (false, true) => (50, 80, 220),    // canopy gained — blue
                (true, true) => (40, 180, 50),     // stable canopy — green
                (false, false) => (110, 110, 110), // non-canopy — gray
            }
        };
        f.write_all(&[r, g, b])?;
    }

    eprintln!("\nWrote: {}", path);
    Ok(())
}

fn write_ndvi_map(ndvi: &[f32], nx: usize, ny: usize, year: u16) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let path = format!("ndvi_{}.pgm", year);
    let mut f = std::fs::File::create(&path)?;
    writeln!(f, "P5")?;
    writeln!(f, "{} {}", nx, ny)?;
    writeln!(f, "255")?;
    let pixels: Vec<u8> = ndvi.iter().map(|&v| {
        if v == NODATA { 0 } else { ((v + 1.0) / 2.0 * 255.0).clamp(0.0, 255.0) as u8 }
    }).collect();
    f.write_all(&pixels)?;
    eprintln!("Wrote: {}", path);
    Ok(())
}

/// Write a side-by-side canopy comparison for first and last year.
fn write_canopy_comparison(
    masks: &BTreeMap<u16, Vec<bool>>,
    ndvis: &BTreeMap<u16, Vec<f32>>,
    nx: usize, ny: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let years: Vec<u16> = masks.keys().copied().collect();
    if years.len() < 2 { return Ok(()); }

    let first = years[0];
    let last = *years.last().unwrap();

    // Side-by-side PPM: first year | last year
    let path = format!("canopy_sidebyside_{}_{}.ppm", first, last);
    let mut f = std::fs::File::create(&path)?;
    let gap = 4; // pixel gap between panels
    let total_w = nx * 2 + gap;
    writeln!(f, "P6")?;
    writeln!(f, "{} {}", total_w, ny)?;
    writeln!(f, "255")?;

    let m0 = &masks[&first];
    let n0 = &ndvis[&first];
    let m1 = &masks[&last];
    let n1 = &ndvis[&last];

    for y in 0..ny {
        // Left panel: first year
        for x in 0..nx {
            let i = y * nx + x;
            let (r, g, b) = if n0[i] == NODATA {
                (40, 40, 40)
            } else if m0[i] {
                (40, 180, 50)  // canopy
            } else {
                (110, 110, 110) // non-canopy
            };
            f.write_all(&[r, g, b])?;
        }
        // Gap
        for _ in 0..gap {
            f.write_all(&[20, 20, 20])?;
        }
        // Right panel: last year
        for x in 0..nx {
            let i = y * nx + x;
            let (r, g, b) = if n1[i] == NODATA {
                (40, 40, 40)
            } else if m1[i] {
                (40, 180, 50)
            } else {
                (110, 110, 110)
            };
            f.write_all(&[r, g, b])?;
        }
    }

    eprintln!("Wrote: {} (left={}, right={})", path, first, last);
    Ok(())
}

/// Write true-color RGB image from NAIP bands.
fn write_rgb_image(bands: &[Vec<u8>; 4], nx: usize, ny: usize, year: u16) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let path = format!("rgb_{}.ppm", year);
    let mut f = std::fs::File::create(&path)?;
    writeln!(f, "P6")?;
    writeln!(f, "{} {}", nx, ny)?;
    writeln!(f, "255")?;
    for i in 0..nx * ny {
        f.write_all(&[bands[0][i], bands[1][i], bands[2][i]])?;
    }
    eprintln!("  Wrote: {}", path);
    Ok(())
}

/// Write Color InfraRed (CIR) composite: NIR→R, R→G, G→B.
/// Vegetation appears bright red/magenta, making canopy pop visually.
fn write_cir_image(bands: &[Vec<u8>; 4], nx: usize, ny: usize, year: u16) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let path = format!("cir_{}.ppm", year);
    let mut f = std::fs::File::create(&path)?;
    writeln!(f, "P6")?;
    writeln!(f, "{} {}", nx, ny)?;
    writeln!(f, "255")?;
    for i in 0..nx * ny {
        // CIR: NIR→R, Red→G, Green→B
        f.write_all(&[bands[3][i], bands[0][i], bands[1][i]])?;
    }
    eprintln!("  Wrote: {}", path);
    Ok(())
}
