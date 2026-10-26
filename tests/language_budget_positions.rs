// enforce_budget を速くする前に、`--history-memory` ごとの実行結果（終了コード、stderr の先頭、stdout）を固定する。
use std::{
    fs,
    path::Path,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// 文字列を伸ばし、途中で出力しながら数を足す。
const STRING_PROGRAM: &str = r#"
fn main() -> Int {
    var s = "";
    var total = 0;
    for i in 0..200 {
        s += "ab";
        total += i;
        if i % 50 == 49 {
            Out.println(s);
        }
    }
    Out.println(total);
    return 0;
}
"#;

/// リストに数と文字列を積む。
const LIST_PROGRAM: &str = r#"
fn main() -> Int {
    var xs = List<Int>();
    var names = List<String>();
    for i in 0..300 {
        xs.add(i);
        names.add("item-" + "x");
        if i % 100 == 99 {
            Out.println(i);
        }
    }
    Out.println("done");
    return 0;
}
"#;

/// 文字列のプログラムが成功へ変わる付近。
fn string_budgets() -> Vec<usize> {
    (0..20).map(|k| 4000 + 250 * k).collect()
}

/// リストのプログラムが成功へ変わる付近。
fn list_budgets() -> Vec<usize> {
    (0..20).map(|k| 100_000 + 10_000 * k).collect()
}

fn dir() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "rewind-budget-positions-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // A directory left by an earlier process with the same id must not break this run.
    let _ = fs::remove_dir_all(&root);
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

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
}

/// (終了コード, stderr の先頭 2 行, stdout の長さ, stdout の FNV-1a)
type Row = (i32, String, usize, u64);

fn summary(out: &Output) -> Row {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let head = stderr.lines().take(2).collect::<Vec<_>>().join(" | ");
    (out.status.code().unwrap(), head, out.stdout.len(), fnv(&out.stdout))
}

/// 各予算で実行し、結果が前の予算から変わった所と最後だけ、記録を replay して同じ結果になることを確かめる。
fn run_all(source: &str, budgets: &[usize]) -> Vec<(usize, Row, bool)> {
    let root = dir();
    fs::write(root.join("main.rw"), source).unwrap();
    let mut rows: Vec<(usize, Row, bool)> = Vec::new();
    for (index, budget) in budgets.iter().enumerate() {
        let text = budget.to_string();
        let run = call(&root, &["run", "main.rw", "--history-memory", &text, "--record", "trace.json"]);
        let first = summary(&run);
        let changed = rows.last().map_or(true, |(_, previous, _)| *previous != first);
        let replayed = (changed || index + 1 == budgets.len()) && root.join("trace.json").exists();
        if replayed {
            let replay = summary(&call(&root, &["replay", "trace.json"]));
            assert_eq!(replay, first, "replay differs at --history-memory {budget}");
        }
        rows.push((*budget, first, replayed));
        let _ = fs::remove_file(root.join("trace.json"));
    }
    fs::remove_dir_all(root).unwrap();
    rows
}

/// (--history-memory, 終了コード, stderr の先頭 2 行, stdout の長さ, stdout の FNV-1a, replay したか)
type Expected = (usize, i32, &'static str, usize, u64, bool);

const STRING_EXPECTED: [Expected; 20] = [
    (4000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, true),
    (4250, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (4500, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (4750, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (5000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (5250, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (5500, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (5750, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, true),
    (6000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (6250, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (6500, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, true),
    (6750, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (7000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (7250, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:5:5", 0, 14695981039346656037, false),
    (7500, 0, "", 1010, 13707941645729629024, true),
    (7750, 0, "", 1010, 13707941645729629024, false),
    (8000, 0, "", 1010, 13707941645729629024, false),
    (8250, 0, "", 1010, 13707941645729629024, false),
    (8500, 0, "", 1010, 13707941645729629024, false),
    (8750, 0, "", 1010, 13707941645729629024, true),
];

const LIST_EXPECTED: [Expected; 20] = [
    (100000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, true),
    (110000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (120000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (130000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (140000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (150000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (160000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (170000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (180000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (190000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (200000, 71, "error[HistoryMemory]: history budget exceeded |   --> main.rw:7:9", 0, 14695981039346656037, false),
    (210000, 0, "", 16, 2442555593850704278, true),
    (220000, 0, "", 16, 2442555593850704278, false),
    (230000, 0, "", 16, 2442555593850704278, false),
    (240000, 0, "", 16, 2442555593850704278, false),
    (250000, 0, "", 16, 2442555593850704278, false),
    (260000, 0, "", 16, 2442555593850704278, false),
    (270000, 0, "", 16, 2442555593850704278, false),
    (280000, 0, "", 16, 2442555593850704278, false),
    (290000, 0, "", 16, 2442555593850704278, true),
];

fn check(source: &str, budgets: &[usize], expected: &[Expected]) {
    let got: Vec<Expected> = run_all(source, budgets)
        .into_iter()
        .map(|(b, r, p)| (b, r.0, Box::leak(r.1.into_boxed_str()) as &'static str, r.2, r.3, p))
        .collect();
    assert_eq!(got, expected, "{got:#?}");
}

#[test]
fn string_program_results_by_history_memory() {
    check(STRING_PROGRAM, &string_budgets(), &STRING_EXPECTED);
}

#[test]
fn list_program_results_by_history_memory() {
    check(LIST_PROGRAM, &list_budgets(), &LIST_EXPECTED);
}
