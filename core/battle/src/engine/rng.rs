//! 固定派生域与随机流，保证历史种子可重放。
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};

/// 用种子和用途域派生一条独立 ChaCha8 随机流。
pub(super) fn stream(seed: u64, domain: &[u8]) -> ChaCha8Rng {
    let mut hash = Sha256::new();
    // 保留随机流派生域，保证既有种子的回放结果一致。
    hash.update(b"hcwa-battle-core/rng/v1\0");
    hash.update(domain);
    hash.update(seed.to_le_bytes());
    ChaCha8Rng::from_seed(hash.finalize().into())
}
/// 消耗随机流生成零到一之间、包含零的均匀值。
pub(super) fn uniform(rng: &mut ChaCha8Rng) -> f64 {
    ((rng.next_u64() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}
