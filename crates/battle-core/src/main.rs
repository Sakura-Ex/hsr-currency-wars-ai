use hcwa_battle_core::{
    batch::{run_batch, BatchRequest},
    engine::{simulate, RNG_ALGORITHM},
    model::{validate, Scenario},
};
use serde_json::{json, Value};
use std::{
    env, fs,
    io::{self, Read, Write},
};

fn execute() -> Result<Value, String> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        return Err("usage: hcwa-battle validate SCENARIO.json | simulate SCENARIO.json SEED | batch SCENARIO.json TRIALS SEED".into());
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
    let raw: Scenario =
        serde_json::from_slice(&bytes).map_err(|e| format!("scenario schema: {e}"))?;
    let input = validate(raw)?;
    match args[1].as_str() {
        "validate" if args.len() == 3 => {
            Ok(json!({"status":"valid","ruleset":input.scenario().ruleset,
            "scenario_sha256":input.fingerprint(),"provenance":input.scenario().provenance,
            "rng":RNG_ALGORITHM,"scope":"explicit_basic_profile_not_full_game_or_run_settlement"}))
        }
        "simulate" if args.len() == 4 => serde_json::to_value(simulate(
            &input,
            args[3]
                .parse()
                .map_err(|_| "seed must be an unsigned integer")?,
            true,
        )?)
        .map_err(|e| e.to_string()),
        "batch" if args.len() == 5 => {
            let request = BatchRequest {
                trials: args[3]
                    .parse()
                    .map_err(|_| "trials must be an unsigned integer")?,
                seed: args[4]
                    .parse()
                    .map_err(|_| "seed must be an unsigned integer")?,
            };
            serde_json::to_value(run_batch(&input, &request, &mut || false)?)
                .map_err(|e| e.to_string())
        }
        _ => Err("invalid command or argument count".into()),
    }
}
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
