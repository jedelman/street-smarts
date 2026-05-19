//! DINO self-supervised training loop.
//!
//! Implements "Emerging Properties in Self-Supervised Vision Transformers"
//! (Caron et al., 2021) adapted for multi-channel semantic rasters.
//!
//! Student and teacher share the same ViT architecture. Teacher is an
//! exponential moving average of the student (no gradient). Both see
//! different augmented views of the same tile. Loss is cross-entropy
//! between teacher's centered/sharpened softmax and student's sharpened
//! softmax.

use ferrotorch_core::autograd::no_grad::no_grad;
use ferrotorch_core::autograd::graph::backward;
use ferrotorch_core::{Device, FerrotorchResult, Tensor, TensorStorage, zeros};
use ferrotorch_nn::Module;
use ferrotorch_nn::Parameter;
use ferrotorch_optim::{AdamW, AdamWConfig, Optimizer};
use ferrotorch_vision::models::vit::VisionTransformer;

use crate::encoder::{self, PROJ_DIM};

/// DINO training configuration.
pub struct DinoConfig {
    pub ema_start: f64,
    pub ema_end: f64,
    pub student_temp: f32,
    pub teacher_temp: f32,
    pub lr: f64,
    pub weight_decay: f64,
    pub total_steps: usize,
    pub device: Device,
}

impl Default for DinoConfig {
    fn default() -> Self {
        Self {
            ema_start: 0.996,
            ema_end: 1.0,
            student_temp: 0.1,
            teacher_temp: 0.04,
            lr: 5e-4,
            weight_decay: 0.04,
            total_steps: 10_000,
            device: Device::Cpu,
        }
    }
}

/// Create a scalar tensor on a given device, without grad tracking.
fn scalar(val: f32, device: Device) -> FerrotorchResult<Tensor<f32>> {
    no_grad(|| {
        Tensor::from_storage(TensorStorage::cpu(vec![val]), vec![1], false)?
            .to(device)
    })
}

/// DINO trainer state.
pub struct DinoTrainer {
    pub student: VisionTransformer<f32>,
    pub teacher: VisionTransformer<f32>,
    optimizer: AdamW<f32>,
    center: Tensor<f32>,
    center_momentum: f32,
    config: DinoConfig,
    step: usize,
}

impl DinoTrainer {
    pub fn new(config: DinoConfig) -> FerrotorchResult<Self> {
        let mut student = encoder::build_encoder()?;
        let mut teacher = encoder::build_encoder()?;

        student.to_device(config.device)?;
        teacher.to_device(config.device)?;

        // Sync teacher to student
        copy_params(&student, &mut teacher)?;

        for p in teacher.parameters_mut() {
            p.set_requires_grad(false);
        }

        let mut adamw_config = AdamWConfig::default();
        adamw_config.lr = config.lr;
        adamw_config.weight_decay = config.weight_decay;
        let params: Vec<Parameter<f32>> = student.parameters()
            .into_iter()
            .cloned()
            .collect();
        let optimizer = AdamW::new(params, adamw_config);

        let center = zeros::<f32>(&[1, PROJ_DIM])?.to(config.device)?;

        Ok(Self {
            student,
            teacher,
            optimizer,
            center,
            center_momentum: 0.9,
            config,
            step: 0,
        })
    }

    fn ema_decay(&self) -> f64 {
        let progress = (self.step as f64 / self.config.total_steps as f64).min(1.0);
        self.config.ema_start + (self.config.ema_end - self.config.ema_start) * progress
    }

    /// Run one DINO training step. Returns loss for logging.
    pub fn train_step(
        &mut self,
        view1: &Tensor<f32>,
        view2: &Tensor<f32>,
    ) -> FerrotorchResult<f32> {
        let s1 = self.student.forward(view1)?;
        let s2 = self.student.forward(view2)?;

        let t1 = no_grad(|| self.teacher.forward(view1))?;
        let t2 = no_grad(|| self.teacher.forward(view2))?;

        let dev = self.config.device;
        let loss1 = dino_loss(&s1, &t2, &self.center, self.config.student_temp, self.config.teacher_temp, dev)?;
        let loss2 = dino_loss(&s2, &t1, &self.center, self.config.student_temp, self.config.teacher_temp, dev)?;
        let half = scalar(0.5, dev)?;
        let loss = (&(&loss1 + &loss2)? * &half)?;

        self.optimizer.zero_grad()?;
        backward(&loss)?;
        self.optimizer.step()?;

        let decay = self.ema_decay();
        ema_update(&self.student, &mut self.teacher, decay)?;

        // Update center (running mean of teacher outputs)
        let batch_center = no_grad(|| -> FerrotorchResult<Tensor<f32>> {
            let mean_t1 = t1.mean_dim(0, true)?;
            let mean_t2 = t2.mean_dim(0, true)?;
            &(&mean_t1 + &mean_t2)? * &half
        })?;
        let m = scalar(self.center_momentum, dev)?;
        let one_m_m = scalar(1.0 - self.center_momentum, dev)?;
        self.center = (&(&self.center * &m)? + &(&batch_center * &one_m_m)?)?;

        self.step += 1;
        let loss_val = loss.cpu()?.data_vec()?;
        Ok(loss_val[0])
    }

    pub fn step(&self) -> usize {
        self.step
    }
}

fn dino_loss(
    student_out: &Tensor<f32>,
    teacher_out: &Tensor<f32>,
    center: &Tensor<f32>,
    student_temp: f32,
    teacher_temp: f32,
    device: Device,
) -> FerrotorchResult<Tensor<f32>> {
    let s_temp = scalar(student_temp, device)?;
    let t_temp = scalar(teacher_temp, device)?;

    // Teacher: center, sharpen, softmax
    let t_soft = {
        use ferrotorch_nn::Softmax;
        let t_sharp = (&(teacher_out - center)? / &t_temp)?;
        Softmax::new(-1).forward(&t_sharp)?
    };

    // Student: sharpen, log_softmax
    let s_log_soft = (student_out / &s_temp)?.log_softmax()?;

    // Cross-entropy: -sum(t * log(s)) / batch
    let batch_size = scalar(student_out.shape()[0] as f32, device)?;
    let neg_one = scalar(-1.0, device)?;
    let ce = (&t_soft * &s_log_soft)?.sum_all()?;
    let neg_ce = (&ce * &neg_one)?;
    &neg_ce / &batch_size
}

fn copy_params(
    src: &VisionTransformer<f32>,
    dst: &mut VisionTransformer<f32>,
) -> FerrotorchResult<()> {
    let src_params = src.parameters();
    let mut dst_params = dst.parameters_mut();
    for (s, d) in src_params.iter().zip(dst_params.iter_mut()) {
        d.set_data(s.tensor().clone());
    }
    Ok(())
}

fn ema_update(
    student: &VisionTransformer<f32>,
    teacher: &mut VisionTransformer<f32>,
    decay: f64,
) -> FerrotorchResult<()> {
    let decay_f = decay as f32;
    no_grad(|| -> FerrotorchResult<()> {
        let s_params = student.parameters();
        let mut t_params = teacher.parameters_mut();
        let device = s_params.first()
            .map(|p| p.tensor().device())
            .unwrap_or(Device::Cpu);
        let d = Tensor::from_storage(TensorStorage::cpu(vec![decay_f]), vec![1], false)?
            .to(device)?;
        let one_d = Tensor::from_storage(TensorStorage::cpu(vec![1.0 - decay_f]), vec![1], false)?
            .to(device)?;
        for (s, t) in s_params.iter().zip(t_params.iter_mut()) {
            let new_val = (&(t.tensor() * &d)? + &(s.tensor() * &one_d)?)?;
            t.set_data(new_val);
        }
        Ok(())
    })
}
