//! 战斗拥有的可变状态、单位入场与有限队列。
use super::*;

/// 单位的本场可变状态，初始定义在每场开始时复制。
pub(super) struct CombatUnit {
    /// 原始定义的本场副本。
    pub(super) spec: Unit,
    /// 是否属于我方。
    pub(super) ally: bool,
    /// 当前生命值。
    pub(super) hp: f64,
    /// 当前护盾容量。
    pub(super) shield: f64,
    /// 护盾剩余普通回合计数。
    pub(super) shield_turns: u16,
    /// 当前能量。
    pub(super) energy: f64,
    /// 距下次普通行动的剩余距离。
    pub(super) remaining: f64,
    /// 普通 Turn 结束重排时应用的距离调整。
    pub(super) next_shift: f64,
    /// 单位当前韧性或按单位关联的初始韧性。
    pub(super) toughness: f64,
    /// 单位持久变量表或按单位关联的初始变量。
    pub(super) dynamic_values: std::collections::BTreeMap<String, f64>,
    /// 当前活动属性状态。
    pub(super) modifiers: ModifierBook,
    /// 本场剩余即时复活次数。
    pub(super) revivals_left: u8,
}
impl CombatUnit {
    /// 按所属类型的初始配置建立独立运行状态。
    pub(super) fn new(spec: &Unit, ally: bool, mechanics: &Mechanics) -> Self {
        Self {
            spec: spec.clone(),
            ally,
            hp: spec.initial_hp,
            shield: 0.0,
            shield_turns: 0,
            energy: spec.initial_energy,
            remaining: DISTANCE,
            next_shift: 0.0,
            toughness: mechanics.toughness.get(&spec.id).copied().unwrap_or(0.0),
            dynamic_values: mechanics
                .dynamic_values
                .get(&spec.id)
                .cloned()
                .unwrap_or_default(),
            modifiers: ModifierBook::default(),
            revivals_left: spec.revival.as_ref().map_or(0, |r| r.charges),
        }
    }
    /// 返回当前生命值是否大于零。
    pub(super) fn alive(&self) -> bool {
        self.hp > 0.0
    }
    /// 按 scale 取得攻击、防御或生命上限，供效果合成基础量。
    pub(super) fn scale(&self, scale: Scale) -> f64 {
        match scale {
            Scale::Attack => self.spec.stats.attack,
            Scale::Defense => self.spec.stats.defense,
            Scale::MaxHp => self.spec.stats.max_hp,
        }
    }
    /// 导出当前单位快照；观测和最终结果共用相同字段。
    pub(super) fn snapshot(
        &self,
        weaknesses: Option<crate::weakness::WeaknessState>,
    ) -> UnitResult {
        UnitResult {
            weaknesses,
            id: self.spec.id.clone(),
            ally: self.ally,
            hp: self.hp,
            shield: self.shield,
            energy: self.energy,
            alive: self.alive(),
            revivals_left: self.revivals_left,
            remaining_distance: self.remaining,
            speed: self.spec.stats.speed,
            toughness: self.toughness,
            dynamic_values: self.dynamic_values.clone(),
            modifiers: self.modifiers.states(),
        }
    }
}
#[derive(Clone)]
/// 完整动作结束后等待调度的插入请求。
pub(super) struct Pending {
    /// 本场执行者索引或回放中的执行者 ID。
    pub(super) actor: usize,
    /// 本次请求的调度方式。
    pub(super) kind: RequestKind,
    /// 可选动作标识或该执行者动作列表中的索引。
    pub(super) action: Option<usize>,
}
/// 一条监听规则在本场的剩余次数与寿命。
pub(super) struct Hook {
    /// 原始定义的本场副本。
    pub(super) spec: Trigger,
    /// 剩余触发次数，None 表示无限，Some(0) 表示停用。
    pub(super) remaining: Option<u32>,
    /// 剩余或初始寿命事件计数。
    pub(super) duration: u32,
}
/// 本场单位、资源、时钟、监听器和随机流的统一存储。
pub(super) struct State<'a> {
    /// 借用的组装后配置。
    pub(super) input: &'a Scenario,
    /// 已经入场的单位，死亡记录保留。
    pub(super) units: Vec<CombatUnit>,
    /// 本场时钟及各轮剩余额度。
    pub(super) timeline: Timeline,
    /// 按登记顺序排队的后续动作。
    pub(super) pending: std::collections::VecDeque<Pending>,
    /// 正在进行的普通 Turn 执行者。
    pub(super) turn: Option<usize>,
    /// 本场监听器的剩余次数和寿命。
    pub(super) hooks: Vec<Hook>,
    /// 嵌套事件的当前深度。
    pub(super) event_depth: u32,
    /// 嵌套效果的当前深度。
    pub(super) effect_depth: u32,
    /// 当前顶层效果调用链已执行的节点数。
    pub(super) effect_steps: u32,
    /// 当前完整动作的类别。
    pub(super) current_action_kind: Option<ActionKind>,
    /// 最终 Cycle 已结束且额度耗尽的标记。
    pub(super) exhausted: bool,
    /// Action 入口绑定的技能目标。
    pub(super) bound_target: Option<usize>,
    /// 临时重选作用域中的目标。
    pub(super) context_target: Option<usize>,
    /// 已命中目标列表所属的动作执行者。
    pub(super) attack_owner: Option<usize>,
    /// 当前动作实际命中过的不同单位。
    pub(super) attack_targets: Vec<usize>,
    /// 已执行的完整 Action 数。
    pub(super) actions: u32,
    /// 当前敌方波次的内部索引。
    pub(super) wave: usize,
    /// 有限队列下一位待入场敌人的位置。
    pub(super) queue_next: usize,
    /// 当前队列槽位到单位索引的映射。
    pub(super) queue_slots: Vec<Option<usize>>,
    /// 双方当前战技点，零索引为我方。
    pub(super) sp: [u8; 2],
    #[cfg(feature = "diagnostics")]
    /// 是否启用本场诊断回放。
    pub(super) trace: bool,
    #[cfg(feature = "diagnostics")]
    /// 按执行顺序保存的诊断回放。
    pub(super) events: Vec<crate::diagnostics::Event>,
    #[cfg(feature = "diagnostics")]
    /// 诊断回放是否达到条数上限。
    pub(super) trace_truncated: bool,
    /// 目标选择专用的独立随机流。
    pub(super) targets: ChaCha8Rng,
    /// 暴击专用随机流。
    pub(super) criticals: ChaCha8Rng,
    /// 元素选择专用随机流。
    pub(super) elements: ChaCha8Rng,
    /// 当前伤害的显式元素上下文。
    pub(super) damage_element: Option<Element>,
    /// 正在计算的伤害类别。
    pub(super) damage_attack_type: Option<AttackType>,
    /// 当前事件关联的命中类别。
    pub(super) event_attack_type: Option<AttackType>,
    /// 能力与任务模板临时变量帧栈。
    pub(super) dynamic_scopes: crate::dynamic_scope::ScopeFrames,
}

impl<'a> State<'a> {
    /// 按所属类型的初始配置建立独立运行状态。
    pub(super) fn new(
        input: &'a Scenario,
        seed: u64,
        #[cfg(feature = "diagnostics")] trace: bool,
    ) -> Self {
        let s = input;
        let mut units: Vec<_> = s
            .allies
            .iter()
            .map(|u| CombatUnit::new(u, true, &s.mechanics))
            .collect();
        let initial_count = s
            .mechanics
            .queue_capacity
            .unwrap_or(s.waves[0].len())
            .min(s.waves[0].len());
        units.extend(
            s.waves[0]
                .iter()
                .take(initial_count)
                .map(|u| CombatUnit::new(u, false, &s.mechanics)),
        );
        Self {
            input,
            units,
            timeline: Timeline::new(if s.mechanics.cycles.is_empty() {
                vec![s.max_av]
            } else {
                s.mechanics.cycles.clone()
            }),
            pending: Default::default(),
            turn: None,
            hooks: s
                .mechanics
                .triggers
                .iter()
                .map(|t| Hook {
                    spec: t.clone(),
                    remaining: if t.uses == 0 { None } else { Some(t.uses) },
                    duration: t.duration,
                })
                .collect(),
            event_depth: 0,
            effect_depth: 0,
            effect_steps: 0,
            current_action_kind: None,
            exhausted: false,
            bound_target: None,
            context_target: None,
            attack_owner: None,
            attack_targets: vec![],
            actions: 0,
            wave: 0,
            queue_next: initial_count,
            queue_slots: (0..s.mechanics.queue_capacity.unwrap_or(0))
                .map(|i| (i < initial_count).then_some(s.allies.len() + i))
                .collect(),
            sp: [s.ally_skill_points.initial, s.enemy_skill_points.initial],
            #[cfg(feature = "diagnostics")]
            trace,
            #[cfg(feature = "diagnostics")]
            events: Vec::new(),
            #[cfg(feature = "diagnostics")]
            trace_truncated: false,
            targets: stream(seed, b"targets"),
            criticals: stream(seed, b"criticals"),
            elements: stream(seed, b"elements"),
            damage_element: None,
            damage_attack_type: None,
            event_attack_type: None,
            dynamic_scopes: Default::default(),
        }
    }

    /// Reuse slots only after the previous complete Action. Revival has already resolved.
    pub(super) fn refill_queue(&mut self) {
        if self.input.mechanics.queue_capacity.is_none() {
            return;
        }
        let queue = &self.input.waves[0];
        for slot in 0..self.queue_slots.len() {
            if self.queue_slots[slot].is_some_and(|i| self.units[i].alive()) {
                continue;
            }
            self.queue_slots[slot] = None;
            if self.queue_next == queue.len() {
                continue;
            }
            let spec = &queue[self.queue_next];
            let unit = CombatUnit::new(spec, false, &self.input.mechanics);
            let index = self.units.len();
            self.units.push(unit);
            self.queue_slots[slot] = Some(index);
            self.queue_next += 1;
            trace!(
                self,
                "queue_spawn",
                index,
                None,
                None,
                Some(slot as f64),
                None
            );
        }
    }

    /// 返回执行者所属资源侧的索引，零为我方。
    pub(super) fn side(&self, actor: usize) -> usize {
        usize::from(!self.units[actor].ally)
    }

    /// 按资源侧索引取得共享战技点上限。
    pub(super) fn sp_max(&self, side: usize) -> u8 {
        if side == 0 {
            self.input.ally_skill_points.maximum
        } else {
            self.input.enemy_skill_points.maximum
        }
    }

    /// 根据当前战技点与执行者能量判断费用是否可支付。
    pub(super) fn affordable(&self, actor: usize, action: &Action) -> bool {
        action.sp_cost <= self.sp[self.side(actor)]
            && action.energy_cost <= self.units[actor].energy
    }
}
