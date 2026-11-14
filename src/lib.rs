//! Engine core + mind layers + UCI, and a tiny wasm api for the browser build.

pub mod board;
pub mod clock;
pub mod eval;
pub mod eval_params;
pub mod mind;
pub mod search;
pub mod see;
pub mod uci;

#[cfg(target_arch = "wasm32")]
mod wasm_api;
