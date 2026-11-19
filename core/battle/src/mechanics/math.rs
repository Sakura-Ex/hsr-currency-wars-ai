//! 伤害、治疗与护盾的确定性数值计算。
//!
//! 调用方组装基础值、加成和暴击结果；本模块返回伤害、治疗和护盾数值。

#[derive(Debug, Clone)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 一次直接伤害所需的已组装乘区参数。
pub struct DamageInput {
    /// 已经合成的单次基础伤害。
    pub base_damage: f64,
    /// 攻击方等级。
    pub attacker_level: u16,
    /// 静态防御力或本击目标防御。
    pub defense: f64,
    /// 攻击方携带的有效减防比例。
    pub defense_reduction: f64,
    /// 攻击方无视防御的比例。
    pub defense_ignore: f64,
    /// 目标抗性或按元素关联的完整抗性。
    pub resistance: f64,
    /// 攻击方的抗性穿透。
    pub resistance_penetration: f64,
    /// Aggregated additive bonus ratio, e.g. 0.2 means +20%.
    pub damage_bonus: f64,
    /// 目标易伤比例。
    pub vulnerability: f64,
    /// 目标是否处于韧性击破状态。
    pub toughness_broken: bool,
    /// Aggregated damage-reduction ratio, not the retained multiplier.
    pub damage_reduction: f64,
    /// Already resolved independent multiplier; neutral is 1, not 0.
    pub independent_multiplier: f64,
    /// 暴击伤害加成，暴击倍率为一加此值。
    pub crit_damage: f64,
}

/// 返回有限计算结果，遇到 NaN 或无穷值时附带字段名报错。
fn finite(name: &str, value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{name} must be finite"))
    }
}

/// 按约定顺序应用已组装的直接伤害乘区，返回有限的伤害量。
///
/// 参数的静态范围由组装方保证，运行时检查合计增伤和暴伤的下界。
/// 暴击结果由调用方采样；沿用参考实现的相乘顺序，保留浮点精度。
pub fn damage(input: &DamageInput, critical: bool) -> Result<f64, String> {
    // 这两项可由运行中的 Modifier 与元素增伤合成，静态配置无法保证合计下界。
    if input.damage_bonus < -1.0 {
        return Err("combined damage bonus must be >= -1".into());
    }
    if input.crit_damage < 0.0 {
        return Err("combined critical damage must be >= 0".into());
    }
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

    // 当前接口中的最终伤害与虚弱乘区取 1。
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
    Ok(amount)
}

/// 对已组装的治疗基础值应用治疗加成与受治疗加成。
/// 两项加成的静态范围由组装方保证，运行时检查合计治疗乘数非负。
pub fn heal(base_amount: f64, outgoing_boost: f64, incoming_boost: f64) -> Result<f64, String> {
    let multiplier = finite("healing multiplier", 1.0 + outgoing_boost + incoming_boost)?;
    if multiplier < 0.0 {
        return Err("negative healing multiplier".into());
    }
    finite("healing amount", base_amount * multiplier)
}

/// 对已组装的护盾基础值应用护盾加成。
pub fn shield(base_amount: f64, outgoing_boost: f64) -> Result<f64, String> {
    let multiplier = finite("shield multiplier", 1.0 + outgoing_boost)?;
    finite("shield amount", base_amount * multiplier)
}
