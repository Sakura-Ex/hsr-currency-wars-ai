//! 合法操作、策略选择与动作入口的资源结算。
use super::*;

impl State<'_> {
    /// 从当前资源中扣除该动作费用，调用前已确认可支付。
    pub(super) fn pay(&mut self, actor: usize, action: &Action) {
        let side = self.side(actor);
        self.sp[side] -= action.sp_cost;
        self.units[actor].energy -= action.energy_cost;
        trace!(
            self,
            "energy_cost",
            actor,
            Some(actor),
            Some(&action.id),
            Some(action.energy_cost),
            None,
        );
    }

    /// 登记后续动作，去重同单位终结技并限制排队工作量。
    pub(super) fn enqueue(&mut self, request: Pending) -> Result<(), String> {
        if request.kind == RequestKind::Ultimate
            && self
                .pending
                .iter()
                .any(|p| p.actor == request.actor && p.kind == RequestKind::Ultimate)
        {
            return Ok(());
        }
        if self.pending.len() >= 10000 {
            return Err("pending action limit exceeded".into());
        }
        trace!(
            self,
            "request_queued",
            request.actor,
            None,
            None,
            Some(self.units[request.actor].energy),
            None,
        );
        self.pending.push_back(request);
        Ok(())
    }

    /// 只生成当前合法操作；决策快照与倒计时收尾共用同一规则。
    pub(super) fn legal_operations(
        &self,
        point: DecisionPoint,
        actor: Option<usize>,
    ) -> Vec<Operation> {
        let mut legal = Vec::new();
        for (i, u) in self.units.iter().enumerate() {
            if !u.alive() || actor.is_some_and(|a| a != i) {
                continue;
            }
            if point == DecisionPoint::Insert
                && self
                    .pending
                    .iter()
                    .any(|p| p.actor == i && p.kind == RequestKind::Ultimate)
            {
                continue;
            }
            for (j, a) in u.spec.actions.iter().enumerate() {
                let correct = if point == DecisionPoint::Insert {
                    a.kind == ActionKind::Ultimate
                } else {
                    matches!(a.kind, ActionKind::Basic | ActionKind::Skill)
                };
                if correct && self.affordable(i, a) {
                    legal.push(Operation {
                        actor: i,
                        action: j,
                    });
                }
            }
        }
        if point == DecisionPoint::Insert {
            let rules = &self.input.mechanics;
            legal.sort_by(|a, b| {
                let priority = |op: &Operation| {
                    *rules
                        .ultimate_priority
                        .get(&self.units[op.actor].spec.id)
                        .unwrap_or(&0)
                };
                let order = priority(a).cmp(&priority(b));
                (if rules.larger_priority_first {
                    order.reverse()
                } else {
                    order
                })
                .then(a.actor.cmp(&b.actor))
                .then(a.action.cmp(&b.action))
            });
        }
        legal
    }

    /// 生成决策数据；该步骤只读当前状态，选招由调用方单独完成。
    pub(super) fn decision(&self, point: DecisionPoint, actor: Option<usize>, id: u64) -> Decision {
        let legal = self.legal_operations(point, actor);
        let mut axis: Vec<_> = self
            .units
            .iter()
            .filter(|u| u.alive())
            .map(|u| AxisPosition {
                id: u.spec.id.clone(),
                remaining_distance: u.remaining,
                speed: u.spec.stats.speed,
                waiting_av: u.remaining / u.spec.stats.speed,
            })
            .collect();
        axis.sort_by(|a, b| a.waiting_av.total_cmp(&b.waiting_av));
        let observation = Observation {
            now: self.timeline.now,
            cycle: self.timeline.cycle + 1,
            cycle_remaining_av: self.timeline.remaining,
            remaining_av: self.timeline.remaining_total(),
            ally_skill_points: self.sp[0],
            enemy_skill_points: self.sp[1],
            axis,
            units: self
                .units
                .iter()
                .enumerate()
                .map(|(owner, u)| u.snapshot(self.weakness_state(owner)))
                .collect(),
        };
        Decision {
            id,
            point,
            observation,
            legal,
        }
    }

    /// 执行一个完整动作，依次处理绑定、付费、效果、收益与结束事件。
    pub(super) fn act(
        &mut self,
        actor: usize,
        action_index: usize,
        prepaid: bool,
    ) -> Result<(), String> {
        let action = self.units[actor].spec.actions[action_index].clone();
        // 普通选招已通过当前合法列表；排队请求在实际入口重新检查资源。
        // Bind once even when a nested conditional/loop is the first target use.
        self.bound_target = self.targets(actor, Target::EnemyLowestHp)?.first().copied();
        self.current_action_kind = Some(action.kind);
        self.attack_owner = Some(actor);
        self.attack_targets.clear();
        let side = self.side(actor);
        if !prepaid {
            self.pay(actor, &action);
        }
        self.actions += 1;
        self.dispatch(BattleEvent::ActionStart, Some(actor), Some(&action.id))?;
        self.dispatch(BattleEvent::BeforeSkillUse, Some(actor), Some(&action.id))?;
        trace!(self, "action", actor, None, Some(&action.id), None, None);
        self.apply_effects(actor, &action.id, &action.effects)?;
        if self.units[actor].alive() {
            self.sp[side] = self.sp[side]
                .saturating_add(action.sp_gain)
                .min(self.sp_max(side));
            self.units[actor].energy = (self.units[actor].energy + action.energy_gain)
                .min(self.units[actor].spec.max_energy);
        }
        self.dispatch(BattleEvent::ActionEnd, Some(actor), Some(&action.id))?;
        trace!(
            self,
            "action_end",
            actor,
            None,
            Some(&action.id),
            None,
            None
        );
        self.bound_target = None;
        self.current_action_kind = None;
        self.attack_owner = None;
        self.attack_targets.clear();
        Ok(())
    }
}
