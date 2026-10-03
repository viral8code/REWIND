use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v193-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
#[test]
fn sequential_tasks_channels_and_groups_do_not_hit_the_lifetime_object_limit() {
    let root = root();
    fs::write(root.join("main.rw"),r#"
async fn work()->Int effects {} {return 7;}
for i in 0..5000 {let task=work();assert_eq(await task,Ok(7));}
for i in 0..1500 {{let c=Channel<Int>(1);assert_eq(await c.send(7),Ok(()));assert_eq(await c.receive(),Ok(7));c.close();}}
for i in 0..100 {{using group=TaskGroup();group.add(work());}}
Out.println(1);publish;
"#).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rw",
            "--steps",
            "5000000",
            "--task-steps",
            "5000000",
            "--native-work",
            "5000000",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn queued_handles_cleanup_groups_and_checkpoints_survive_collection_and_replay() {
    let root = root();
    fs::write(root.join("main.rw"),r#"
async fn work()->Int effects {} {return 7;}
{let channel=Channel<Task<Int>>(1);assert_eq(await channel.send(work()),Ok(()));commit queued;
for i in 0..100 {assert_eq(await work(),Ok(7));}
match await channel.receive(){Ok(task)=>{assert_eq(await task,Ok(7));},Err(_)=>{panic("queue lost its task");}}
revert queued;
for i in 0..100 {assert_eq(await work(),Ok(7));}
match await channel.receive(){Ok(task)=>{assert_eq(await task,Ok(7));},Err(_)=>{panic("checkpoint lost its task");}}
channel.close();drop queued;}
{using group=TaskGroup();group.add(work());for i in 0..100 {assert_eq(await work(),Ok(7));}}
for i in 0..100 {assert_eq(await work(),Ok(7));}
Out.println(1);publish;
"#).unwrap();
    ok(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let out = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--record",
            "trace.json",
            "--native-work",
            "2000000",
        ],
    );
    ok(&out);
    assert_eq!(out.stdout, b"1\n");
    let trace: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("trace.json")).unwrap()).unwrap();
    assert!(
        trace["debug"]["final"]["scheduler"]["tasks"]
            .as_object()
            .unwrap()
            .len()
            < 100
    );
    assert!(trace["debug"]["final"]["scheduler"]["channels"]
        .as_object()
        .unwrap()
        .is_empty());
    let replay = call(&root, &["replay", "trace.json"]);
    ok(&replay);
    assert_eq!(out.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn unobserved_failure_is_not_hidden_by_scheduler_collection() {
    let root = root();
    fs::write(
        root.join("main.rw"),
        r#"
async fn fail()->Unit effects {} {panic("unobserved failure retained");}
async fn work()->Int effects {} {return 7;}
{let bad=spawn fail();for i in 0..100 {assert_eq(await work(),Ok(7));}}
for i in 0..100 {assert_eq(await work(),Ok(7));}
publish;
"#,
    )
    .unwrap();
    let out = call(&root, &["run", "main.rw"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unobserved failure retained"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
