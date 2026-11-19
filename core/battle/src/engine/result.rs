//! 导出单场执行结果，整局入账由局面运营层处理。
use super::*;

impl State<'_> {
    /// 取出本场单位和资源结果，交由局面运营层处理整局入账。
    pub(super) fn finish(
        &mut self,
        outcome: Outcome,
        termination: Termination,
        waves_cleared: usize,
    ) -> BattleResult {
        let weaknesses: Vec<_> = (0..self.units.len())
            .map(|i| self.weakness_state(i))
            .collect();
        let queue = self
            .input
            .mechanics
            .queue_capacity
            .map(|capacity| QueueResult {
                capacity,
                active_slots: self
                    .queue_slots
                    .iter()
                    .map(|i| {
                        i.filter(|&i| self.units[i].alive())
                            .map(|i| self.units[i].spec.id.clone())
                    })
                    .collect(),
                remaining_ids: self.input.waves[0][self.queue_next..]
                    .iter()
                    .map(|u| u.id.clone())
                    .collect(),
            });
        BattleResult {
            queue,
            cycle: self.timeline.cycle + 1,
            cycle_remaining_av: self.timeline.remaining,
            remaining_av: self.timeline.remaining_total(),
            outcome,
            termination,
            elapsed_av: self.timeline.now(),
            penalty_av: self.timeline.penalty(),
            actions: self.actions,
            waves_cleared,
            ally_skill_points: self.sp[0],
            enemy_skill_points: self.sp[1],
            units: std::mem::take(&mut self.units)
                .into_iter()
                .zip(weaknesses)
                .map(|(u, weaknesses)| u.snapshot(weaknesses))
                .collect(),
            #[cfg(feature = "diagnostics")]
            events: std::mem::take(&mut self.events),
            #[cfg(feature = "diagnostics")]
            trace_truncated: self.trace_truncated,
        }
    }
}
