//! 监听器到期、事件分派、复活与回放记录。
use super::*;

impl State<'_> {
    /// 从自然弱点与当前活动状态合成单位弱点。
    pub(super) fn weakness_state(&self, owner: usize) -> Option<crate::weakness::WeaknessState> {
        let mechanics = &self.input.mechanics;
        let unit = &self.units[owner];
        let natural = mechanics.natural_weaknesses.get(&unit.spec.id)?;
        let attached = unit.modifiers.instances.iter().flat_map(|instance| {
            mechanics
                .modifier_weaknesses
                .get(&instance.state.id)
                .into_iter()
                .flatten()
                .copied()
        });
        Some(crate::weakness::WeaknessState::compose(natural, attached))
    }

    #[cfg(feature = "diagnostics")]
    /// 合成弱点变化时输出诊断记录。
    pub(super) fn record_weakness_change(
        &mut self,
        owner: usize,
        before: Option<crate::weakness::WeaknessState>,
        action: Option<&str>,
    ) {
        let after = self.weakness_state(owner);
        if before != after {
            if let Some(after) = after {
                // Observation only; this is not a native OnBeingStackWeakness callback.
                trace!(
                    self,
                    "weakness_count",
                    owner,
                    Some(owner),
                    action,
                    Some(after.all.len() as f64),
                    None,
                );
            }
        }
    }

    /// 先处理适用状态到期，再按声明顺序运行事件监听器。
    pub(super) fn dispatch(
        &mut self,
        event: BattleEvent,
        subject: Option<usize>,
        action: Option<&str>,
    ) -> Result<(), String> {
        if self.event_depth >= 64 {
            return Err("runtime event recursion exceeds 64".into());
        }
        self.event_depth += 1;
        let subject_id = subject.map(|i| self.units[i].spec.id.clone());
        #[cfg(feature = "diagnostics")]
        let weakness_before: Vec<_> = if self.input.mechanics.modifier_weaknesses.is_empty() {
            Vec::new()
        } else {
            (0..self.units.len())
                .map(|i| self.weakness_state(i))
                .collect()
        };
        // Settle all expiry before callbacks. Future-wave units are absent here.
        #[cfg(feature = "diagnostics")]
        let mut expired = vec![];
        let units = self.units.iter_mut();
        #[cfg(feature = "diagnostics")]
        let units = units.enumerate();
        #[cfg(not(feature = "diagnostics"))]
        let units = units.map(|unit| ((), unit));
        for (_owner, unit) in units {
            let _expired = unit
                .modifiers
                .tick(event, subject_id.as_deref(), &unit.spec.id);
            #[cfg(feature = "diagnostics")]
            expired.extend(_expired.into_iter().map(|modifier| (_owner, modifier.id)));
        }
        #[cfg(feature = "diagnostics")]
        for (_owner, _id) in expired {
            trace!(
                self,
                "modifier_expired",
                _owner,
                Some(_owner),
                Some(&_id),
                None,
                None,
            );
        }
        #[cfg(feature = "diagnostics")]
        for (owner, before) in weakness_before.into_iter().enumerate() {
            self.record_weakness_change(owner, before, action);
        }
        // Expire every applicable status before any listener can dispatch nested
        // events. Global events have no unit subject; absent future-wave owners
        // have not entered the battle and their lifetimes have not started.
        for t in &mut self.hooks {
            if t.remaining == Some(0) {
                continue;
            }
            let Some(owner) = self.units.iter().position(|u| u.spec.id == t.spec.owner) else {
                continue;
            };
            if t.spec.expires_on == Some(event) && (event.is_global() || subject == Some(owner)) {
                t.duration -= 1;
                if t.duration == 0 {
                    t.remaining = Some(0);
                }
            }
        }
        for index in 0..self.hooks.len() {
            let t = &mut self.hooks[index];
            if t.remaining == Some(0)
                || t.spec.event != event
                || t.spec
                    .subject
                    .as_ref()
                    .is_some_and(|id| Some(id) != subject_id.as_ref())
                || t.spec.action.as_deref().is_some_and(|a| Some(a) != action)
            {
                continue;
            }
            let Some(owner) = self.units.iter().position(|u| u.spec.id == t.spec.owner) else {
                continue;
            };
            // The owner's own death is the one event it receives while dead.
            // Other dead-owner listeners remain inactive, including nested deaths.
            if !self.units[owner].alive()
                && !(event == BattleEvent::Death && subject == Some(owner))
            {
                continue;
            }
            if let Some(left) = &mut t.remaining {
                *left -= 1;
            }
            let effects = t.spec.effects.clone();
            trace!(self, "trigger", owner, subject, action, None, None);
            self.apply_effects(owner, action.unwrap_or("event"), &effects)?;
        }
        self.event_depth -= 1;
        Ok(())
    }

    #[cfg(feature = "diagnostics")]
    /// 按 trace 开关与条数上限保存诊断记录。
    pub(super) fn emit(
        &mut self,
        kind: &str,
        actor: usize,
        target: Option<usize>,
        action: Option<&str>,
        amount: Option<f64>,
        critical: Option<bool>,
    ) {
        if !self.trace {
            return;
        }
        if self.events.len() >= TRACE_LIMIT {
            self.trace_truncated = true;
            return;
        }
        self.events.push(crate::diagnostics::Event {
            index: self.events.len() as u32,
            av: self.timeline.now(),
            kind: kind.into(),
            actor: self.units[actor].spec.id.clone(),
            target: target.map(|i| self.units[i].spec.id.clone()),
            action: action.map(str::to_owned),
            amount,
            critical,
            attack_type: (kind == "damage")
                .then_some(self.damage_attack_type.unwrap_or(AttackType::Direct)),
        });
    }

    /// 分派死亡事件并处理护盾清除、即时复活和当前轮罚时。
    pub(super) fn resolve_death(&mut self, target: usize, action: &str) -> Result<(), String> {
        trace!(
            self,
            "death",
            target,
            Some(target),
            Some(action),
            None,
            None
        );
        self.dispatch(BattleEvent::Death, Some(target), Some(action))?;
        self.units[target].shield = 0.0;
        self.units[target].shield_turns = 0;
        if self.units[target].revivals_left > 0 {
            let r = self.units[target]
                .spec
                .revival
                .as_ref()
                .ok_or("revival state missing policy")?
                .clone();
            self.units[target].revivals_left -= 1;
            self.units[target].hp = finite(
                self.units[target].spec.stats.max_hp * r.hp_fraction,
                "revival hp",
            )?;
            // Revival preserves distance and clock; only current-cycle AV is charged.
            let _applied = self.timeline.penalize(r.av_penalty);
            trace!(
                self,
                "revive",
                target,
                Some(target),
                Some(action),
                Some(self.units[target].hp),
                None,
            );
            trace!(
                self,
                "revival_av_penalty",
                target,
                Some(target),
                Some(action),
                Some(_applied),
                None,
            );
        }
        Ok(())
    }
}
