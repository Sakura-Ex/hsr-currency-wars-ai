//! 接收局面运营层组装的配置，以显式随机源执行战斗。
//!
//! 账号静态配置先经过局面运营与战斗组装，再进入本库；额外局内乘区待中间层接入。
pub mod batch;
pub mod decision;
#[cfg(feature = "diagnostics")]
pub mod diagnostics;
pub mod engine;
pub mod mechanics;
pub mod model;

// 保留现有调用方使用的公开路径；实现统一放在 mechanics。
pub use mechanics::{dynamic_scope, elements, expression, math, modifiers, weakness};
