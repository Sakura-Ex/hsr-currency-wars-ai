//! 分步会话与同步运行入口，共用同一套战斗推进。
use crate::{
    decision::*,
    elements::{DamageElement, Element},
    engine::timeline::{self as scheduler, Timeline, DISTANCE},
    math,
    model::*,
    modifiers::{ModifierBook, Property},
};
use rand_chacha::ChaCha8Rng;

// 编译时排除回放及其参数构造，正式执行保留同一套规则分派。
macro_rules! trace {
    ($state:expr, $($argument:expr),* $(,)?) => {
        #[cfg(feature = "diagnostics")]
        $state.emit($($argument),*);
    };
}

mod damage;
mod effects;
mod events;
mod operations;
mod result;
mod rng;
mod session;
mod state;
mod targets;
mod timeline;
mod values;

use effects::finite;
use rng::{stream, uniform};
use state::{CombatUnit, Pending, State};

pub use session::{BattleSession, BattleStep};

/// 诊断报告记录的随机算法及分流版本。
#[cfg(feature = "diagnostics")]
pub const RNG_ALGORITHM: &str =
    "ChaCha8Rng/rand_chacha=0.9.0; SHA256-separated target/critical/element streams v1";
/// 单场诊断回放的条数上限。
#[cfg(feature = "diagnostics")]
const TRACE_LIMIT: usize = 100_000;

/// 使用默认策略完成一场战斗，输入配置保持只读。
///
/// seed 初始化本场目标、暴击和元素随机流；运行错误返回具体原因。
pub fn simulate(input: &Scenario, seed: u64) -> Result<BattleResult, String> {
    simulate_cancellable(input, seed, &mut || false)
}

/// 使用默认策略运行，并在完整动作边界检查取消。
///
/// 取消得到 Incomplete/Cancelled；清场及已到期的 Turn/Cycle 收尾按规则先结算。
pub fn simulate_cancellable(
    input: &Scenario,
    seed: u64,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BattleResult, String> {
    simulate_with_policy(input, seed, cancel, &mut Automatic)
}

/// 让外部策略从当前合法操作中选招，执行规则由同一会话维护。
///
/// policy 可以保存自身记忆；cancel 在调度边界轮询，执行错误保持为 Err。
pub fn simulate_with_policy(
    input: &Scenario,
    seed: u64,
    cancel: &mut impl FnMut() -> bool,
    policy: &mut impl Policy,
) -> Result<BattleResult, String> {
    drive(BattleSession::new(input, seed)?, cancel, policy)
}

/// 连续驱动一个已初始化会话，供正式入口与诊断入口共用。
pub(crate) fn drive(
    mut session: BattleSession<'_>,
    cancel: &mut impl FnMut() -> bool,
    policy: &mut impl Policy,
) -> Result<BattleResult, String> {
    loop {
        let (id, choice) = match session.advance_continuous(cancel)? {
            BattleStep::Decision(decision) => (
                decision.id,
                policy.decide(decision.point, &decision.observation, &decision.legal),
            ),
            BattleStep::Finished(_) => return session.into_result(),
        };
        session.submit(id, choice)?;
    }
}
