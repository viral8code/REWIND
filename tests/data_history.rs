use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "rewind-data-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.rw"), source).unwrap();
        Self(root)
    }

    fn call(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn snapshots_outlive_the_capture_frame_and_work_in_source_free_replay() {
    let fixture = Fixture::new(
        r#"
import std.snapshot as snapshots;
import std.snapshot.{Snapshot};
import std.history as history;
import std.history.{create as createHistory};
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(v)=>{return move v;},Err(_)=>{panic("setup");}}
}
fn initial()->Snapshot<List<Int>> effects {} {
    let data=List<Int>();data.add(10);
    return snapshots.capture(&data);
}
let saved=initial();
let model=snapshots.fork(saved);
let versions=take(createHistory<List<Int>>(2));
assert_eq(history.save(&mut versions,"initial",&model),Ok(()));
model.set(0,99);
assert_eq(history.save(&mut versions,"latest",&model),Ok(()));
history.clear(&mut versions);
assert_eq(history.length(&versions),0);
Out.println(snapshots.read(saved).get(0));
Out.println(model.get(0));
let fork=snapshots.fork(saved);fork.set(0,7);
Out.println(fork.get(0));
assert_eq(snapshots.read(saved).get(0),10);
publish;
"#,
    );
    success(&fixture.call(&["compile", "main.rw"]));
    fs::remove_file(fixture.0.join("main.rw")).unwrap();
    fs::remove_dir_all(fixture.0.join(".rewind")).unwrap();
    let run = fixture.call(&[
        "run",
        "main.rwc",
        "--record",
        "record.json",
        "--record-mode",
        "compact",
    ]);
    success(&run);
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n"),
        "10\n99\n7\n"
    );
    let replay = fixture.call(&["replay", "record.json"]);
    success(&replay);
    assert_eq!(run.stdout, replay.stdout);
}

#[test]
fn history_is_ordinary_model_state_and_revert_restores_its_registry() {
    let fixture = Fixture::new(
        r#"
import std.snapshot as snapshots;
import std.history as history;
import std.history.{create as createHistory};
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(v)=>{return move v;},Err(_)=>{panic("setup");}}
}
let versions=take(createHistory<List<Int>>(4));
let data=List<Int>();data.add(10);
assert_eq(history.save(&mut versions,"initial",&data),Ok(()));
commit saved;
data.set(0,99);
assert_eq(history.save(&mut versions,"later",&data),Ok(()));
assert_eq(history.length(&versions),2);
history.clear(&mut versions);
assert_eq(history.length(&versions),0);
assert_eq(history.save(&mut versions,"replacement",&data),Ok(()));
revert saved;
assert_eq(history.length(&versions),1);
assert(history.contains(&versions,"initial"));
assert(!history.contains(&versions,"later"));
assert(!history.contains(&versions,"replacement"));
assert_eq(data.get(0),10);
match history.get(&versions,"initial") {
    Some(first)=>{
        let edited=snapshots.fork(first);edited.set(0,7);
        assert_eq(snapshots.read(first).get(0),10);
    },
    None=>{panic("lost initial snapshot");}
}
drop saved;
"#,
    );
    success(&fixture.call(&["run", "main.rw"]));
}

#[test]
fn snapshot_capture_is_deep_for_nested_mutable_values() {
    let fixture = Fixture::new(
        r#"
import std.snapshot as snapshots;
struct Model {private data:List<Int>}
fn model()->Model effects {} {
    let values=List<Int>();values.add(10);
    return Model(move values);
}
let current=model();
let saved=snapshots.capture(&current);
current.data.set(0,99);
let left=snapshots.fork(saved);
let right=snapshots.fork(saved);
left.data.set(0,7);
assert_eq(current.data.get(0),99);
assert_eq(left.data.get(0),7);
assert_eq(right.data.get(0),10);
"#,
    );
    success(&fixture.call(&["run", "main.rw"]));
}

#[test]
fn snapshot_does_not_expose_mutation_or_private_construction() {
    for (source, message) in [
        (
            "import std.snapshot as s;let xs=List<Int>();xs.add(1);let saved=s.capture(&xs);let view=s.read(saved);view.add(2);",
            "cannot mutate Frozen",
        ),
        (
            "import std.snapshot.{Snapshot};let saved=Snapshot<Int>(freeze(1));",
            "private",
        ),
        (
            "import std.snapshot as s;let value=1;let saved=s.capture(&value);let raw=saved.value;",
            "private",
        ),
        (
            "import std.snapshot as s;fn save(file:&FileHandle)->Unit effects {} {s.capture(file);}",
            "Send",
        ),
    ] {
        let fixture = Fixture::new(source);
        let output = fixture.call(&["check", "main.rw"]);
        assert!(!output.status.success(), "unexpectedly accepted: {source}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
