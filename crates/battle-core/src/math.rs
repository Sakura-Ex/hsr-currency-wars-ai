//! Deterministic settlement math adapted from HSR_Nous, not a full rule engine.
//!
//! Upstream commit: f8935ddb407b297732fbd3c624d55897e3e648b9.
//! Source: src/hsr_nous/sim_schema/rulebook.yaml, lines 234-238, 262-266,
//! 279-375. SHA-256: d37c1b0a9dbe3819cd210e02bef96db9598486ab9b8554bad90602ca136277fe.
//! The direct-damage subset takes an already assembled base and bonus pool.
//! Upstream final-damage and weaken zones are neutral (1) in this interface.
//! Critical outcome is provided by the caller; this module consumes no RNG.
//! Healing/shielding return raw amounts; application, caps, duration, stacking,
//! target selection, costs, and resource gains belong to the simulation layer.
//!
//! MIT License
//!
//! Copyright (c) 2026 The HSR_Nous Authors
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DamageInput {
    pub base_damage: f64,
    pub attacker_level: u16,
    pub defense: f64,
    pub defense_reduction: f64,
    pub defense_ignore: f64,
    pub resistance: f64,
    pub resistance_penetration: f64,
    /// Aggregated additive bonus ratio, e.g. 0.2 means +20%.
    pub damage_bonus: f64,
    pub vulnerability: f64,
    pub toughness_broken: bool,
    /// Aggregated damage-reduction ratio, not the retained multiplier.
    pub damage_reduction: f64,
    /// Already resolved independent multiplier; neutral is 1, not 0.
    pub independent_multiplier: f64,
    pub crit_damage: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DamageBreakdown {
    pub base_damage: f64,
    pub damage_bonus_multiplier: f64,
    pub independent_multiplier: f64,
    pub defense_multiplier: f64,
    pub resistance_multiplier: f64,
    pub toughness_multiplier: f64,
    pub vulnerability_multiplier: f64,
    pub critical_multiplier: f64,
    pub damage_reduction_multiplier: f64,
    pub critical: bool,
    pub amount: f64,
}

fn finite(name: &str, value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{name} must be finite"))
    }
}

fn at_least(name: &str, value: f64, lower: f64) -> Result<(), String> {
    finite(name, value)?;
    if value < lower {
        return Err(format!("{name} must be >= {lower}"));
    }
    Ok(())
}

fn ratio(name: &str, value: f64) -> Result<(), String> {
    at_least(name, value, 0.0)?;
    if value > 1.0 {
        return Err(format!("{name} must be <= 1"));
    }
    Ok(())
}

/// Validate inputs, then apply the upstream direct-damage factors in their
/// original multiplication order. No rounding to display integers is performed.
pub fn damage(input: &DamageInput, critical: bool) -> Result<DamageBreakdown, String> {
    at_least("base_damage", input.base_damage, 0.0)?;
    if input.attacker_level == 0 {
        return Err("attacker_level must be positive".into());
    }
    at_least("defense", input.defense, 0.0)?;
    at_least("defense_reduction", input.defense_reduction, 0.0)?;
    at_least("defense_ignore", input.defense_ignore, 0.0)?;
    finite("resistance", input.resistance)?;
    finite("resistance_penetration", input.resistance_penetration)?;
    at_least("damage_bonus", input.damage_bonus, -1.0)?;
    at_least("vulnerability", input.vulnerability, -1.0)?;
    ratio("damage_reduction", input.damage_reduction)?;
    at_least("independent_multiplier", input.independent_multiplier, 0.0)?;
    at_least("crit_damage", input.crit_damage, 0.0)?;

    let level_term = f64::from(input.attacker_level) * 10.0 + 200.0;
    let defense_penetration = finite(
        "combined defense penetration",
        input.defense_reduction + input.defense_ignore,
    )?;
    let defense_remaining = (1.0 - defense_penetration).max(0.0);
    let defense_multiplier = level_term
        / finite(
            "defense denominator",
            input.defense * defense_remaining + level_term,
        )?;
    let effective_resistance = finite(
        "effective resistance",
        input.resistance - input.resistance_penetration,
    )?;
    let resistance_multiplier = 1.0 - effective_resistance.clamp(-1.0, 0.9);
    let damage_bonus_multiplier = finite("damage bonus multiplier", 1.0 + input.damage_bonus)?;
    let vulnerability_multiplier = finite("vulnerability multiplier", 1.0 + input.vulnerability)?;
    let toughness_multiplier = if input.toughness_broken { 1.0 } else { 0.9 };
    let critical_multiplier = if critical {
        finite("critical multiplier", 1.0 + input.crit_damage)?
    } else {
        1.0
    };
    let damage_reduction_multiplier = 1.0 - input.damage_reduction;

    // final_dmg_multi and weaken_multi are deliberately neutral in this subset.
    let mut amount = input.base_damage;
    for factor in [
        damage_bonus_multiplier,
        input.independent_multiplier,
        defense_multiplier,
        resistance_multiplier,
        toughness_multiplier,
        vulnerability_multiplier,
        1.0,
        critical_multiplier,
        1.0,
        damage_reduction_multiplier,
    ] {
        amount = finite("damage product", amount * factor)?;
    }
    Ok(DamageBreakdown {
        base_damage: input.base_damage,
        damage_bonus_multiplier,
        independent_multiplier: input.independent_multiplier,
        defense_multiplier,
        resistance_multiplier,
        toughness_multiplier,
        vulnerability_multiplier,
        critical_multiplier,
        damage_reduction_multiplier,
        critical,
        amount,
    })
}

/// Upstream heal formula after the caller assembles its ATK/HP/flat base.
/// Negative incoming boost is supported down to -1 (e.g. healing received down).
/// The combined multiplier must remain nonnegative; no silent lower clamp.
pub fn heal(base_amount: f64, outgoing_boost: f64, incoming_boost: f64) -> Result<f64, String> {
    at_least("base_amount", base_amount, 0.0)?;
    at_least("outgoing_boost", outgoing_boost, -1.0)?;
    at_least("incoming_boost", incoming_boost, -1.0)?;
    let multiplier = finite("healing multiplier", 1.0 + outgoing_boost + incoming_boost)?;
    at_least("healing multiplier", multiplier, 0.0)?;
    finite("healing amount", base_amount * multiplier)
}

/// Upstream shield formula after the caller assembles its DEF/HP/ATK/flat base.
pub fn shield(base_amount: f64, outgoing_boost: f64) -> Result<f64, String> {
    at_least("base_amount", base_amount, 0.0)?;
    at_least("outgoing_boost", outgoing_boost, -1.0)?;
    let multiplier = finite("shield multiplier", 1.0 + outgoing_boost)?;
    finite("shield amount", base_amount * multiplier)
}
