use ferrotorch_core::{zeros, ones, Device};
use ferrotorch_gpu::init_cuda_backend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_cuda_backend()?;
    println!("CUDA backend initialized");

    println!("\n--- Test 1: basic GPU ops ---");
    let a = ones::<f32>(&[64, 192])?.cuda()?;
    let b = ones::<f32>(&[64, 192])?.cuda()?;
    let c = (&a + &b)?;
    println!("  add: OK, shape={:?}", c.shape());

    println!("\n--- Test 2: matmul ---");
    let x = ones::<f32>(&[1, 64, 192])?.cuda()?;
    let w = ones::<f32>(&[192, 192])?.cuda()?;
    let y = x.matmul(&w)?;
    println!("  matmul: OK, shape={:?}", y.shape());

    println!("\n--- Test 3: LayerNorm ---");
    use ferrotorch_nn::{LayerNorm, Module};
    let mut ln = LayerNorm::new(vec![192], 1e-6, true)?;
    ln.to_device(Device::Cuda(0))?;
    let inp = ones::<f32>(&[1, 64, 192])?.cuda()?;
    match ln.forward(&inp) {
        Ok(out) => println!("  LayerNorm: OK, shape={:?}", out.shape()),
        Err(e) => println!("  LayerNorm: FAILED — {}", e),
    }

    println!("\n--- Test 4: Linear ---");
    use ferrotorch_nn::Linear;
    let mut lin = Linear::new(192, 192, true)?;
    lin.to_device(Device::Cuda(0))?;
    match lin.forward(&inp) {
        Ok(out) => println!("  Linear: OK, shape={:?}", out.shape()),
        Err(e) => println!("  Linear: FAILED — {}", e),
    }

    println!("\n--- Test 5: Softmax ---");
    use ferrotorch_nn::Softmax;
    let sm = Softmax::new(-1);
    let attn = ones::<f32>(&[1, 3, 64, 64])?.cuda()?;
    match sm.forward(&attn) {
        Ok(out) => println!("  Softmax: OK, shape={:?}", out.shape()),
        Err(e) => println!("  Softmax: FAILED — {}", e),
    }

    println!("\n--- Test 6: ViT-Tiny forward ---");
    use ferrotorch_vision::models::vit::VisionTransformer;
    let mut model: VisionTransformer<f32> = VisionTransformer::new(
        128, 16, 9, 12, 192, 12, 3, 4,
    )?;
    let num_params = model.num_parameters();
    println!("  params: {} ({:.1}M)", num_params, num_params as f64 / 1e6);
    model.to_device(Device::Cuda(0))?;
    let input = zeros::<f32>(&[1, 9, 128, 128])?.cuda()?;
    match model.forward(&input) {
        Ok(out) => println!("  ViT-Tiny: OK, shape={:?}", out.shape()),
        Err(e) => println!("  ViT-Tiny: FAILED — {}", e),
    }

    Ok(())
}
