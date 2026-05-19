//! Tile fetcher for semantic raster channels.
//!
//! Reads Cloud-Optimized GeoTIFFs via GDAL's /vsicurl/ virtual filesystem,
//! extracts 128×128 windows, and assembles 9-channel training tiles.
//!
//! Data sources (all public, no API keys):
//! - NAIP 4-band (RGBIR) via Microsoft Planetary Computer
//! - NLCD land cover + impervious via USGS
//! - 3DEP elevation via Microsoft Planetary Computer
//! - MS Building Footprints (GeoJSON → rasterized)
//! - OSM roads (Overpass API → rasterized)

use std::path::Path;

/// A 128×128 semantic raster tile with 9 channels.
pub struct SemanticTile {
    /// Channel data in CHW order: [9, 128, 128]
    pub data: Vec<f32>,
    /// Geographic bounds (west, south, east, north) in EPSG:4326
    pub bounds: [f64; 4],
}

impl SemanticTile {
    pub const CHANNELS: usize = 9;
    pub const SIZE: usize = 128;
    pub const NUMEL: usize = Self::CHANNELS * Self::SIZE * Self::SIZE;

    /// Convert to a flat f32 slice for tensor construction.
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }
}

/// Norfolk, VA bounding box (Military Circle / Eastside Commons area).
pub const NORFOLK_BBOX: [f64; 4] = [-76.22, 36.855, -76.18, 36.885];

/// Tile specification: a geographic window to extract.
#[derive(Debug, Clone)]
pub struct TileSpec {
    /// Center longitude (EPSG:4326)
    pub lon: f64,
    /// Center latitude (EPSG:4326)
    pub lat: f64,
    /// Tile size in meters (default: 128m for ~1m/pixel at 128px)
    pub size_m: f64,
}

/// Generate a grid of tile specs covering a bounding box.
pub fn tile_grid(bbox: [f64; 4], tile_size_m: f64, stride_m: f64) -> Vec<TileSpec> {
    let [west, south, east, north] = bbox;
    // Approximate meters per degree at this latitude
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
            let lon = west + cx_m / m_per_deg_lon;
            let lat = south + cy_m / m_per_deg_lat;
            specs.push(TileSpec {
                lon,
                lat,
                size_m: tile_size_m,
            });
        }
    }
    specs
}

/// Fetch NAIP RGBIR for a tile spec. Returns [4, 128, 128] f32 data
/// (R, G, B, NIR normalized to [0, 1]).
///
/// Uses GDAL to read a window from the COG over HTTP.
#[cfg(feature = "gdal")]
pub fn fetch_naip_tile(
    naip_url: &str,
    spec: &TileSpec,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    use gdal::Dataset;
    use gdal::raster::ResampleAlg;

    let half = spec.size_m / 2.0;
    let lat_mid = spec.lat;
    let m_per_deg_lat = 111_320.0;
    let m_per_deg_lon = 111_320.0 * lat_mid.to_radians().cos();

    let west = spec.lon - half / m_per_deg_lon;
    let south = spec.lat - half / m_per_deg_lat;
    let east = spec.lon + half / m_per_deg_lon;
    let north = spec.lat + half / m_per_deg_lat;

    let vsicurl = format!("/vsicurl/{}", naip_url);
    let ds = Dataset::open(&vsicurl)?;
    let gt = ds.geo_transform()?;

    // Convert geographic bounds to pixel coordinates
    // gt: [origin_x, pixel_width, 0, origin_y, 0, pixel_height (negative)]
    let px_west = ((west - gt[0]) / gt[1]) as isize;
    let px_north = ((north - gt[3]) / gt[5]) as isize;
    let px_east = ((east - gt[0]) / gt[1]) as isize;
    let px_south = ((south - gt[3]) / gt[5]) as isize;

    let x_off = px_west.max(0) as usize;
    let y_off = px_north.max(0) as usize;
    let x_size = (px_east - px_west).unsigned_abs();
    let y_size = (px_south - px_north).unsigned_abs();

    let out_size = SemanticTile::SIZE;
    let mut result = Vec::with_capacity(4 * out_size * out_size);

    for band_idx in 1..=4 {
        let band = ds.rasterband(band_idx)?;
        let buf = band.read_as::<u8>(
            (x_off as isize, y_off as isize),
            (x_size, y_size),
            (out_size, out_size),
            Some(ResampleAlg::Bilinear),
        )?;
        // Normalize u8 [0, 255] to f32 [0, 1]
        for &v in buf.data() {
            result.push(v as f32 / 255.0);
        }
    }

    Ok(result)
}
