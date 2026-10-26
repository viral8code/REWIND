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
            "rewind-frozen-bounds-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // A directory left by an earlier process with the same id must not break this run.
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.rw"), source).unwrap();
        Self(root)
    }

    fn call(&self, command: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rewind"))
            .current_dir(&self.0)
            .args([command, "main.rw"])
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn send_parameters_can_be_shared_under_frozen_roots_and_records() {
    let fixture = Fixture::new(
        r#"
record Saved<T:Send> {value:Frozen<T>}
fn share<T:Share>(value:T)->T effects {} {return value;}
fn root<T:Send>(value:Frozen<T>)->Frozen<T> effects {} {
    let first=share(value);
    return share(value);
}
fn wrapped<T:Send>(value:Saved<T>)->Saved<T> effects {} {
    let first=share(value);
    return share(value);
}
fn main() -> Int {
    let values=List<Int>();
    values.add(10);
    let frozen=root(freeze(values));
    let saved=wrapped(Saved<List<Int>>(frozen));
    values.set(0,99);
    assert_eq(frozen.get(0),10);
    assert_eq(saved.value.get(0),10);
    let copy=thaw(saved.value);
    copy.set(0,7);
    assert_eq(frozen.get(0),10);
    return 0;
}
"#,
    );
    let output = fixture.call("run");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn send_bound_does_not_allow_sharing_unfrozen_values() {
    for source in [
        "fn share<T:Share>(value:T)->T {return value;} fn bad<T:Send>(value:T)->T {return share(value);}",
        "record Box<T:Send> {value:T} fn share<T:Share>(value:T)->T {return value;} fn bad<T:Send>(value:Box<T>)->Box<T> {return share(value);}",
        "fn share<T:Share>(value:T)->T {return value;} fn bad<T:Send>(value:List<T>)->List<T> {return share(value);}",
        "fn share<T:Share>(value:T)->T {return value;} fn bad<T>(value:Frozen<T>)->Frozen<T> {return share(value);}",
        "record Box<T:Send> {value:T} let data=List<Int>();let bad=Box<List<Int>>(move data);",
    ] {
        let fixture = Fixture::new(source);
        let output = fixture.call("check");
        assert!(
            !output.status.success(),
            "unjustified Share accepted: {source}"
        );
    }
}

#[test]
fn frozen_does_not_make_non_send_resources_shareable() {
    let fixture = Fixture::new(
        "fn share<T:Share>(value:T)->T {return value;} fn bad(value:Frozen<FileHandle>)->Frozen<FileHandle> {return share(value);}",
    );
    let output = fixture.call("check");
    assert!(!output.status.success(), "FileHandle became shareable");
}

#[test]
fn thaw_removes_only_the_outer_layer_and_preserves_nested_frozen_values() {
    let fixture = Fixture::new(
        r#"
record Box<T:Share> {value:T}
fn main() -> Int {
    let data=List<Int>();
    data.add(10);
    let first=freeze(data);
    let second=freeze(first);
    let stillFrozen=thaw(second);
    assert_eq(stillFrozen.get(0),10);
    let editable=thaw(stillFrozen);
    editable.set(0,7);
    assert_eq(first.get(0),10);
    let wrapped=Box<Frozen<List<Int>>>(first);
    let copy=thaw(freeze(wrapped));
    let member=copy.value;
    assert_eq(member.get(0),10);
    let independent=thaw(member);
    independent.set(0,9);
    assert_eq(first.get(0),10);
    let values=List<Frozen<List<Int>>>();
    values.add(first);
    let restored=thaw(freeze(values));
    let item=restored.get(0);
    assert_eq(thaw(item).get(0),10);
    for item in freeze(values) {assert_eq(item.get(0),10);}
    return 0;
}
"#,
    );
    let output = fixture.call("run");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
