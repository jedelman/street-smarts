//! Prefetch Norfolk tiles into the disk cache.
//!
//! Usage: cargo run --bin prefetch --features gdal --release

use street_smarts_encoder::data::cache::TileCache;
use street_smarts_encoder::data::tiles::{self, DataSources, NORFOLK_BBOX, CHANNEL_NAMES};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = TileCache::new("../../data/tiles/norfolk");

    eprintln!("Existing cached tiles: {}", cache.len());
    if cache.len() > 0 {
        eprintln!("  (delete data/tiles/norfolk/ to re-fetch)\n");
    }

    eprintln!("Setting up data sources...");
    let sources = DataSources::norfolk()?;

    // 64m stride gives good overlap for augmentation
    let specs = tiles::tile_grid(NORFOLK_BBOX, 128.0, 64.0);
    eprintln!("{} tile specs for Norfolk bbox", specs.len());

    let mut fetched = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for (i, spec) in specs.iter().enumerate() {
        match cache.get_or_fetch(&sources, spec) {
            Ok(tile) => {
                let n = tile.channels_present.iter().filter(|&&v| v).count();
                if n >= 2 {
                    fetched += 1;
                } else {
                    skipped += 1;
                }
            }
            Err(e) => {
                failed += 1;
                if failed <= 3 {
                    eprintln!("  tile {}: {}", i, e);
                }
            }
        }

        if (i + 1) % 50 == 0 || i + 1 == specs.len() {
            eprintln!(
                "  [{}/{}] fetched={} skipped={} failed={}",
                i + 1, specs.len(), fetched, skipped, failed
            );
        }
    }

    eprintln!("\nDone. {} tiles cached in data/tiles/norfolk/", cache.len());

    // Print channel coverage stats
    let tiles = cache.load_all();
    if !tiles.is_empty() {
        eprintln!("\nChannel coverage across {} tiles:", tiles.len());
        for (i, name) in CHANNEL_NAMES.iter().enumerate() {
            let count = tiles.iter().filter(|t| t.channels_present[i]).count();
            eprintln!("  [{}] {:12}: {}/{} ({:.0}%)",
                i, name, count, tiles.len(), count as f64 / tiles.len() as f64 * 100.0);
        }
    }

    Ok(())
}
