//! 通过真实开发入口核对来源、种子和解析结果。
use serde_json::Value;
use std::{path::PathBuf, process::Command};

/// 返回仓库内开发场景的绝对路径。
fn example(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// 调用由 diagnostics 功能构建的实际命令行程序。
fn invoke(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_hcwa-battle"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn development_cli_preserves_full_width_seeds_and_reports() {
    let file = example("duel.json");
    let parsed = invoke(&["inspect", &file]);
    assert!(parsed.status.success());
    let source: Value = serde_json::from_slice(&parsed.stdout).unwrap();
    let a = invoke(&["simulate", &file, "18446744073709551615"]);
    let b = invoke(&["simulate", &file, "18446744073709551615"]);
    assert!(a.status.success());
    assert_eq!(a.stdout, b.stdout);
    let result: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(result["seed"], "18446744073709551615");
    assert_eq!(result["scenario_sha256"], source["scenario_sha256"]);
}

#[test]
fn development_cli_reuses_batch_statistics() {
    let output = invoke(&["batch", &example("duel.json"), "64", "7"]);
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["completed"], 64);
    assert_eq!(result["ledger"].as_array().unwrap().len(), 64);
    assert_eq!(
        invoke(&["batch", &example("duel.json"), "0", "7"])
            .status
            .code(),
        Some(2)
    );
}
