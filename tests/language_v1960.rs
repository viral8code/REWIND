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
const SOURCE: &str = r#"import std.guiDialog as dialog;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(v)=>{return move v;},Err(_)=>{panic("task failed");}}}
fn future(value:Option<Task<Result<Option<String>,StdError>>>)->Task<Result<Option<String>,StdError>> effects {} {match value{Some(v)=>{return v;},None=>{panic("no task");}}}
var pending:Option<Task<Result<Option<String>,StdError>>>=None;
commit before;
external{pending=Some(dialog.openFile("","ignored"));}
assert_eq(take(await future(pending)),Err(StdError("GuiDialogTitle",0)));
revert before;external{pending=Some(dialog.saveFile("Valid","bad\0path"));}
assert_eq(take(await future(pending)),Err(StdError("GuiDialogPath",0)));
drop before;Out.println("bounded");publish;"#;
#[test]
fn dialog_argument_errors_do_not_contact_host_and_both_trace_modes_are_source_free() {
    let r = root(SOURCE);
    ok(&call(
        &r,
        &["compile", "main.rw", "--allow-effects", "gui,external"],
    ));
    fs::remove_file(r.join("main.rw")).unwrap();
    fs::remove_dir_all(r.join(".rewind")).unwrap();
    for mode in ["debug", "compact"] {
        let run = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--allow-effects",
                "gui,external",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&run);
        assert_eq!(run.stdout, b"bounded\n");
        let replay = call(
            &r,
            &["replay", "trace.json", "--allow-effects", "gui,external"],
        );
        ok(&replay);
        assert_eq!(run.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn dialog_requires_declared_permission_external_region_application_task_and_version() {
    let r = root("import std.guiDialog as dialog;let pending=dialog.openFile(\"Open\",\"\");");
    let denied = call(&r, &["check", "main.rw", "--allow-effects", "gui,external"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("ExternalBoundary"));
    fs::write(r.join("main.rw"), SOURCE).unwrap();
    assert!(!call(&r, &["run", "main.rw"]).status.success());
    fs::write(r.join("main.rw"),"import std.guiDialog as dialog;async fn forbidden()->Unit effects {gui,external,tasks}{external{let pending=dialog.openFile(\"Open\",\"\");}}").unwrap();
    let denied = call(&r, &["check", "main.rw", "--allow-effects", "gui,external"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("GuiMainTaskOnly"));
    fs::write(
        r.join("main.rw"),
        "external{let pending=stdExternalGuiDialog(false,\"Open\",\"\");}",
    )
    .unwrap();
    fs::write(r.join("rewind.toml"),"language = \"1.9.59\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"gui,external,tasks\"\n").unwrap();
    ok(&call(&r, &["update", "--root", "."]));
    let denied = call(&r, &["check", "main.rw", "--allow-effects", "gui,external"]);
    assert!(!denied.status.success());
    assert!(
        String::from_utf8_lossy(&denied.stderr).contains("1.9.60"),
        "{}",
        String::from_utf8_lossy(&denied.stderr)
    );
    fs::remove_dir_all(r).unwrap();
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
