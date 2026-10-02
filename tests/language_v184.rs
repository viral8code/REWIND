use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-v184-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn datetime_source_free_replay_and_dst_map_key() {
    let root = dir();
    fs::write(
        root.join("main.rw"),
        include_str!("../examples/datetime/main.rw"),
    )
    .unwrap();
    success(&call(&root, &["compile", "main.rw"]));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(&root, &["run", "main.rwc", "--record", "trace.json"]);
    success(&output);
    assert_eq!(output.stdout,b"3600\n2024-11-03T06:30:00.123456789Z\n2024-11-03T05:30:00.123456789Z\n32400\nsame instant\n");
    let replay = call(&root, &["replay", "trace.json"]);
    success(&replay);
    assert_eq!(replay.stdout, output.stdout);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn datetime_freeze_tasks_duration_and_json() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
import std.datetime as dt;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
let value=take(dt.parse("1969-12-31T23:59:59.999999999Z"));let saved=freeze(value);let owned=thaw(saved);
async fn read(value:Instant)->String effects {} {return take(dt.format(&value));}
let job=spawn read(move owned);Out.println(take(await job));
let one=take(dt.duration(0,1));var changed=take(dt.add(&value,&one));Out.println(take(dt.seconds(&changed)));
commit mark;changed=take(dt.parse("2024-01-01T00:00:00Z"));revert mark;Out.println(take(dt.seconds(&changed)));
let encoded=take(dt.toJson(&value));let decoded=take(dt.fromJson(move encoded));Out.println(value==decoded);Out.println(value.hash()==decoded.hash());
let negative=take(dt.duration(0,-1));let positive=take(dt.negateDuration(&negative));Out.println(negative<positive);let durationJson=take(dt.durationToJson(&negative));let durationCopy=take(dt.durationFromJson(move durationJson));Out.println(durationCopy==negative);let map=Map<Duration,Int>();map.set(negative,7);let key=take(dt.duration(-1,999999999));match map.get(key){Some(item)=>{Out.println(item);},None=>{panic("missing duration");}}publish;
"#).unwrap();
    let output = call(&root, &["run", "main.rw"]);
    success(&output);
    assert_eq!(
        output.stdout,
        b"1969-12-31T23:59:59.999999999Z\n0\n0\ntrue\ntrue\ntrue\ntrue\n7\n"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn recorded_clock_revert_reuses_one_atomic_observation() {
    let root = dir();
    fs::write(root.join("main.rw"),r#"
effects {external,clock};import std.clock as clock;import std.datetime as dt;
fn take<T,E>(value:Result<T,E>)->T effects {} {match move value{Ok(item)=>{return move item;},Err(_)=>{panic("failed");}}}
var first=take(dt.instant(0,0));commit before;external {first=take(clock.now());}Out.println(take(dt.format(&first)));publish;revert before;external {first=take(clock.now());}Out.println(take(dt.format(&first)));publish;
"#).unwrap();
    success(&call(
        &root,
        &["compile", "main.rw", "--allow-effects", "external,clock"],
    ));
    fs::remove_file(root.join("main.rw")).unwrap();
    let output = call(
        &root,
        &[
            "run",
            "main.rwc",
            "--allow-effects",
            "external,clock",
            "--record",
            "trace.json",
        ],
    );
    success(&output);
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    let replay = call(
        &root,
        &["replay", "trace.json", "--allow-effects", "external,clock"],
    );
    success(&replay);
    assert_eq!(output.stdout, replay.stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn datetime_cannot_be_forged_or_read_clock_from_a_pure_function() {
    for source in [
        "let forged=Instant();",
        "let forged=Duration();",
        "record Instant {}",
        "fn stdDateTimeParse(value:String)->Int effects {} {return 0;}",
        "import std.clock as clock;fn hidden()->Result<Instant,StdError> effects {} {return clock.now();}",
    ] {
        let root=dir();fs::write(root.join("main.rw"),source).unwrap();
        assert!(!call(&root,&["compile","main.rw"]).status.success());
        fs::remove_dir_all(root).unwrap();
    }
}
