//! Disk cache for semantic raster tiles.
//!
//! Tiles are stored as raw f32 binary files (9 × 128 × 128 × 4 bytes = 576KB each)
//! with a sidecar `.meta` file for bounds and channel presence.
//! This avoids re-fetching from COGs and Overpass on every training run.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::tiles::{SemanticTile, TileSpec, DataSources, CHANNEL_NAMES};

/// Tile cache backed by a directory of binary files.
pub struct TileCache {
    dir: PathBuf,
}

impl TileCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        fs::create_dir_all(&dir).ok();
        Self { dir }
    }

    /// Key for a tile spec (based on center coordinates).
    fn key(spec: &TileSpec) -> String {
        format!("{:.6}_{:.6}_{:.0}", spec.lon, spec.lat, spec.size_m)
    }

    fn data_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.bin", key))
    }

    fn meta_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.meta", key))
    }

    /// Load a tile from cache, or fetch and cache it.
    #[cfg(feature = "gdal")]
    pub fn get_or_fetch(
        &self,
        sources: &DataSources,
        spec: &TileSpec,
    ) -> Result<SemanticTile, Box<dyn std::error::Error>> {
        let key = Self::key(spec);

        // Try loading from disk
        if let Ok(tile) = self.load(&key) {
            return Ok(tile);
        }

        // Fetch from remote sources
        let tile = super::tiles::fetch_tile(sources, spec)?;

        // Cache to disk
        self.store(&key, &tile)?;

        Ok(tile)
    }

    /// Load a cached tile from disk.
    pub fn load(&self, key: &str) -> Result<SemanticTile, Box<dyn std::error::Error>> {
        let data_path = self.data_path(key);
        let meta_path = self.meta_path(key);

        let mut f = fs::File::open(&data_path)?;
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;

        if bytes.len() != SemanticTile::NUMEL * 4 {
            return Err(format!(
                "cached tile has wrong size: {} bytes, expected {}",
                bytes.len(),
                SemanticTile::NUMEL * 4
            ).into());
        }

        // Reinterpret as f32
        let data: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        // Read metadata
        let meta_str = fs::read_to_string(&meta_path)?;
        let mut bounds = [0.0f64; 4];
        let mut channels_present = [false; 9];

        for line in meta_str.lines() {
            if let Some(rest) = line.strip_prefix("bounds=") {
                let parts: Vec<f64> = rest.split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                if parts.len() == 4 {
                    bounds.copy_from_slice(&parts);
                }
            } else if let Some(rest) = line.strip_prefix("present=") {
                for (i, ch) in rest.split(',').enumerate() {
                    if i < 9 {
                        channels_present[i] = ch.trim() == "1";
                    }
                }
            }
        }

        Ok(SemanticTile {
            data,
            bounds,
            channels_present,
        })
    }

    /// Store a tile to disk.
    pub fn store(&self, key: &str, tile: &SemanticTile) -> Result<(), Box<dyn std::error::Error>> {
        // Write raw f32 data
        let data_path = self.data_path(key);
        let bytes: Vec<u8> = tile.data.iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let mut f = fs::File::create(&data_path)?;
        f.write_all(&bytes)?;

        // Write metadata
        let meta_path = self.meta_path(key);
        let present: Vec<&str> = tile.channels_present.iter()
            .map(|&v| if v { "1" } else { "0" })
            .collect();
        let meta = format!(
            "bounds={},{},{},{}\npresent={}\n",
            tile.bounds[0], tile.bounds[1], tile.bounds[2], tile.bounds[3],
            present.join(","),
        );
        fs::write(&meta_path, meta)?;

        Ok(())
    }

    /// List all cached tile keys.
    pub fn keys(&self) -> Vec<String> {
        fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".bin") {
                    Some(name.trim_end_matches(".bin").to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Number of cached tiles.
    pub fn len(&self) -> usize {
        self.keys().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Load all cached tiles.
    pub fn load_all(&self) -> Vec<SemanticTile> {
        self.keys()
            .iter()
            .filter_map(|k| self.load(k).ok())
            .collect()
    }
}
