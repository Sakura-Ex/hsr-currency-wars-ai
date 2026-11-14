//! Exercise the public executable rather than a substitute for its entry point.
use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Command};
fn example(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}
fn invoke(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_hcwa-battle"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn executable_validates_and_replays_full_width_seeds() {
    let file = example("duel.json");
    let valid = invoke(&["validate", &file]);
    assert!(valid.status.success());
    let v: Value = serde_json::from_slice(&valid.stdout).unwrap();
    assert_eq!(v["status"], "valid");
    let a = invoke(&["simulate", &file, "18446744073709551615"]);
    let b = invoke(&["simulate", &file, "18446744073709551615"]);
    assert!(a.status.success());
    assert_eq!(a.stdout, b.stdout);
    let r: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(r["seed"], "18446744073709551615");
    assert_eq!(r["scenario_sha256"], v["scenario_sha256"]);
    assert!(r["run_hp_loss"].is_null());
}
#[test]
fn executable_evaluates_public_scenarios_and_batch_denominators() {
    for name in ["team-waves.json", "revival-budget.json"] {
        let output = invoke(&["simulate", &example(name), "17"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(result["actions"].as_u64().unwrap() > 0);
    }
    let output = invoke(&["batch", &example("duel.json"), "64", "7"]);
    assert!(output.status.success());
    let r: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(r["completed"], 64);
    assert_eq!(r["probability_denominator"], 64);
    assert_eq!(r["master_seed"], "7");
    assert_eq!(r["ledger"].as_array().unwrap().len(), 64);
    assert_eq!(r["run_hp_loss_unknown_count"], 64);
}
#[test]
fn executable_rejects_unsupported_and_malformed_input_without_results() {
    let mut value: Value =
        serde_json::from_str(&fs::read_to_string(example("duel.json")).unwrap()).unwrap();
    value["unsupported_mechanics"] = json!(["background_follow_up"]);
    let dir = std::env::temp_dir().join(format!("hcwa-battle-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("unsupported-{}.json", std::process::id()));
    fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    let output = invoke(&["simulate", file.to_str().unwrap(), "1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("unsupported mechanics"));
    value["unsupported_mechanics"] = json!([]);
    value["max_av"] = json!("500");
    fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    let output = invoke(&["validate", file.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(error["error"].as_str().unwrap().contains("scenario schema"));
    let output = invoke(&["batch", &example("duel.json"), "0", "1"]);
    assert_eq!(output.status.code(), Some(2));
    fs::remove_file(file).unwrap();
    fs::remove_dir(dir).unwrap();
}
