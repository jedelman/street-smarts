//! Tile fetcher for 9-channel semantic rasters.
//!
//! Assembles training tiles from multiple open data sources via GDAL's
//! /vsicurl/ virtual filesystem (Cloud-Optimized GeoTIFF reads over HTTP).
//!
//! Channel layout:
//!   0: building footprint mask   (MS Building Footprints → rasterized)
//!   1: building height           (3DEP HAG, normalized)
//!   2: terrain elevation         (3DEP DTM, local-detrended)
//!   3: NDVI                      (computed from NAIP NIR/Red)
//!   4: impervious surface        (NLCD impervious %)
//!   5: water mask                (NLCD class 11)
//!   6: road network mask         (OSM)
//!   7: parcel boundary mask      (OSM / county)
//!   8: land-cover class          (NLCD Anderson Level II, normalized)

/// A 128×128 semantic raster tile with 9 channels.
pub struct SemanticTile {
    /// Channel data in CHW order: [9, 128, 128]
    pub data: Vec<f32>,
    /// Geographic bounds (west, south, east, north) in EPSG:4326
    pub bounds: [f64; 4],
    /// Which channels were successfully populated.
    pub channels_present: [bool; 9],
}

impl SemanticTile {
    pub const CHANNELS: usize = 9;
    pub const SIZE: usize = 128;
    pub const PIXELS: usize = Self::SIZE * Self::SIZE;
    pub const NUMEL: usize = Self::CHANNELS * Self::PIXELS;

    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Get a single channel as a slice.
    pub fn channel(&self, idx: usize) -> &[f32] {
        let start = idx * Self::PIXELS;
        &self.data[start..start + Self::PIXELS]
    }
}

/// Norfolk, VA bounding box (Military Circle / Eastside Commons area).
pub const NORFOLK_BBOX: [f64; 4] = [-76.22, 36.855, -76.18, 36.885];

/// Channel names for logging.
pub const CHANNEL_NAMES: [&str; 9] = [
    "buildings", "height", "terrain", "ndvi", "impervious",
    "water", "roads", "parcels", "landcover",
];

#[derive(Debug, Clone)]
pub struct TileSpec {
    pub lon: f64,
    pub lat: f64,
    pub size_m: f64,
}

impl TileSpec {
    /// Compute geographic bounds from center + size.
    pub fn bounds(&self) -> [f64; 4] {
        let half = self.size_m / 2.0;
        let m_per_deg_lat = 111_320.0;
        let m_per_deg_lon = 111_320.0 * self.lat.to_radians().cos();
        [
            self.lon - half / m_per_deg_lon,
            self.lat - half / m_per_deg_lat,
            self.lon + half / m_per_deg_lon,
            self.lat + half / m_per_deg_lat,
        ]
    }
}

/// Generate a grid of tile specs covering a bounding box.
pub fn tile_grid(bbox: [f64; 4], tile_size_m: f64, stride_m: f64) -> Vec<TileSpec> {
    let [west, south, east, north] = bbox;
    let lat_mid = (south + north) / 2.0;
    let m_per_deg_lat = 111_320.0;
    let m_per_deg_lon = 111_320.0 * lat_mid.to_radians().cos();

    let width_m = (east - west) * m_per_deg_lon;
    let height_m = (north - south) * m_per_deg_lat;

    let nx = ((width_m - tile_size_m) / stride_m).floor() as usize + 1;
    let ny = ((height_m - tile_size_m) / stride_m).floor() as usize + 1;

    let mut specs = Vec::with_capacity(nx * ny);
    for iy in 0..ny {
        for ix in 0..nx {
            let cx_m = tile_size_m / 2.0 + ix as f64 * stride_m;
            let cy_m = tile_size_m / 2.0 + iy as f64 * stride_m;
            specs.push(TileSpec {
                lon: west + cx_m / m_per_deg_lon,
                lat: south + cy_m / m_per_deg_lat,
                size_m: tile_size_m,
            });
        }
    }
    specs
}

/// Data source URLs for a region. Tokens are ephemeral (Planetary Computer SAS).
pub struct DataSources {
    pub naip_url: String,
    pub dem_url: String,
    pub overpass_endpoint: String,
}

impl DataSources {
    /// Get a SAS token from Planetary Computer for a collection.
    fn pc_token(collection: &str) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!(
            "https://planetarycomputer.microsoft.com/api/sas/v1/token/{}",
            collection
        );
        let resp = ureq::get(&url).call()?.into_string()?;
        // Parse {"msft:expiry":"...","token":"..."}
        let token = resp.split("\"token\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .ok_or("failed to parse SAS token")?;
        Ok(token.to_string())
    }

    /// Build data sources for Norfolk with fresh SAS tokens.
    pub fn norfolk() -> Result<Self, Box<dyn std::error::Error>> {
        let dem_token = Self::pc_token("3dep-seamless")?;
        Ok(Self {
            naip_url: "https://naipeuwest.blob.core.windows.net/naip/v002/va/2023/va_060cm_2023/36076/m_3607631_sw_18_060_20231009_20240103.tif".into(),
            dem_url: format!(
                "https://ai4edataeuwest.blob.core.windows.net/3dep/Elevation/13/TIFF/n37w077/USGS_13_n37w077.tif?{}",
                dem_token
            ),
            overpass_endpoint: "https://overpass.kumi.systems/api/interpreter".into(),
        })
    }
}

/// Assemble a full 9-channel semantic tile.
/// Channels that can't be fetched are left as zeros with `channels_present` = false.
#[cfg(feature = "gdal")]
pub fn fetch_tile(
    sources: &DataSources,
    spec: &TileSpec,
) -> Result<SemanticTile, Box<dyn std::error::Error>> {
    let bounds = spec.bounds();
    let mut data = vec![0.0f32; SemanticTile::NUMEL];
    let mut present = [false; 9];
    let px = SemanticTile::PIXELS;

    // Channel 3 (NDVI) from NAIP
    match read_cog_window(&sources.naip_url, &bounds, SemanticTile::SIZE, 4) {
        Ok(naip) => {
            let red = &naip[0..px];
            let nir = &naip[3 * px..4 * px];
            for i in 0..px {
                let sum = nir[i] + red[i];
                let ndvi = if sum > 0.001 { (nir[i] - red[i]) / sum } else { 0.0 };
                data[3 * px + i] = (ndvi + 1.0) / 2.0;
            }
            present[3] = true;
        }
        Err(e) => eprintln!("    NAIP failed: {}", e),
    }

    // Channel 2: terrain elevation from 3DEP
    match read_cog_window(&sources.dem_url, &bounds, SemanticTile::SIZE, 1) {
        Ok(dem) => {
            let mean: f32 = dem.iter().sum::<f32>() / px as f32;
            let max_dev = dem.iter().map(|v| (v - mean).abs()).fold(0.0f32, f32::max).max(1.0);
            for i in 0..px {
                data[2 * px + i] = ((dem[i] - mean) / max_dev + 1.0) / 2.0;
            }
            present[2] = true;
        }
        Err(e) => eprintln!("    3DEP failed: {}", e),
    }

    // Channel 6: roads from OSM
    match fetch_osm_roads(&sources.overpass_endpoint, &bounds, SemanticTile::SIZE) {
        Ok(roads) => {
            data[6 * px..7 * px].copy_from_slice(&roads);
            present[6] = true;
        }
        Err(e) => eprintln!("    OSM roads failed: {}", e),
    }

    // Channel 0: buildings from OSM
    match fetch_osm_polygons(&sources.overpass_endpoint, &bounds, SemanticTile::SIZE, "building") {
        Ok(mask) => {
            data[0..px].copy_from_slice(&mask);
            present[0] = true;
        }
        Err(e) => eprintln!("    OSM buildings failed: {}", e),
    }

    // Channel 5: water from OSM
    match fetch_osm_polygons(&sources.overpass_endpoint, &bounds, SemanticTile::SIZE, "natural=water") {
        Ok(mask) => {
            data[5 * px..6 * px].copy_from_slice(&mask);
            present[5] = true;
        }
        Err(e) => eprintln!("    OSM water failed: {}", e),
    }

    // Channels 1, 4, 7, 8: not yet implemented — remain zeros

    Ok(SemanticTile {
        data,
        bounds,
        channels_present: present,
    })
}

/// Read a geographic window from a Cloud-Optimized GeoTIFF via /vsicurl/.
/// Handles CRS reprojection (e.g. NAIP in UTM, bounds in WGS84).
/// Returns [bands * size * size] f32 data.
#[cfg(feature = "gdal")]
fn read_cog_window(
    url: &str,
    bounds: &[f64; 4],
    out_size: usize,
    num_bands: usize,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    use gdal::Dataset;
    use gdal::spatial_ref::SpatialRef;
    use gdal::spatial_ref::CoordTransform;
    use gdal::raster::ResampleAlg;

    let [west, south, east, north] = *bounds;
    let vsicurl = format!("/vsicurl/{}", url);
    let ds = Dataset::open(&vsicurl)?;
    let gt = ds.geo_transform()?;

    // Get the raster's CRS and set up reprojection from WGS84
    let raster_srs = ds.spatial_ref()?;
    let mut wgs84 = SpatialRef::from_epsg(4326)?;
    // Force traditional (lon, lat) axis order for WGS84
    wgs84.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);

    // Transform corner coordinates from WGS84 (lon, lat) to raster CRS
    let transform = CoordTransform::new(&wgs84, &raster_srs)?;
    let mut xs = [west, east];   // longitudes
    let mut ys = [south, north]; // latitudes
    let mut zs = [0.0, 0.0];
    transform.transform_coords(&mut xs, &mut ys, &mut zs)?;
    let (r_west, r_east) = (xs[0], xs[1]);
    let (r_south, r_north) = (ys[0], ys[1]);

    // Convert reprojected bounds to pixel coordinates
    let px_left = ((r_west - gt[0]) / gt[1]) as isize;
    let px_top = ((r_north - gt[3]) / gt[5]) as isize;
    let px_right = ((r_east - gt[0]) / gt[1]) as isize;
    let px_bottom = ((r_south - gt[3]) / gt[5]) as isize;

    let x_off = px_left.max(0);
    let y_off = px_top.max(0);
    let x_size = (px_right - px_left).unsigned_abs().max(1);
    let y_size = (px_bottom - px_top).unsigned_abs().max(1);

    // Bounds check
    let (raster_w, raster_h) = ds.raster_size();
    if x_off as usize >= raster_w || y_off as usize >= raster_h {
        return Err(format!(
            "tile outside raster: pixel ({}, {}) vs raster {}×{}",
            x_off, y_off, raster_w, raster_h
        ).into());
    }
    let x_size = x_size.min(raster_w - x_off as usize);
    let y_size = y_size.min(raster_h - y_off as usize);

    let mut result = Vec::with_capacity(num_bands * out_size * out_size);
    let bands_in_file = ds.raster_count() as usize;

    for band_idx in 1..=num_bands.min(bands_in_file) {
        let band = ds.rasterband(band_idx)?;

        // Try reading as f32 first (elevation data), fall back to u8 (imagery)
        match band.read_as::<f32>(
            (x_off, y_off),
            (x_size, y_size),
            (out_size, out_size),
            Some(ResampleAlg::Bilinear),
        ) {
            Ok(buf) => {
                result.extend_from_slice(buf.data());
            }
            Err(_) => {
                let buf = band.read_as::<u8>(
                    (x_off, y_off),
                    (x_size, y_size),
                    (out_size, out_size),
                    Some(ResampleAlg::Bilinear),
                )?;
                for &v in buf.data() {
                    result.push(v as f32 / 255.0);
                }
            }
        }
    }

    Ok(result)
}

/// Fetch OSM road geometries and rasterize them into a [size × size] mask.
fn fetch_osm_roads(
    endpoint: &str,
    bounds: &[f64; 4],
    size: usize,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let [west, south, east, north] = *bounds;
    let query = format!(
        "[out:json][bbox:{},{},{},{}];way[\"highway\"];out geom;",
        south, west, north, east
    );

    let resp = ureq::post(endpoint)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&format!("data={}", query))?
        .into_string()?;

    // Simple rasterization: for each way's geometry, draw lines on the grid
    let mut mask = vec![0.0f32; size * size];

    // Parse way geometries from JSON (minimal parsing, no serde)
    for line in resp.lines() {
        // Look for "lat": and "lon": pairs in geometry arrays
        if let (Some(lat_start), Some(lon_start)) = (line.find("\"lat\":"), line.find("\"lon\":")) {
            let _ = lat_start; let _ = lon_start; // avoid unused warning
        }
    }

    // Parse geometry from the JSON more carefully
    // Each element has "geometry": [{"lat": ..., "lon": ...}, ...]
    let mut points: Vec<(f64, f64)> = Vec::new();

    for segment in resp.split("\"geometry\"") {
        if segment.contains("\"lat\"") {
            points.clear();
            // Extract lat/lon pairs
            for lat_chunk in segment.split("\"lat\":").skip(1) {
                let lat: f64 = lat_chunk.split([',', '}'].as_ref())
                    .next()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);
                // Find the corresponding lon
                if let Some(lon_chunk) = lat_chunk.split("\"lon\":").nth(1) {
                    let lon: f64 = lon_chunk.split([',', '}'].as_ref())
                        .next()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0.0);
                    points.push((lon, lat));
                }
            }
            // Rasterize line segments
            for pair in points.windows(2) {
                draw_line(&mut mask, size, bounds, pair[0], pair[1]);
            }
        }
    }

    Ok(mask)
}

/// Draw a line between two geographic points on a pixel grid.
fn draw_line(
    mask: &mut [f32],
    size: usize,
    bounds: &[f64; 4],
    p0: (f64, f64),
    p1: (f64, f64),
) {
    let [west, south, east, north] = *bounds;
    let w = east - west;
    let h = north - south;
    if w <= 0.0 || h <= 0.0 { return; }

    // Convert to pixel coords
    let x0 = ((p0.0 - west) / w * size as f64) as i32;
    let y0 = ((north - p0.1) / h * size as f64) as i32;
    let x1 = ((p1.0 - west) / w * size as f64) as i32;
    let y1 = ((north - p1.1) / h * size as f64) as i32;

    // Bresenham's line
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < size as i32 && y >= 0 && y < size as i32 {
            mask[y as usize * size + x as usize] = 1.0;
        }
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

/// Fetch OSM polygon features and rasterize them as filled masks.
/// `tag` can be "building" or "natural=water" etc.
fn fetch_osm_polygons(
    endpoint: &str,
    bounds: &[f64; 4],
    size: usize,
    tag: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let [west, south, east, north] = *bounds;
    // Build Overpass query for ways with this tag
    let tag_filter = if tag.contains('=') {
        format!("[\"{}\"]", tag.replace('=', "\"=\""))
    } else {
        format!("[\"{}\"]", tag)
    };
    let query = format!(
        "[out:json][bbox:{},{},{},{}];way{};out geom;",
        south, west, north, east, tag_filter
    );

    let resp = ureq::post(endpoint)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&format!("data={}", query))?
        .into_string()?;

    let mut mask = vec![0.0f32; size * size];

    // Parse polygons and fill them
    for segment in resp.split("\"geometry\"") {
        if !segment.contains("\"lat\"") { continue; }
        let mut points: Vec<(f64, f64)> = Vec::new();
        for lat_chunk in segment.split("\"lat\":").skip(1) {
            let lat: f64 = lat_chunk.split([',', '}'].as_ref())
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0.0);
            if let Some(lon_chunk) = lat_chunk.split("\"lon\":").nth(1) {
                let lon: f64 = lon_chunk.split([',', '}'].as_ref())
                    .next()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0.0);
                points.push((lon, lat));
            }
        }
        if points.len() >= 3 {
            fill_polygon(&mut mask, size, bounds, &points);
        }
    }

    Ok(mask)
}

/// Scanline fill a polygon on a pixel grid.
fn fill_polygon(
    mask: &mut [f32],
    size: usize,
    bounds: &[f64; 4],
    points: &[(f64, f64)],
) {
    let [west, south, east, north] = *bounds;
    let w = east - west;
    let h = north - south;
    if w <= 0.0 || h <= 0.0 || points.len() < 3 { return; }

    // Convert to pixel coords
    let px_points: Vec<(f64, f64)> = points.iter().map(|&(lon, lat)| {
        ((lon - west) / w * size as f64, (north - lat) / h * size as f64)
    }).collect();

    // Find y range
    let min_y = px_points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).max(0.0) as usize;
    let max_y = px_points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max).min(size as f64 - 1.0) as usize;

    // Scanline fill
    for y in min_y..=max_y {
        let yf = y as f64 + 0.5;
        let mut intersections: Vec<f64> = Vec::new();
        let n = px_points.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let (_, y0) = px_points[i];
            let (_, y1) = px_points[j];
            if (y0 <= yf && y1 > yf) || (y1 <= yf && y0 > yf) {
                let (x0, _) = px_points[i];
                let (x1, _) = px_points[j];
                let x = x0 + (yf - y0) / (y1 - y0) * (x1 - x0);
                intersections.push(x);
            }
        }
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in intersections.chunks(2) {
            if pair.len() == 2 {
                let x_start = (pair[0].max(0.0) as usize).min(size - 1);
                let x_end = (pair[1].min(size as f64 - 1.0) as usize).min(size - 1);
                for x in x_start..=x_end {
                    mask[y * size + x] = 1.0;
                }
            }
        }
    }
}
