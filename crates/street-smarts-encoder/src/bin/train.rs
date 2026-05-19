//! DINO pretraining on real Norfolk semantic raster tiles.
//!
//! Usage (from crates/street-smarts-encoder/):
//!   cargo run --bin train --features cuda,gdal --release

use ferrotorch_core::{Device, Tensor, TensorStorage, randn};
use ferrotorch_gpu::init_cuda_backend;
use street_smarts_encoder::dino::{DinoConfig, DinoTrainer};
use street_smarts_encoder::encoder::{IMAGE_SIZE, IN_CHANNELS};
use street_smarts_encoder::data::tiles::{self, DataSources, SemanticTile, NORFOLK_BBOX};

fn tile_to_tensor(tile: &SemanticTile, device: Device) -> Result<Tensor<f32>, Box<dyn std::error::Error>> {
    let storage = TensorStorage::cpu(tile.data.clone());
    let shape = vec![1, SemanticTile::CHANNELS, SemanticTile::SIZE, SemanticTile::SIZE];
    Ok(Tensor::from_storage(storage, shape, false)?.to(device)?)
}

/// Simple augmentation: add gaussian noise as a second "view".
fn augment_view(tile: &SemanticTile) -> Vec<f32> {
    let mut data = tile.data.clone();
    // Flip horizontally
    for c in 0..SemanticTile::CHANNELS {
        for y in 0..SemanticTile::SIZE {
            let row_start = c * SemanticTile::PIXELS + y * SemanticTile::SIZE;
            let row = &mut data[row_start..row_start + SemanticTile::SIZE];
            row.reverse();
        }
    }
    data
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_cuda_backend()?;
    let device = Device::Cuda(0);
    eprintln!("CUDA backend initialized");

    // Fetch data sources
    eprintln!("Setting up data sources...");
    let sources = DataSources::norfolk()?;

    // Generate tile grid
    let specs = tiles::tile_grid(NORFOLK_BBOX, 128.0, 64.0); // 64m stride for overlap
    eprintln!("{} tile specs generated", specs.len());

    // Pre-fetch a batch of tiles
    eprintln!("Fetching tiles (this may take a moment)...");
    let mut real_tiles: Vec<SemanticTile> = Vec::new();
    let max_tiles = 20; // Start small
    for (i, spec) in specs.iter().take(max_tiles * 2).enumerate() {
        match tiles::fetch_tile(&sources, spec) {
            Ok(tile) => {
                let n_present: usize = tile.channels_present.iter().filter(|&&v| v).count();
                if n_present >= 2 { // At least 2 channels populated
                    real_tiles.push(tile);
                    if real_tiles.len() >= max_tiles { break; }
                }
            }
            Err(e) => {
                if i < 3 { eprintln!("  tile {}: {}", i, e); }
            }
        }
    }
    eprintln!("Fetched {} usable tiles\n", real_tiles.len());

    if real_tiles.is_empty() {
        eprintln!("No tiles fetched — falling back to random data");
        // Fall back to random data
        let num_steps = 20;
        let config = DinoConfig {
            device,
            total_steps: num_steps,
            lr: 1e-4,
            ..Default::default()
        };
        let mut trainer = DinoTrainer::new(config)?;
        for step in 0..num_steps {
            let v1 = randn::<f32>(&[1, IN_CHANNELS, IMAGE_SIZE, IMAGE_SIZE])?.to(device)?;
            let v2 = randn::<f32>(&[1, IN_CHANNELS, IMAGE_SIZE, IMAGE_SIZE])?.to(device)?;
            let loss = trainer.train_step(&v1, &v2)?;
            if (step + 1) % 5 == 0 { eprintln!("  step {}: loss = {:.4}", step + 1, loss); }
        }
        return Ok(());
    }

    // Train on real tiles
    let num_steps = 50;
    let config = DinoConfig {
        device,
        total_steps: num_steps,
        lr: 1e-4,
        ..Default::default()
    };

    eprintln!("Building DINO trainer...");
    let mut trainer = DinoTrainer::new(config)?;
    eprintln!("Training on {} real Norfolk tiles for {} steps...\n", real_tiles.len(), num_steps);

    for step in 0..num_steps {
        let idx = step % real_tiles.len();
        let tile = &real_tiles[idx];

        // View 1: original tile
        let view1 = tile_to_tensor(tile, device)?;

        // View 2: horizontally flipped (simple augmentation)
        let aug_data = augment_view(tile);
        let aug_storage = TensorStorage::cpu(aug_data);
        let view2 = Tensor::from_storage(
            aug_storage,
            vec![1, SemanticTile::CHANNELS, SemanticTile::SIZE, SemanticTile::SIZE],
            false,
        )?.to(device)?;

        let loss = trainer.train_step(&view1, &view2)?;

        if (step + 1) % 10 == 0 || step == 0 {
            eprintln!("  step {}/{}: loss = {:.4} (tile {})", step + 1, num_steps, loss, idx);
        }
    }

    eprintln!("\nDINO pretraining on real data complete!");
    Ok(())
}
