//! Validated input and result contracts shared by the CLI and library callers.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Effective values, already composed by the caller. No account data is guessed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub level: u16,
    pub max_hp: f64,
    pub attack: f64,
    pub defense: f64,
    pub speed: f64,
    pub taunt: f64,
    pub crit_rate: f64,
    pub crit_damage: f64,
    pub damage_bonus: f64,
    pub defense_reduction: f64,
    pub defense_ignore: f64,
    pub resistance: f64,
    pub resistance_penetration: f64,
    pub vulnerability: f64,
    pub damage_reduction: f64,
    pub independent_multiplier: f64,
    pub toughness_broken: bool,
    pub healing_bonus: f64,
    pub incoming_healing_bonus: f64,
    pub shield_bonus: f64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    Attack,
    Defense,
    MaxHp,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    EnemyLowestHp,
    EnemyRandomTaunt,
    AllEnemies,
    AllyLowestHp,
    AllAllies,
    SelfUnit,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Damage {
        scale: Scale,
        ratio: f64,
        flat: f64,
        hits: u8,
        can_crit: bool,
        target: Target,
    },
    Heal {
        scale: Scale,
        ratio: f64,
        flat: f64,
        target: Target,
    },
    Shield {
        scale: Scale,
        ratio: f64,
        flat: f64,
        max_total: f64,
        duration_turns: u16,
        target: Target,
    },
    Energy {
        amount: f64,
        target: Target,
    },
    Advance {
        fraction: f64,
        target: Target,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Basic,
    Skill,
    Ultimate,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub kind: ActionKind,
    pub sp_cost: u8,
    pub sp_gain: u8,
    pub energy_cost: f64,
    pub energy_gain: f64,
    pub effects: Vec<Effect>,
}
/// Optional explicit model of immediate revival. It is not inferred from game data.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Revival {
    pub charges: u8,
    pub hp_fraction: f64,
    pub av_penalty: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    pub id: String,
    pub stats: Stats,
    pub initial_hp: f64,
    pub initial_energy: f64,
    pub max_energy: f64,
    pub revival: Option<Revival>,
    /// First affordable action wins, with ultimates considered at action boundaries.
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPoints {
    pub initial: u8,
    pub maximum: u8,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub game_version: String,
    pub data_snapshot: String,
    pub description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub schema_version: u32,
    /// The only implemented profile is basic_v1; it is not a full character catalog.
    pub ruleset: String,
    pub provenance: Provenance,
    pub unsupported_mechanics: Vec<String>,
    pub max_av: f64,
    pub max_actions: u32,
    pub ally_skill_points: SkillPoints,
    pub enemy_skill_points: SkillPoints,
    pub allies: Vec<Unit>,
    pub waves: Vec<Vec<Unit>>,
}
/// Constructed only after validation. Callers cannot mutate the validated input.
#[derive(Clone, Debug)]
pub struct ValidatedScenario {
    pub(crate) scenario: Scenario,
    pub(crate) fingerprint: String,
}
impl ValidatedScenario {
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Victory,
    Defeat,
    Draw,
    Incomplete,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    EnemiesDefeated,
    AlliesDefeated,
    BothDefeated,
    AvBudgetExhausted,
    ActionLimit,
    Cancelled,
}
#[derive(Clone, Debug, Serialize)]
pub struct UnitResult {
    pub id: String,
    pub ally: bool,
    pub hp: f64,
    pub shield: f64,
    pub energy: f64,
    pub alive: bool,
    pub revivals_left: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub index: u32,
    pub av: f64,
    pub kind: String,
    pub actor: String,
    pub target: Option<String>,
    pub action: Option<String>,
    pub amount: Option<f64>,
    pub critical: Option<bool>,
}
#[derive(Clone, Debug, Serialize)]
pub struct BattleResult {
    pub scenario_sha256: String,
    #[serde(serialize_with = "seed_decimal::serialize")]
    pub seed: u64,
    pub ruleset: String,
    pub outcome: Outcome,
    pub termination: Termination,
    pub elapsed_av: f64,
    pub penalty_av: f64,
    pub actions: u32,
    pub waves_cleared: usize,
    pub ally_skill_points: u8,
    pub enemy_skill_points: u8,
    pub units: Vec<UnitResult>,
    pub events: Vec<Event>,
    pub trace_truncated: bool,
    /// Shared run settlement is deliberately outside the basic battle engine.
    pub run_hp_loss: Option<f64>,
    pub validation_scope: String,
}

/// JSON transports full-width seeds as strings so TypeScript cannot round them.
pub mod seed_decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.is_empty() || !text.bytes().all(|c| c.is_ascii_digit()) {
            return Err(serde::de::Error::custom(
                "seed must be a decimal u64 string",
            ));
        }
        text.parse().map_err(serde::de::Error::custom)
    }
}

fn bounded(value: f64, min: f64, max: f64, name: &str) -> Result<(), String> {
    if !value.is_finite() || value < min || value > max {
        return Err(format!("invalid {name}: must be finite in [{min}, {max}]"));
    }
    Ok(())
}
fn identifier(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 200 || value.trim() != value {
        return Err(format!("invalid {name}"));
    }
    Ok(())
}
fn validate_unit(unit: &Unit, sp: &SkillPoints) -> Result<(), String> {
    identifier(&unit.id, "unit id")?;
    let s = &unit.stats;
    if s.level == 0 || s.level > 1000 {
        return Err("invalid level".into());
    }
    for (value, name) in [(s.max_hp, "max_hp"), (s.speed, "speed"), (s.taunt, "taunt")] {
        bounded(value, 0.000001, 1e12, name)?;
    }
    for (value, name) in [
        (s.attack, "attack"),
        (s.defense, "defense"),
        (s.crit_damage, "crit_damage"),
        (s.defense_reduction, "defense_reduction"),
        (s.defense_ignore, "defense_ignore"),
        (s.independent_multiplier, "independent_multiplier"),
        (s.healing_bonus, "healing_bonus"),
        (s.incoming_healing_bonus, "incoming_healing_bonus"),
        (s.shield_bonus, "shield_bonus"),
    ] {
        bounded(value, 0.0, 1e12, name)?;
    }
    for (value, name) in [
        (s.damage_bonus, "damage_bonus"),
        (s.resistance, "resistance"),
        (s.resistance_penetration, "resistance_penetration"),
        (s.vulnerability, "vulnerability"),
    ] {
        bounded(value, -1.0, 1e6, name)?;
    }
    bounded(s.crit_rate, 0.0, 1.0, "crit_rate")?;
    bounded(s.damage_reduction, 0.0, 1.0, "damage_reduction")?;
    bounded(unit.initial_hp, 0.000001, s.max_hp, "initial_hp")?;
    bounded(unit.max_energy, 0.0, 1e9, "max_energy")?;
    bounded(unit.initial_energy, 0.0, unit.max_energy, "initial_energy")?;
    if let Some(r) = &unit.revival {
        bounded(r.hp_fraction, 0.000001, 1.0, "revival.hp_fraction")?;
        bounded(r.av_penalty, 0.0, 1e12, "revival.av_penalty")?;
    }
    if unit.actions.is_empty() || unit.actions.len() > 16 {
        return Err("actions must contain 1..16 entries".into());
    }
    let mut ids = BTreeSet::new();
    let mut fallback = false;
    for a in &unit.actions {
        identifier(&a.id, "action id")?;
        if !ids.insert(&a.id) {
            return Err("duplicate action id".into());
        }
        if a.sp_cost > sp.maximum || a.sp_gain > sp.maximum {
            return Err("action skill points exceed team maximum".into());
        }
        bounded(a.energy_cost, 0.0, unit.max_energy, "energy_cost")?;
        bounded(a.energy_gain, 0.0, 1e9, "energy_gain")?;
        if a.kind == ActionKind::Ultimate && a.energy_cost <= 0.0 {
            return Err("ultimate must cost positive energy".into());
        }
        if a.kind != ActionKind::Ultimate && a.sp_cost == 0 && a.energy_cost == 0.0 {
            fallback = true;
        }
        if a.effects.is_empty() || a.effects.len() > 32 {
            return Err("effects must contain 1..32 entries".into());
        }
        for e in &a.effects {
            match e {
                Effect::Damage {
                    ratio,
                    flat,
                    hits,
                    target,
                    ..
                } => {
                    bounded(*ratio, 0.0, 1e6, "damage ratio")?;
                    bounded(*flat, 0.0, 1e12, "damage flat")?;
                    if *hits == 0 || *hits > 100 {
                        return Err("damage hits must be 1..100".into());
                    }
                    if !matches!(
                        target,
                        Target::EnemyLowestHp | Target::EnemyRandomTaunt | Target::AllEnemies
                    ) {
                        return Err("damage requires enemy targets".into());
                    }
                }
                Effect::Heal {
                    ratio,
                    flat,
                    target,
                    ..
                }
                | Effect::Shield {
                    ratio,
                    flat,
                    target,
                    ..
                } => {
                    bounded(*ratio, 0.0, 1e6, "support ratio")?;
                    bounded(*flat, 0.0, 1e12, "support flat")?;
                    if !matches!(
                        target,
                        Target::AllyLowestHp | Target::AllAllies | Target::SelfUnit
                    ) {
                        return Err("support requires ally targets".into());
                    }
                    if let Effect::Shield {
                        max_total,
                        duration_turns,
                        ..
                    } = e
                    {
                        bounded(*max_total, 0.0, 1e15, "shield max_total")?;
                        if *duration_turns == 0 {
                            return Err("shield duration_turns must be positive".into());
                        }
                    }
                }
                Effect::Energy { amount, .. } => bounded(*amount, -1e9, 1e9, "energy amount")?,
                Effect::Advance { fraction, .. } => {
                    bounded(*fraction, -1.0, 1.0, "advance fraction")?
                }
            }
        }
    }
    if !fallback {
        return Err("unit requires a free non-ultimate fallback action".into());
    }
    Ok(())
}
/// Validate once before running any trials; unsupported mechanisms fail closed.
pub fn validate(scenario: Scenario) -> Result<ValidatedScenario, String> {
    use sha2::{Digest, Sha256};
    if scenario.schema_version != 1 || scenario.ruleset != "basic_v1" {
        return Err("unsupported schema_version or ruleset".into());
    }
    for (value, name) in [
        (&scenario.provenance.game_version, "game_version"),
        (&scenario.provenance.data_snapshot, "data_snapshot"),
        (&scenario.provenance.description, "description"),
    ] {
        identifier(value, name)?;
    }
    if !scenario.unsupported_mechanics.is_empty() {
        return Err(format!(
            "unsupported mechanics: {}",
            scenario.unsupported_mechanics.join(", ")
        ));
    }
    bounded(scenario.max_av, 0.000001, 1e9, "max_av")?;
    if scenario.max_actions == 0 || scenario.max_actions > 1_000_000 {
        return Err("max_actions must be 1..1000000".into());
    }
    for sp in [&scenario.ally_skill_points, &scenario.enemy_skill_points] {
        if sp.maximum > 20 || sp.initial > sp.maximum {
            return Err("invalid team skill point limits".into());
        }
    }
    if scenario.allies.is_empty()
        || scenario.allies.len() > 32
        || scenario.waves.is_empty()
        || scenario.waves.len() > 100
    {
        return Err("invalid allies/wave count".into());
    }
    let mut ids = BTreeSet::new();
    for u in &scenario.allies {
        validate_unit(u, &scenario.ally_skill_points)?;
        if !ids.insert(&u.id) {
            return Err("unit ids must be globally unique".into());
        }
    }
    for wave in &scenario.waves {
        if wave.is_empty() || wave.len() > 32 {
            return Err("enemy wave must contain 1..32 units".into());
        }
        for u in wave {
            validate_unit(u, &scenario.enemy_skill_points)?;
            if !ids.insert(&u.id) {
                return Err("unit ids must be globally unique".into());
            }
        }
    }
    let encoded = serde_json::to_vec(&scenario).map_err(|e| e.to_string())?;
    Ok(ValidatedScenario {
        scenario,
        fingerprint: format!("{:x}", Sha256::digest(encoded)),
    })
}
