//! 逐命中计算属性、暴击和伤害，再分派命中与死亡事件。
use super::*;

impl State<'_> {
    /// 逐命中读取当前属性并结算伤害、护盾、生命和监听事件。
    pub(super) fn damage_hits(
        &mut self,
        actor: usize,
        action: &str,
        base: f64,
        hits: u8,
        can_crit: bool,
        target: Target,
    ) -> Result<(), String> {
        let factor = if self.units[actor].ally {
            self.input.mechanics.ally_damage_factor
        } else {
            1.0
        };
        let valid_factor = factor.is_finite() && factor >= 0.0;
        for _ in 0..hits {
            for victim in self.targets(actor, target)? {
                // Earlier recipients can dispatch immediate damage. A
                // cached AoE target list is not proof this victim lives.
                if !self.units[victim].alive() {
                    continue;
                }
                // Validate only at a hit. A negative factor would grant shield.
                if !valid_factor {
                    return Err("ally_damage_factor must be finite and non-negative".into());
                }
                if self.attack_owner == Some(actor) && !self.attack_targets.contains(&victim) {
                    self.attack_targets.push(victim);
                }
                // A prior hit's callbacks may have changed active properties.
                let mut attacker = self.units[actor].spec.stats.clone();
                attacker.crit_damage = finite(
                    attacker.crit_damage
                        + self.units[actor].modifiers.delta(Property::CritDamage)?,
                    "modified crit damage",
                )?;
                attacker.damage_bonus = finite(
                    attacker.damage_bonus
                        + self.units[actor].modifiers.delta(Property::DamageBonus)?,
                    "modified damage bonus",
                )?;
                let defender = &self.units[victim].spec.stats;
                let resistance = if let Some(element) = self.damage_element {
                    let profiles = &self.input.mechanics.element_profiles;
                    let source = profiles
                        .get(&self.units[actor].spec.id)
                        .ok_or("missing attack element panel")?;
                    let target = profiles
                        .get(&self.units[victim].spec.id)
                        .ok_or("missing defense element panel")?;
                    attacker.damage_bonus = finite(
                        attacker.damage_bonus
                            + source
                                .damage_bonus
                                .get(&element)
                                .ok_or("missing element damage bonus")?,
                        "element damage bonus",
                    )?;
                    *target
                        .resistance
                        .get(&element)
                        .ok_or("missing element resistance")?
                } else {
                    defender.resistance
                };
                let critical = can_crit && uniform(&mut self.criticals) < attacker.crit_rate;
                let amount = math::damage(
                    &math::DamageInput {
                        base_damage: base,
                        attacker_level: attacker.level,
                        defense: defender.defense,
                        defense_reduction: attacker.defense_reduction,
                        defense_ignore: attacker.defense_ignore,
                        resistance,
                        resistance_penetration: attacker.resistance_penetration,
                        damage_bonus: attacker.damage_bonus,
                        vulnerability: defender.vulnerability,
                        toughness_broken: defender.toughness_broken,
                        damage_reduction: defender.damage_reduction,
                        independent_multiplier: attacker.independent_multiplier,
                        crit_damage: attacker.crit_damage,
                    },
                    critical,
                )? * factor;
                finite(amount, "mode damage")?;
                let absorbed = amount.min(self.units[victim].shield);
                self.units[victim].shield -= absorbed;
                let hp_loss = (amount - absorbed).min(self.units[victim].hp);
                self.units[victim].hp -= hp_loss;
                let died_from_this_hit = self.units[victim].hp == 0.0;
                trace!(
                    self,
                    "damage",
                    actor,
                    Some(victim),
                    Some(action),
                    Some(amount),
                    Some(critical),
                );
                trace!(
                    self,
                    "shield_absorbed",
                    actor,
                    Some(victim),
                    Some(action),
                    Some(absorbed),
                    None,
                );
                trace!(
                    self,
                    "hp_loss",
                    actor,
                    Some(victim),
                    Some(action),
                    Some(hp_loss),
                    None,
                );
                // The active element belongs to this hit, not nested hits
                // dispatched by listeners. Such hits select their own panel.
                let active_element = self.damage_element.take();
                let active_attack_type = self.damage_attack_type.take();
                let prior_event_attack_type = self
                    .event_attack_type
                    .replace(active_attack_type.unwrap_or(AttackType::Direct));
                let event_result: Result<(), String> = (|| {
                    self.dispatch(BattleEvent::Hit, Some(actor), Some(action))?;
                    if died_from_this_hit {
                        self.resolve_death(victim, action)?;
                    }
                    Ok(())
                })();
                self.damage_element = active_element;
                self.damage_attack_type = active_attack_type;
                self.event_attack_type = prior_event_attack_type;
                event_result?;
            }
        }
        Ok(())
    }
}
