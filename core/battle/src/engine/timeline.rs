//! 按剩余行动距离与速度选择下一行动者，维护轮次 AV 预算。
//! 单位在 TurnEnd 时重排行动距离。
/// 普通行动的标准距离，等待 AV 由剩余距离除以速度计算。
pub const DISTANCE: f64 = 10000.0;
/// 实际 AV 时钟及各倒计时轮次的剩余额度。
pub struct Timeline {
    /// 实际推进的 AV 时钟。
    pub now: f64,
    /// 当前倒计时轮次的内部索引，从零开始。
    pub cycle: usize,
    /// 当前 Cycle 剩余 AV 额度。
    pub remaining: f64,
    /// 累计实际扣除的当前轮罚时。
    pub penalty: f64,
    /// 各倒计时轮次的初始额度。
    pub budgets: Vec<f64>,
}
impl Timeline {
    /// 按所属类型的初始配置建立独立运行状态。
    pub fn new(budgets: Vec<f64>) -> Self {
        Self {
            now: 0.0,
            cycle: 0,
            remaining: budgets[0],
            penalty: 0.0,
            budgets,
        }
    }
    /// 返回已实际推进的 AV 时钟。
    pub fn now(&self) -> f64 {
        self.now
    }
    /// 返回累计实际扣除的复活罚时。
    pub fn penalty(&self) -> f64 {
        self.penalty
    }
    /// 返回当前轮余额与后续轮初始额度之和。
    pub fn remaining_total(&self) -> f64 {
        self.remaining + self.budgets.iter().skip(self.cycle + 1).sum::<f64>()
    }
    /// 最多扣光当前轮额度，累计并返回实际扣除量。
    pub fn penalize(&mut self, amount: f64) -> f64 {
        let applied = amount.min(self.remaining);
        self.remaining -= applied;
        self.penalty += applied;
        applied
    }
    /// 推进实际 AV 并扣减当前轮额度。
    pub fn advance(&mut self, delta: f64) {
        self.now += delta;
        self.remaining = (self.remaining - delta).max(0.0);
    }
    /// 存在下一轮时装载其额度并返回 true。
    pub fn next_cycle(&mut self) -> bool {
        if self.cycle + 1 >= self.budgets.len() {
            false
        } else {
            self.cycle += 1;
            self.remaining = self.budgets[self.cycle];
            true
        }
    }
}
/// 按距离除以速度选择最近的单位，同刻按索引稳定排序。
pub fn next_normal(items: impl Iterator<Item = (usize, f64, f64)>) -> Option<(usize, f64)> {
    items
        .map(|(i, d, s)| (i, d / s))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}
