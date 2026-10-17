//! MEM-02: layouts agree with an independent implementation (Python's `struct`, which follows the
//! platform C ABI for `@` and the byte-exact packed rules for `<`/`>`).
use std::{fs, process::Command};

const PROGRAM: &str = r#"import std.layout as layout;
import std.number as number;
import std.result as result;
fn dump(b: Bytes) -> String effects {} {
    var out = "";
    for i in 0..stdBytesLength(b) {
        match stdBytesGet(b, i) {
            Ok(v) => { out = out + result.valueOr(number.formatInt(v, 10), "?") + " "; },
            Err(_) => {}
        }
    }
    return out;
}
fn one(v: Int) -> List<Int> effects {} { let l = List<Int>(); l.add(v); return l; }
fn show(endian: layout.Endian, packed: Bool) effects { output } {
    var b = layout.builder(endian, packed);
    let _1 = layout.add(&mut b, "a", layout.Kind::I8, 1);
    let _2 = layout.add(&mut b, "b", layout.Kind::U16, 1);
    let _3 = layout.add(&mut b, "c", layout.Kind::I32, 1);
    let _4 = layout.add(&mut b, "d", layout.Kind::I64, 1);
    let _5 = layout.add(&mut b, "e", layout.Kind::U8, 3);
    let l = layout.finish(move b);
    let values = Map<String, List<Int>>();
    values.set("a", one(-5)); values.set("b", one(51966)); values.set("c", one(-123456789));
    values.set("d", one(1234567890123456789));
    let e = List<Int>(); e.add(1); e.add(2); e.add(255); values.set("e", e);
    match layout.pack(&l, &values) {
        Ok(bytes) => { Out.println(result.valueOr(number.formatInt(layout.size(&l), 10), "?")); Out.println(dump(bytes)); },
        Err(e) => { Out.println(e.code); }
    }
}
fn main() -> Int effects { output } {
    show(layout.Endian::Little, false);
    show(layout.Endian::Little, true);
    show(layout.Endian::Big, true);
    return 0;
}
"#;

#[test]
fn sizes_offsets_and_bytes_match_python_struct() {
    let dir = std::env::temp_dir().join(format!("rewind-layout-native-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    fs::write(
        dir.join("main.rw"),
        PROGRAM.replace(
            "number.formatInt(layout.size(&l), 10).unwrapOr(\"?\")",
            "result.valueOr(number.formatInt(layout.size(&l), 10), \"?\")",
        ),
    )
    .unwrap();
    let ran = Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(&dir)
        .args(["run", "main.rw", "--allow-effects", "output"])
        .output()
        .unwrap();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let ours = String::from_utf8_lossy(&ran.stdout).into_owned();
    let script = r#"
import struct
v = (-5, 51966, -123456789, 1234567890123456789, 1, 2, 255)
for fmt in ('@bHiq3B0q', '<bHiq3B', '>bHiq3B'):
    data = struct.pack(fmt, *v)
    print(struct.calcsize(fmt))
    print(''.join(str(b) + ' ' for b in data))
"#;
    let theirs = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args(["-c", script])
        .output()
        .unwrap();
    assert!(theirs.status.success());
    let theirs = String::from_utf8_lossy(&theirs.stdout).replace("\r\n", "\n");
    assert_eq!(ours.replace("\r\n", "\n"), theirs);
    fs::remove_dir_all(&dir).ok();
}
