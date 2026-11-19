//! 读取动态表达式与运行时条件。
use super::*;

impl State<'_> {
    /// 使用调用方提供的变量读取函数求值，算术失败返回原因。
    pub(super) fn evaluate(
        &self,
        owner: usize,
        value: &crate::expression::Expression,
    ) -> Result<f64, String> {
        value.evaluate_with_scopes(
            &|key| self.units[owner].dynamic_values.get(key).copied(),
            &|scope, key| self.dynamic_scopes.get(owner, scope, key),
        )
    }

    /// 读取当前动作和命中上下文，短路计算条件树。
    pub(super) fn condition(&self, actor: usize, condition: &Condition) -> Result<bool, String> {
        Ok(match condition {
            Condition::HasModifier { id, target } => {
                self.units.iter().enumerate().any(|(i, u)| {
                    let selected = match target {
                        Target::SelfUnit => i == actor,
                        Target::AbilityTarget => Some(i) == self.bound_target,
                        Target::AllEnemies => u.ally != self.units[actor].ally,
                        _ => false, // rejected by input validation
                    };
                    selected && u.alive() && u.modifiers.instances.iter().any(|m| m.state.id == *id)
                })
            }
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                let l = self.evaluate(actor, left)?;
                let r = self.evaluate(actor, right)?;
                match comparison {
                    Comparison::Equal => l == r,
                    Comparison::NotEqual => l != r,
                    Comparison::Greater => l > r,
                    Comparison::GreaterEqual => l >= r,
                    Comparison::Less => l < r,
                    Comparison::LessEqual => l <= r,
                }
            }
            Condition::ActionKind { kind } => self.current_action_kind == Some(*kind),
            Condition::HitAttackType { attack_type } => {
                self.event_attack_type == Some(*attack_type)
            }
            Condition::Not { condition } => !self.condition(actor, condition)?,
            Condition::All { conditions } => {
                let mut yes = true;
                for c in conditions {
                    if !self.condition(actor, c)? {
                        yes = false;
                        break;
                    }
                }
                yes
            }
            Condition::Any { conditions } => {
                let mut yes = false;
                for c in conditions {
                    if self.condition(actor, c)? {
                        yes = true;
                        break;
                    }
                }
                yes
            }
        })
    }
}
