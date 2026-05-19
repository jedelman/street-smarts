//! Fetch real semantic tiles from Norfolk to verify the data pipeline.

use street_smarts_encoder::data::tiles::{
    self, DataSources, SemanticTile, TileSpec, CHANNEL_NAMES, NORFOLK_BBOX,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Setting up data sources (fetching SAS tokens)...");
    let sources = DataSources::norfolk()?;

    let specs = tiles::tile_grid(NORFOLK_BBOX, 128.0, 128.0);
    eprintln!("{} tiles cover Norfolk bbox\n", specs.len());

    // Fetch a center tile
    let spec = &specs[specs.len() / 2];
    eprintln!("Fetching tile at ({:.4}, {:.4})...", spec.lon, spec.lat);

    let tile = tiles::fetch_tile(&sources, spec)?;

    eprintln!("\nChannel statistics:");
    let px = SemanticTile::PIXELS;
    for (i, name) in CHANNEL_NAMES.iter().enumerate() {
        let ch = tile.channel(i);
        let present = tile.channels_present[i];
        if present {
            let min = ch.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = ch.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let mean: f32 = ch.iter().sum::<f32>() / px as f32;
            let nonzero = ch.iter().filter(|&&v| v > 0.001).count();
            eprintln!("  [{}] {:12}: min={:.3} max={:.3} mean={:.3} nonzero={}/{}",
                i, name, min, max, mean, nonzero, px);
        } else {
            eprintln!("  [{}] {:12}: (not available)", i, name);
        }
    }

    eprintln!("\nPresent: {}/9 channels", tile.channels_present.iter().filter(|&&v| v).count());
    Ok(())
}
