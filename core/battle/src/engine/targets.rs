//! 按声明的选择器查找当前存活目标。
use super::*;

impl State<'_> {
    /// 按 target 取得当前存活目标，随机选择消耗目标随机流。
    pub(super) fn targets(&mut self, actor: usize, target: Target) -> Result<Vec<usize>, String> {
        if target == Target::ContextTarget {
            let index = self
                .context_target
                .ok_or("context target used outside a retarget scope")?;
            return Ok(if self.units[index].alive() {
                vec![index]
            } else {
                vec![]
            });
        }
        if target == Target::AbilityTarget {
            return Ok(self
                .bound_target
                .into_iter()
                .filter(|&i| self.units[i].alive())
                .collect());
        }
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
            Target::EnemyRandomUniform => Ok(vec![
                candidates[(uniform(&mut self.targets) * candidates.len() as f64) as usize],
            ]),
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
            Target::AbilityTarget => unreachable!("bound target handled above"),
            Target::ContextTarget => unreachable!("context target handled above"),
            Target::SelfUnit => unreachable!("self target handled before candidate selection"),
        }
    }
    // Called only by the hit that changed this unit's HP from positive to zero.
    // Do not infer a new death from HP observed after a nested listener returns.
}
