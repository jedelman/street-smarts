//! Shared ViT-Tiny encoder for street-smarts.
//!
//! Wraps ferrotorch's VisionTransformer with our fixed architecture:
//! 9 channels, 128×128, patch=16, 192-dim, 12 layers, 3 heads.

use ferrotorch_core::{Device, FerrotorchResult};
use ferrotorch_nn::Module;
use ferrotorch_vision::models::vit::VisionTransformer;

/// Fixed encoder architecture matching the street-smarts design doc.
pub const IMAGE_SIZE: usize = 128;
pub const PATCH_SIZE: usize = 16;
pub const IN_CHANNELS: usize = 9;
pub const EMBED_DIM: usize = 192;
pub const DEPTH: usize = 12;
pub const NUM_HEADS: usize = 3;
pub const MLP_RATIO: usize = 4;

/// Number of patches: (128/16)^2 = 64
pub const NUM_PATCHES: usize = (IMAGE_SIZE / PATCH_SIZE) * (IMAGE_SIZE / PATCH_SIZE);

/// DINO projection head output dimension.
/// This is the dimension of the space where student/teacher outputs are compared.
pub const PROJ_DIM: usize = 256;

/// Create a new ViT-Tiny encoder with DINO projection head.
///
/// The `num_classes` parameter becomes `PROJ_DIM` for DINO — the head
/// outputs a vector in the projection space, not class logits.
pub fn build_encoder() -> FerrotorchResult<VisionTransformer<f32>> {
    VisionTransformer::new(
        IMAGE_SIZE,
        PATCH_SIZE,
        IN_CHANNELS,
        PROJ_DIM,
        EMBED_DIM,
        DEPTH,
        NUM_HEADS,
        MLP_RATIO,
    )
}

/// Move an encoder to the specified device.
pub fn to_device(model: &mut VisionTransformer<f32>, device: Device) -> FerrotorchResult<()> {
    model.to_device(device)
}
