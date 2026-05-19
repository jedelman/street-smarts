//! DINO pretraining on cached Norfolk tiles.
//!
//! Run `cargo run --bin prefetch --features gdal --release` first to populate the cache.
//! Then: `cargo run --bin train --features cuda --release`

use ferrotorch_core::{Device, Tensor, TensorStorage, randn};
use ferrotorch_gpu::init_cuda_backend;
use street_smarts_encoder::data::cache::TileCache;
use street_smarts_encoder::data::tiles::SemanticTile;
use street_smarts_encoder::dino::{DinoConfig, DinoTrainer};
use street_smarts_encoder::encoder::{IMAGE_SIZE, IN_CHANNELS};

fn tile_to_tensor(tile: &SemanticTile, device: Device) -> Result<Tensor<f32>, Box<dyn std::error::Error>> {
    let storage = TensorStorage::cpu(tile.data.clone());
    let shape = vec![1, SemanticTile::CHANNELS, SemanticTile::SIZE, SemanticTile::SIZE];
    Ok(Tensor::from_storage(storage, shape, false)?.to(device)?)
}

fn augment_flip_h(tile: &SemanticTile) -> SemanticTile {
    let mut data = tile.data.clone();
    for c in 0..SemanticTile::CHANNELS {
        for y in 0..SemanticTile::SIZE {
            let start = c * SemanticTile::PIXELS + y * SemanticTile::SIZE;
            data[start..start + SemanticTile::SIZE].reverse();
        }
    }
    SemanticTile {
        data,
        bounds: tile.bounds,
        channels_present: tile.channels_present,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_cuda_backend()?;
    let device = Device::Cuda(0);
    eprintln!("CUDA backend initialized");

    let cache = TileCache::new("../../data/tiles/norfolk");
    let tiles = cache.load_all();
    eprintln!("Loaded {} tiles from cache", tiles.len());

    if tiles.is_empty() {
        eprintln!("No cached tiles. Run `cargo run --bin prefetch --features gdal --release` first.");
        eprintln!("Falling back to random data...\n");

        let num_steps = 50;
        let config = DinoConfig { device, total_steps: num_steps, lr: 1e-4, ..Default::default() };
        let mut trainer = DinoTrainer::new(config)?;
        for step in 0..num_steps {
            let v1 = randn::<f32>(&[1, IN_CHANNELS, IMAGE_SIZE, IMAGE_SIZE])?.to(device)?;
            let v2 = randn::<f32>(&[1, IN_CHANNELS, IMAGE_SIZE, IMAGE_SIZE])?.to(device)?;
            let loss = trainer.train_step(&v1, &v2)?;
            if (step + 1) % 10 == 0 { eprintln!("  step {}: loss = {:.4}", step + 1, loss); }
        }
        return Ok(());
    }

    let num_steps = tiles.len() * 5; // 5 epochs over the dataset
    let config = DinoConfig {
        device,
        total_steps: num_steps,
        lr: 1e-4,
        ..Default::default()
    };

    eprintln!("Building DINO trainer ({} steps, {} tiles, 5 epochs)...", num_steps, tiles.len());
    let mut trainer = DinoTrainer::new(config)?;
    eprintln!("Training...\n");

    let report_every = (num_steps / 10).max(1);

    for step in 0..num_steps {
        let idx = step % tiles.len();
        let tile = &tiles[idx];
        let flipped = augment_flip_h(tile);

        let view1 = tile_to_tensor(tile, device)?;
        let view2 = tile_to_tensor(&flipped, device)?;

        let loss = trainer.train_step(&view1, &view2)?;

        if step == 0 || (step + 1) % report_every == 0 || step + 1 == num_steps {
            let epoch = step / tiles.len();
            eprintln!("  step {}/{} (epoch {}): loss = {:.4}", step + 1, num_steps, epoch, loss);
        }
    }

    eprintln!("\nDINO pretraining complete!");
    Ok(())
}
