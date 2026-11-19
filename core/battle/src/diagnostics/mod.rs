//! 开发工具专用的场景来源、输入指纹、回放与临时结算。
use crate::model::{AttackType, BattleResult, Scenario};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize)]
/// 诊断构建中按顺序保存的观察记录。
pub struct Event {
    /// 从零开始的场次或回放序号。
    pub index: u32,
    /// 该记录发生时的 AV。
    pub av: f64,
    /// 观察记录种类，例如 damage、heal、death。
    pub kind: String,
    /// 本场执行者索引或回放中的执行者 ID。
    pub actor: String,
    /// 效果目标选择器或记录中的目标 ID。
    pub target: Option<String>,
    /// 可选动作标识或该执行者动作列表中的索引。
    pub action: Option<String>,
    /// 按所属效果解释的数值或变化量。
    pub amount: Option<f64>,
    /// 该伤害是否暴击。
    pub critical: Option<bool>,
    /// 本次命中类别。
    pub attack_type: Option<AttackType>,
}

/// 开发场景的资料来源，独立于正式战斗输入。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Provenance {
    /// 游戏资料版本。
    pub game_version: String,
    /// 数据快照标识。
    pub data_snapshot: String,
    /// 人工场景的用途说明。
    pub description: String,
}

/// 开发工具读取的场景及其附加信息。
#[derive(Clone, Debug)]
pub struct ScenarioFile {
    /// 局面组装完成后供引擎直接运行的配置。
    pub battle: Scenario,
    /// 场景的资料来源。
    pub provenance: Provenance,
    /// 人工近似与假设。
    pub assumptions: Vec<String>,
    /// 原始场景 JSON 的 SHA-256。
    pub fingerprint: String,
    /// 开发示例使用的临时整局结算。
    pub settlement: Option<settlement::SettlementPolicy>,
}

impl ScenarioFile {
    /// 解析开发 JSON，拆出附加信息并构造战斗配置。
    ///
    /// 参数 bytes 为完整 UTF-8 JSON。此入口处理文件结构；静态规则与引用
    /// 由资料编译和组装方保证。文件解析失败时返回具体错误。
    pub fn from_slice(bytes: &[u8]) -> Result<Self, String> {
        let mut value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&value).map_err(|e| e.to_string())?)
        );
        let object = value
            .as_object_mut()
            .ok_or("scenario file must be an object")?;
        let provenance = object
            .remove("provenance")
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        object.remove("schema_version");
        object.remove("ruleset");
        object.remove("unsupported_mechanics");
        let mut assumptions = vec![];
        let mut settlement = None;
        if let Some(mechanics) = object.get_mut("mechanics").and_then(|v| v.as_object_mut()) {
            assumptions = mechanics
                .remove("assumptions")
                .map(serde_json::from_value)
                .transpose()
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            settlement = mechanics
                .remove("settlement")
                .map(serde_json::from_value)
                .transpose()
                .map_err(|e| e.to_string())?;
        }
        let battle = serde_json::from_value(value).map_err(|e| e.to_string())?;
        Ok(Self {
            battle,
            provenance,
            assumptions,
            fingerprint,
            settlement,
        })
    }

    /// 将战斗结果与来源合并为报告，并在工具侧应用可选临时结算。
    pub fn report(&self, result: BattleResult, seed: u64) -> BattleReport {
        let settlement = self.settlement.as_ref().map(|policy| {
            let defeated = result
                .units
                .iter()
                .filter(|u| !u.ally && !u.alive)
                .map(|u| u.id.clone())
                .collect();
            settlement::settle(policy, defeated, result.outcome)
        });
        BattleReport {
            result,
            seed,
            provenance: self.provenance.clone(),
            scenario_sha256: self.fingerprint.clone(),
            assumptions: self.assumptions.clone(),
            settlement,
        }
    }
}

/// 开发报告，正式构建排除整个类型。
#[derive(Clone, Debug, Serialize)]
pub struct BattleReport {
    /// 引擎返回的本场结果。
    #[serde(flatten)]
    pub result: BattleResult,
    /// 模拟器种子，JSON 保留完整 64 位。
    #[serde(serialize_with = "seed_decimal::serialize")]
    pub seed: u64,
    /// 场景来源。
    pub provenance: Provenance,
    /// 原始输入指纹。
    pub scenario_sha256: String,
    /// 人工场景采用的近似假设。
    pub assumptions: Vec<String>,
    /// 工具侧执行的临时结算。
    pub settlement: Option<settlement::SettlementResult>,
}

pub mod settlement;

/// 以默认策略运行开发场景，trace 决定是否记录逐事件回放。
pub fn simulate(input: &Scenario, seed: u64, trace: bool) -> Result<BattleResult, String> {
    simulate_cancellable(input, seed, trace, &mut || false)
}

/// 运行可取消的开发场景，保持正式执行的调度与取消时点。
pub fn simulate_cancellable(
    input: &Scenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BattleResult, String> {
    simulate_with_policy(input, seed, trace, cancel, &mut crate::decision::Automatic)
}

/// 使用外部策略运行开发场景，并按 trace 选项收集回放。
pub fn simulate_with_policy(
    input: &Scenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
    policy: &mut impl crate::decision::Policy,
) -> Result<BattleResult, String> {
    crate::engine::drive(
        crate::engine::BattleSession::with_trace(input, seed, trace)?,
        cancel,
        policy,
    )
}
/// JSON transports full-width seeds as strings so TypeScript cannot round them.
pub mod seed_decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    /// 将完整 u64 随机种子输出为十进制字符串。
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    /// 从纯十进制字符串读取完整 u64，格式或范围错误时返回解析错误。
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
