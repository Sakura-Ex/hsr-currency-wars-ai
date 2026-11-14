//! Single-owner event loop for the explicit basic_v1 contract.
use crate::{math, model::*};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};

pub const RNG_ALGORITHM: &str =
    "ChaCha8Rng/rand_chacha=0.9.0; SHA256-separated target/critical streams v1";
const TRACE_LIMIT: usize = 100_000;

struct CombatUnit {
    spec: Unit,
    ally: bool,
    hp: f64,
    shield: f64,
    shield_turns: u16,
    energy: f64,
    next_av: f64,
    revivals_left: u8,
}
impl CombatUnit {
    fn new(spec: &Unit, ally: bool, now: f64) -> Self {
        Self {
            spec: spec.clone(),
            ally,
            hp: spec.initial_hp,
            shield: 0.0,
            shield_turns: 0,
            energy: spec.initial_energy,
            next_av: now + 10000.0 / spec.stats.speed,
            revivals_left: spec.revival.as_ref().map_or(0, |r| r.charges),
        }
    }
    fn alive(&self) -> bool {
        self.hp > 0.0
    }
    fn interval(&self) -> f64 {
        10000.0 / self.spec.stats.speed
    }
    fn scale(&self, scale: Scale) -> f64 {
        match scale {
            Scale::Attack => self.spec.stats.attack,
            Scale::Defense => self.spec.stats.defense,
            Scale::MaxHp => self.spec.stats.max_hp,
        }
    }
}
struct State<'a> {
    input: &'a ValidatedScenario,
    units: Vec<CombatUnit>,
    now: f64,
    penalty: f64,
    actions: u32,
    wave: usize,
    sp: [u8; 2],
    trace: bool,
    events: Vec<Event>,
    trace_truncated: bool,
    targets: ChaCha8Rng,
    criticals: ChaCha8Rng,
}
fn stream(seed: u64, domain: &[u8]) -> ChaCha8Rng {
    let mut hash = Sha256::new();
    hash.update(b"hcwa-battle-core/rng/v1\0");
    hash.update(domain);
    hash.update(seed.to_le_bytes());
    ChaCha8Rng::from_seed(hash.finalize().into())
}
fn uniform(rng: &mut ChaCha8Rng) -> f64 {
    ((rng.next_u64() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}
fn finite(value: f64, name: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("nonfinite {name}"))
    }
}
impl<'a> State<'a> {
    fn new(input: &'a ValidatedScenario, seed: u64, trace: bool) -> Self {
        let s = input.scenario();
        let mut units: Vec<_> = s
            .allies
            .iter()
            .map(|u| CombatUnit::new(u, true, 0.0))
            .collect();
        units.extend(s.waves[0].iter().map(|u| CombatUnit::new(u, false, 0.0)));
        Self {
            input,
            units,
            now: 0.0,
            penalty: 0.0,
            actions: 0,
            wave: 0,
            sp: [s.ally_skill_points.initial, s.enemy_skill_points.initial],
            trace,
            events: Vec::new(),
            trace_truncated: false,
            targets: stream(seed, b"targets"),
            criticals: stream(seed, b"criticals"),
        }
    }
    fn emit(
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
        self.events.push(Event {
            index: self.events.len() as u32,
            av: self.now,
            kind: kind.into(),
            actor: self.units[actor].spec.id.clone(),
            target: target.map(|i| self.units[i].spec.id.clone()),
            action: action.map(str::to_owned),
            amount,
            critical,
        });
    }
    fn side(&self, actor: usize) -> usize {
        usize::from(!self.units[actor].ally)
    }
    fn sp_max(&self, side: usize) -> u8 {
        if side == 0 {
            self.input.scenario().ally_skill_points.maximum
        } else {
            self.input.scenario().enemy_skill_points.maximum
        }
    }
    fn affordable(&self, actor: usize, action: &Action) -> bool {
        action.sp_cost <= self.sp[self.side(actor)]
            && action.energy_cost <= self.units[actor].energy
    }
    fn choose(&self, actor: usize, ultimate: bool) -> Option<usize> {
        self.units[actor]
            .spec
            .actions
            .iter()
            .position(|a| (a.kind == ActionKind::Ultimate) == ultimate && self.affordable(actor, a))
    }
    fn targets(&mut self, actor: usize, target: Target) -> Result<Vec<usize>, String> {
        if target == Target::SelfUnit {
            return Ok(if self.units[actor].alive() {
                vec![actor]
            } else {
                Vec::new()
            });
        }
        let allies = matches!(target, Target::AllyLowestHp | Target::AllAllies);
        let side = self.units[actor].ally;
        let candidates: Vec<_> = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.alive() && (u.ally == side) == allies)
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        match target {
            Target::AllAllies | Target::AllEnemies => Ok(candidates),
            Target::AllyLowestHp | Target::EnemyLowestHp => {
                let selected = candidates
                    .into_iter()
                    .min_by(|&a, &b| {
                        let left = self.units[a].hp / self.units[a].spec.stats.max_hp;
                        let right = self.units[b].hp / self.units[b].spec.stats.max_hp;
                        left.total_cmp(&right).then(a.cmp(&b))
                    })
                    .ok_or("target selection unexpectedly empty")?;
                Ok(vec![selected])
            }
            Target::EnemyRandomTaunt => {
                let total: f64 = candidates
                    .iter()
                    .map(|&i| self.units[i].spec.stats.taunt)
                    .sum();
                finite(total, "taunt sum")?;
                let mut draw = uniform(&mut self.targets) * total;
                for &i in &candidates {
                    let weight = self.units[i].spec.stats.taunt;
                    if draw < weight {
                        return Ok(vec![i]);
                    }
                    draw -= weight;
                }
                // A final ulp at the upper edge belongs to the last interval.
                Ok(vec![*candidates
                    .last()
                    .ok_or("target selection unexpectedly empty")?])
            }
            Target::SelfUnit => unreachable!("self target handled before candidate selection"),
        }
    }
    fn kill_or_revive(&mut self, target: usize, action: &str) -> Result<(), String> {
        if self.units[target].hp > 0.0 {
            return Ok(());
        }
        self.emit("death", target, Some(target), Some(action), None, None);
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
            self.units[target].next_av =
                finite(self.now + self.units[target].interval(), "revival schedule")?;
            self.penalty = finite(self.penalty + r.av_penalty, "revival penalty")?;
            self.emit(
                "revive",
                target,
                Some(target),
                Some(action),
                Some(self.units[target].hp),
                None,
            );
            self.emit(
                "revival_av_penalty",
                target,
                Some(target),
                Some(action),
                Some(r.av_penalty),
                None,
            );
        }
        Ok(())
    }
    fn apply_effect(&mut self, actor: usize, action: &str, effect: &Effect) -> Result<(), String> {
        match *effect {
            Effect::Damage {
                scale,
                ratio,
                flat,
                hits,
                can_crit,
                target,
            } => {
                let attacker = self.units[actor].spec.stats.clone();
                let base = finite(self.units[actor].scale(scale) * ratio + flat, "base damage")?;
                for _ in 0..hits {
                    for victim in self.targets(actor, target)? {
                        let defender = &self.units[victim].spec.stats;
                        let critical =
                            can_crit && uniform(&mut self.criticals) < attacker.crit_rate;
                        let amount = math::damage(
                            &math::DamageInput {
                                base_damage: base,
                                attacker_level: attacker.level,
                                defense: defender.defense,
                                defense_reduction: attacker.defense_reduction,
                                defense_ignore: attacker.defense_ignore,
                                resistance: defender.resistance,
                                resistance_penetration: attacker.resistance_penetration,
                                damage_bonus: attacker.damage_bonus,
                                vulnerability: defender.vulnerability,
                                toughness_broken: defender.toughness_broken,
                                damage_reduction: defender.damage_reduction,
                                independent_multiplier: attacker.independent_multiplier,
                                crit_damage: attacker.crit_damage,
                            },
                            critical,
                        )?
                        .amount;
                        let absorbed = amount.min(self.units[victim].shield);
                        self.units[victim].shield -= absorbed;
                        let hp_loss = (amount - absorbed).min(self.units[victim].hp);
                        self.units[victim].hp -= hp_loss;
                        self.emit(
                            "damage",
                            actor,
                            Some(victim),
                            Some(action),
                            Some(amount),
                            Some(critical),
                        );
                        self.emit(
                            "shield_absorbed",
                            actor,
                            Some(victim),
                            Some(action),
                            Some(absorbed),
                            None,
                        );
                        self.emit(
                            "hp_loss",
                            actor,
                            Some(victim),
                            Some(action),
                            Some(hp_loss),
                            None,
                        );
                        self.kill_or_revive(victim, action)?;
                    }
                }
            }
            Effect::Heal {
                scale,
                ratio,
                flat,
                target,
            } => {
                let base = finite(
                    self.units[actor].scale(scale) * ratio + flat,
                    "base healing",
                )?;
                for recipient in self.targets(actor, target)? {
                    let raw = math::heal(
                        base,
                        self.units[actor].spec.stats.healing_bonus,
                        self.units[recipient].spec.stats.incoming_healing_bonus,
                    )?;
                    let actual =
                        raw.min(self.units[recipient].spec.stats.max_hp - self.units[recipient].hp);
                    self.units[recipient].hp += actual;
                    self.emit(
                        "heal",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(actual),
                        None,
                    );
                }
            }
            Effect::Shield {
                scale,
                ratio,
                flat,
                max_total,
                duration_turns,
                target,
            } => {
                let base = finite(self.units[actor].scale(scale) * ratio + flat, "base shield")?;
                let raw = math::shield(base, self.units[actor].spec.stats.shield_bonus)?;
                for recipient in self.targets(actor, target)? {
                    let before = self.units[recipient].shield;
                    // A lower cap never removes a pre-existing stronger shield.
                    let actual = raw.min((max_total - before).max(0.0));
                    self.units[recipient].shield += actual;
                    self.units[recipient].shield_turns =
                        self.units[recipient].shield_turns.max(duration_turns);
                    self.emit(
                        "shield",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(actual),
                        None,
                    );
                }
            }
            Effect::Energy { amount, target } => {
                for recipient in self.targets(actor, target)? {
                    let before = self.units[recipient].energy;
                    self.units[recipient].energy =
                        (before + amount).clamp(0.0, self.units[recipient].spec.max_energy);
                    self.emit(
                        "energy",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(self.units[recipient].energy - before),
                        None,
                    );
                }
            }
            Effect::Advance { fraction, target } => {
                for recipient in self.targets(actor, target)? {
                    let before = self.units[recipient].next_av;
                    self.units[recipient].next_av = finite(
                        (before - fraction * self.units[recipient].interval()).max(self.now),
                        "action advance",
                    )?;
                    self.emit(
                        "advance",
                        actor,
                        Some(recipient),
                        Some(action),
                        Some(before - self.units[recipient].next_av),
                        None,
                    );
                }
            }
        }
        Ok(())
    }
    fn act(&mut self, actor: usize, action_index: usize, normal: bool) -> Result<(), String> {
        let action = self.units[actor].spec.actions[action_index].clone();
        if !self.affordable(actor, &action) {
            return Err("selected action is not affordable".into());
        }
        if normal {
            if self.units[actor].shield_turns > 0 {
                self.units[actor].shield_turns -= 1;
                if self.units[actor].shield_turns == 0 {
                    let expired = self.units[actor].shield;
                    self.units[actor].shield = 0.0;
                    self.emit(
                        "shield_expired",
                        actor,
                        Some(actor),
                        None,
                        Some(expired),
                        None,
                    );
                }
            }
            self.units[actor].next_av =
                finite(self.now + self.units[actor].interval(), "normal schedule")?;
        }
        let side = self.side(actor);
        self.sp[side] -= action.sp_cost;
        self.units[actor].energy -= action.energy_cost;
        self.actions += 1;
        self.emit("action", actor, None, Some(&action.id), None, None);
        for effect in &action.effects {
            self.apply_effect(actor, &action.id, effect)?;
        }
        if self.units[actor].alive() {
            self.sp[side] = self.sp[side]
                .saturating_add(action.sp_gain)
                .min(self.sp_max(side));
            self.units[actor].energy = (self.units[actor].energy + action.energy_gain)
                .min(self.units[actor].spec.max_energy);
        }
        Ok(())
    }
    fn finish(
        self,
        seed: u64,
        outcome: Outcome,
        termination: Termination,
        waves_cleared: usize,
    ) -> BattleResult {
        BattleResult {
            scenario_sha256: self.input.fingerprint().into(),
            seed,
            ruleset: "basic_v1".into(),
            outcome,
            termination,
            elapsed_av: self.now,
            penalty_av: self.penalty,
            actions: self.actions,
            waves_cleared,
            ally_skill_points: self.sp[0],
            enemy_skill_points: self.sp[1],
            units: self
                .units
                .into_iter()
                .map(|u| UnitResult {
                    id: u.spec.id,
                    ally: u.ally,
                    hp: u.hp,
                    shield: u.shield,
                    energy: u.energy,
                    alive: u.hp > 0.0,
                    revivals_left: u.revivals_left,
                })
                .collect(),
            events: self.events,
            trace_truncated: self.trace_truncated,
            run_hp_loss: None,
            validation_scope: "explicit_basic_profile_not_full_game_or_run_settlement".into(),
        }
    }
}

/// Run a single scenario without cancellation; the input remains unchanged.
pub fn simulate(input: &ValidatedScenario, seed: u64, trace: bool) -> Result<BattleResult, String> {
    simulate_cancellable(input, seed, trace, &mut || false)
}
/// Check cancellation before each action, including immediate ultimate chains.
pub fn simulate_cancellable(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BattleResult, String> {
    let mut state = State::new(input, seed, trace);
    loop {
        let allies = state.units.iter().any(|u| u.ally && u.alive());
        let enemies = state.units.iter().any(|u| !u.ally && u.alive());
        let cleared = state.wave + usize::from(!enemies);
        if !allies && !enemies {
            return Ok(state.finish(seed, Outcome::Draw, Termination::BothDefeated, cleared));
        }
        if !allies {
            return Ok(state.finish(seed, Outcome::Defeat, Termination::AlliesDefeated, cleared));
        }
        if !enemies {
            if state.wave + 1 == input.scenario().waves.len() {
                return Ok(state.finish(
                    seed,
                    Outcome::Victory,
                    Termination::EnemiesDefeated,
                    cleared,
                ));
            }
            state.wave += 1;
            state.units.extend(
                input.scenario().waves[state.wave]
                    .iter()
                    .map(|u| CombatUnit::new(u, false, state.now)),
            );
        }
        if cancel() {
            return Ok(state.finish(seed, Outcome::Incomplete, Termination::Cancelled, cleared));
        }
        if state.actions >= input.scenario().max_actions {
            return Ok(state.finish(seed, Outcome::Incomplete, Termination::ActionLimit, cleared));
        }
        if state.now + state.penalty >= input.scenario().max_av {
            return Ok(state.finish(
                seed,
                Outcome::Defeat,
                Termination::AvBudgetExhausted,
                cleared,
            ));
        }
        let ultimate = state
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.alive())
            .find_map(|(i, _)| state.choose(i, true).map(|a| (i, a)));
        if let Some((actor, action)) = ultimate {
            state.act(actor, action, false)?;
            continue;
        }
        let actor = state
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.alive())
            .min_by(|(ia, a), (ib, b)| a.next_av.total_cmp(&b.next_av).then(ia.cmp(ib)))
            .map(|(i, _)| i)
            .ok_or("no living units after terminal check")?;
        let time = state.units[actor].next_av;
        if time + state.penalty >= input.scenario().max_av {
            state.now = (input.scenario().max_av - state.penalty).max(state.now);
            return Ok(state.finish(
                seed,
                Outcome::Defeat,
                Termination::AvBudgetExhausted,
                cleared,
            ));
        }
        if time < state.now {
            return Err("action clock moved backwards".into());
        }
        state.now = time;
        let action = state
            .choose(actor, false)
            .ok_or("no affordable normal action despite validated fallback")?;
        state.act(actor, action, true)?;
    }
}
