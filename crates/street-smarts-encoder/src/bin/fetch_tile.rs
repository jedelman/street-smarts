//! Fetch a real NAIP tile from Norfolk to verify the data pipeline.

use street_smarts_encoder::data::tiles::{self, TileSpec, NORFOLK_BBOX};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // NAIP URL for Norfolk area (2023 vintage, from Planetary Computer STAC)
    let naip_url = "https://naipeuwest.blob.core.windows.net/naip/v002/va/2023/va_060cm_2023/36076/m_3607631_sw_18_060_20231009_20240103.tif";

    // Generate tile grid over Eastside Commons area
    let specs = tiles::tile_grid(NORFOLK_BBOX, 128.0, 128.0);
    eprintln!("Generated {} tile specs over Norfolk bbox", specs.len());

    // Fetch first tile
    let spec = &specs[specs.len() / 2]; // take a center tile
    eprintln!("Fetching NAIP tile at ({:.4}, {:.4})...", spec.lon, spec.lat);

    let data = tiles::fetch_naip_tile(naip_url, spec)?;
    eprintln!("Got {} floats ({} channels × {}×{})",
        data.len(), 4, tiles::SemanticTile::SIZE, tiles::SemanticTile::SIZE);

    // Print channel statistics
    let size = tiles::SemanticTile::SIZE * tiles::SemanticTile::SIZE;
    for (i, name) in ["Red", "Green", "Blue", "NIR"].iter().enumerate() {
        let ch = &data[i * size..(i + 1) * size];
        let min = ch.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = ch.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mean: f32 = ch.iter().sum::<f32>() / size as f32;
        eprintln!("  {}: min={:.3} max={:.3} mean={:.3}", name, min, max, mean);
    }

    eprintln!("\nNAIP tile fetch OK!");
    Ok(())
}
