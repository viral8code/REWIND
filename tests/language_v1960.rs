use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1960-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
    fs::write(r.join("main.rw"), source).unwrap();
    r
}
fn call(r: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .env_remove("DISPLAY")
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
#[test]
fn native_dialog_responds_and_reuses_recorded_result_with_disconnected_replay() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let r = std::env::temp_dir().join(format!("rewind-native-dialog-sdk-{}", std::process::id()));
    let result = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args([
            repo.join("scripts/smoke-gui-dialog-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            r.as_os_str(),
            repo.join("examples/gui-dialog/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn additive_dependency_contracts_are_compatible_but_removals_and_signature_changes_are_not() {
    let r = root("Out.println(1);publish;");
    let contract = serde_json::json!({"async":false,"effects":[],"generics":[],"parameters":["Int"],"return":"Int"});
    let old = serde_json::json!({"format":2,"kind":"rewind-api","language":"1.9.59","symbols":{"dependency:library.rw":{"fn:$import$library$existing":contract.clone()}}});
    let mut new = old.clone();
    new["symbols"]["dependency:library.rw"]["fn:$import$library$added"] = contract;
    fs::write(r.join("old.json"), serde_json::to_vec(&old).unwrap()).unwrap();
    fs::write(r.join("new.json"), serde_json::to_vec(&new).unwrap()).unwrap();
    let accepted = call(&r, &["api-diff", "old.json", "new.json", "--deny-breaking"]);
    ok(&accepted);
    let output: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(output["compatible"], true);
    assert_eq!(output["changes"][0]["kind"], "dependencyExtended");
    for changed in ["return", "effects", "remove"] {
        let mut bad = new.clone();
        if changed == "return" {
            bad["symbols"]["dependency:library.rw"]["fn:$import$library$existing"]["return"] =
                serde_json::json!("String");
        } else if changed == "effects" {
            bad["symbols"]["dependency:library.rw"]["fn:$import$library$existing"]["effects"] =
                serde_json::json!(["gui"]);
        } else {
            bad["symbols"]["dependency:library.rw"]
                .as_object_mut()
                .unwrap()
                .remove("fn:$import$library$existing");
        }
        fs::write(r.join("new.json"), serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(
            !call(&r, &["api-diff", "old.json", "new.json", "--deny-breaking"])
                .status
                .success()
        );
    }
    fs::remove_dir_all(r).unwrap();
}
