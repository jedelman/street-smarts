//! Linear probes for Alexander pattern detection.
//!
//! Each pattern gets a tiny linear classifier (192 → 1) on top of the
//! frozen DINO encoder. Training a probe requires ~500 labeled tiles.
//!
//! The probe is a learned linear functional over the encoder's latent space.

use ferrotorch_core::autograd::graph::backward;
use ferrotorch_core::autograd::no_grad::no_grad;
use ferrotorch_core::{Device, FerrotorchResult, Tensor};
use ferrotorch_nn::{Linear, Module};
use ferrotorch_nn::functional::binary_cross_entropy_with_logits;
use ferrotorch_nn::Reduction;
use ferrotorch_optim::{AdamW, AdamWConfig, Optimizer};
use ferrotorch_vision::models::vit::VisionTransformer;

use crate::encoder::EMBED_DIM;

/// A linear probe for a single Alexander pattern.
pub struct PatternProbe {
    pub name: String,
    head: Linear<f32>,
}

impl PatternProbe {
    pub fn new(name: impl Into<String>) -> FerrotorchResult<Self> {
        Ok(Self {
            name: name.into(),
            head: Linear::new(EMBED_DIM, 1, true)?,
        })
    }

    pub fn to_device(&mut self, device: Device) -> FerrotorchResult<()> {
        self.head.to_device(device)
    }

    pub fn forward(&self, features: &Tensor<f32>) -> FerrotorchResult<Tensor<f32>> {
        self.head.forward(features)
    }

    pub fn num_parameters(&self) -> usize {
        self.head.parameters().iter()
            .map(|p| p.tensor().shape().iter().product::<usize>())
            .sum()
    }
}

/// Extract CLS features from images using a frozen encoder.
/// Input: [B, 9, 128, 128] → Output: [B, PROJ_DIM]
/// Note: returns projection head output, not raw EMBED_DIM features.
/// For probes we'd ideally want pre-head features, but this works for v0.
pub fn extract_features(
    encoder: &VisionTransformer<f32>,
    images: &Tensor<f32>,
) -> FerrotorchResult<Tensor<f32>> {
    no_grad(|| encoder.forward(images))
}

/// Train a probe on pre-extracted features.
/// `features`: [N, dim], `labels`: [N] with 0.0 or 1.0
pub fn train_probe(
    probe: &mut PatternProbe,
    features: &Tensor<f32>,
    labels: &Tensor<f32>,
    device: Device,
    num_epochs: usize,
    lr: f64,
) -> FerrotorchResult<Vec<f32>> {
    probe.to_device(device)?;

    let mut config = AdamWConfig::default();
    config.lr = lr;
    config.weight_decay = 0.01;
    let params = probe.head.parameters().into_iter().cloned().collect();
    let mut optimizer = AdamW::new(params, config);

    let n = features.shape()[0];
    let labels_col = labels.view(&[n as i64, 1])?;

    let mut losses = Vec::with_capacity(num_epochs);

    for _epoch in 0..num_epochs {
        let logits = probe.forward(features)?;
        let loss = binary_cross_entropy_with_logits(&logits, &labels_col, Reduction::Mean)?;

        optimizer.zero_grad()?;
        backward(&loss)?;
        optimizer.step()?;

        let loss_val = loss.cpu()?.data_vec()?;
        losses.push(loss_val[0]);
    }

    Ok(losses)
}

/// Evaluate probe accuracy.
pub fn eval_probe(
    probe: &PatternProbe,
    features: &Tensor<f32>,
    labels: &Tensor<f32>,
) -> FerrotorchResult<f64> {
    let logits = no_grad(|| probe.forward(features))?;
    let logits_cpu = logits.cpu()?.data_vec()?;
    let labels_cpu = labels.cpu()?.data_vec()?;

    let n = logits_cpu.len();
    let correct = logits_cpu.iter().zip(labels_cpu.iter())
        .filter(|(&logit, &label)| {
            let pred = if logit > 0.0 { 1.0 } else { 0.0 };
            (pred - label).abs() < 0.5
        })
        .count();

    Ok(correct as f64 / n as f64)
}
