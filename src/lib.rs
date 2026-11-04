//! Homo Ludens as a library: the engine core, the human mind layers, UCI,
//! and (in the browser build) a small WebAssembly API.

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
