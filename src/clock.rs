//! Wall-clock time for search limits. Native builds use `Instant`; the
//! browser build asks the page for `performance.now()`.

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub struct Instant(std::time::Instant);

#[cfg(not(target_arch = "wasm32"))]
impl Instant {
    pub fn now() -> Instant {
        Instant(std::time::Instant::now())
    }
    pub fn elapsed_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn hl_now_ms() -> f64;
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
pub struct Instant(f64);

#[cfg(target_arch = "wasm32")]
impl Instant {
    pub fn now() -> Instant {
        Instant(unsafe { hl_now_ms() })
    }
    pub fn elapsed_ms(&self) -> u64 {
        (unsafe { hl_now_ms() } - self.0).max(0.0) as u64
    }
}
