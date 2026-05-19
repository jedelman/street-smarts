//! DINO pretraining binary.
//!
//! Usage (from crates/street-smarts-encoder/):
//!   cargo run --bin train --features cuda --release

use ferrotorch_core::{Device, Tensor, TensorStorage, randn};
use ferrotorch_gpu::init_cuda_backend;
use street_smarts_encoder::dino::{DinoConfig, DinoTrainer};
use street_smarts_encoder::encoder::{IMAGE_SIZE, IN_CHANNELS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_cuda_backend()?;
    let device = Device::Cuda(0);
    eprintln!("CUDA backend initialized");

    let batch_size = 1; // 2GB VRAM constraint
    let num_steps = 50;

    let config = DinoConfig {
        device,
        total_steps: num_steps,
        lr: 1e-4,
        ..Default::default()
    };

    eprintln!("Building DINO trainer (batch={}, steps={})...", batch_size, num_steps);
    let mut trainer = DinoTrainer::new(config)?;
    eprintln!("Training...\n");

    for step in 0..num_steps {
        // Two different random views (simulates augmentation)
        let view1 = randn::<f32>(&[batch_size, IN_CHANNELS, IMAGE_SIZE, IMAGE_SIZE])?
            .to(device)?;
        let view2 = randn::<f32>(&[batch_size, IN_CHANNELS, IMAGE_SIZE, IMAGE_SIZE])?
            .to(device)?;

        let loss = trainer.train_step(&view1, &view2)?;

        if (step + 1) % 10 == 0 || step == 0 {
            eprintln!("  step {}/{}: loss = {:.6}", step + 1, num_steps, loss);
        }
    }

    eprintln!("\nDINO pretraining complete ({} steps).", num_steps);
    Ok(())
}
