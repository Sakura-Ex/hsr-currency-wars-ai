//! 演示外部选招与引擎执行分离；choose可替换为模型推理或训练策略。
use battle_core::{
    decision::{Decision, Operation},
    diagnostics::ScenarioFile,
    engine::{BattleSession, BattleStep},
};

/// 示例策略选择第一个合法操作；空插入窗口返回None。
fn choose(decision: &Decision) -> Option<Operation> {
    decision.legal.first().cloned()
}

/// 从开发文件建立分步会话，提交示例策略并输出结果。
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: step_battle SCENARIO.json SEED".into());
    }
    let bytes = std::fs::read(&args[1]).map_err(|e| e.to_string())?;
    let file = ScenarioFile::from_slice(&bytes)?;
    let input = file.battle;
    let seed = args[2]
        .parse()
        .map_err(|_| "seed must be an unsigned integer")?;
    let mut battle = BattleSession::with_trace(&input, seed, true)?;
    while let BattleStep::Decision(decision) = battle.advance()? {
        let (id, operation) = (decision.id, choose(decision));
        // 此处也可以等待外部进程或Python绑定返回操作，会话保留当前状态。
        battle.submit(id, operation)?;
    }
    let result = battle.into_result()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
    );
    Ok(())
}
