//! 仅由 diagnostics 功能启用的开发场景命令行入口。
use battle_core::{
    batch::{run_batch, BatchRequest},
    diagnostics::{simulate, ScenarioFile},
    engine::RNG_ALGORITHM,
};
use serde_json::{json, Value};
use std::{
    env, fs,
    io::{self, Read, Write},
};

/// 读取文件和命令参数，返回开发报告或明确的解析错误。
fn execute() -> Result<Value, String> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        return Err("usage: hcwa-battle inspect SCENARIO.json | simulate SCENARIO.json SEED | batch SCENARIO.json TRIALS SEED".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&args[2])
        .map_err(|e| format!("scenario: {e}"))?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("scenario: {e}"))?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("scenario exceeds 16 MiB input limit".into());
    }
    let file = ScenarioFile::from_slice(&bytes).map_err(|e| format!("scenario schema: {e}"))?;
    let input = &file.battle;
    match args[1].as_str() {
        "inspect" if args.len() == 3 => Ok(json!({"status":"parsed",
            "scenario_sha256":file.fingerprint,"provenance":file.provenance,"rng":RNG_ALGORITHM})),
        "simulate" if args.len() == 4 => {
            let seed = args[3]
                .parse()
                .map_err(|_| "seed must be an unsigned integer")?;
            let result = simulate(input, seed, true)?;
            serde_json::to_value(file.report(result, seed)).map_err(|e| e.to_string())
        }
        "batch" if args.len() == 5 => {
            let request = BatchRequest {
                trials: args[3]
                    .parse()
                    .map_err(|_| "trials must be an unsigned integer")?,
                seed: args[4]
                    .parse()
                    .map_err(|_| "seed must be an unsigned integer")?,
            };
            serde_json::to_value(run_batch(input, &request, &mut || false)?)
                .map_err(|e| e.to_string())
        }
        _ => Err("invalid command or argument count".into()),
    }
}
/// 输出 JSON 报告，文件或执行失败时以退出码 2 结束。
fn main() {
    match execute() {
        Ok(result) => {
            let mut stdout = io::BufWriter::new(io::stdout().lock());
            let written = serde_json::to_writer_pretty(&mut stdout, &result)
                .map_err(io::Error::other)
                .and_then(|_| writeln!(stdout))
                .and_then(|_| stdout.flush());
            if let Err(error) = written {
                eprintln!(
                    "{}",
                    json!({"status":"error","error":format!("output: {error}")})
                );
                std::process::exit(2);
            }
        }
        Err(error) => {
            eprintln!("{}", json!({"status":"error","error":error}));
            std::process::exit(2);
        }
    }
}
