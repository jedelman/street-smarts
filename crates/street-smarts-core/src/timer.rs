//! WASM-compatible elapsed timer. Uses `performance.now()` in WASM,
//! `std::time::Instant` natively.

pub struct Timer(instant::Instant);

impl Timer {
    pub fn start() -> Self {
        Self(instant::Instant::now())
    }
    /// Elapsed milliseconds, rounded up to at least 1 (so reports never lie
    /// about zero-cost evaluations on machines where the clock resolution is
    /// coarse). Caller can still distinguish reported 1 ms from 100 ms.
    pub fn elapsed_ms(&self) -> u32 {
        let ms = self.0.elapsed().as_micros() as f64 / 1000.0;
        ms.max(1.0).round() as u32
    }
}
