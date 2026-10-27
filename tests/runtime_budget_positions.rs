// enforce_budget を速くする前に、予算超過が起きる位置（何回目の操作か）を今の実装で固定する。
// 定数は現行ビルドの出力で、壁時計には依存しない。
use rewind::{
    numeric::{Array, DType},
    ResourceBudget, Runtime, Value,
};

const STEPS: usize = 3000;

#[derive(Clone, Copy)]
enum Mode {
    Plain,
    AfterCommit,
    AfterRevert,
    InBranch,
}

fn budget(history_memory: usize) -> ResourceBudget {
    ResourceBudget {
        history_memory,
        history_storage: 8 * 1024 * 1024,
        spill_threshold: 8 * 1024 * 1024,
    }
}

fn one_step(rt: &mut Runtime, i: usize, mixed: bool) -> rewind::Result<()> {
    if mixed {
        match i % 4 {
            0 => rt.push_stack(Value::Int(i as i64)),
            1 => rt.push_stack(Value::Text(
                format!("text-value-{i}-{}", "x".repeat(i % 17)).into(),
            )),
            2 => rt.set_local(
                format!("n{}", i % 5),
                Value::NumericArray(Array::zeros(DType::Int64, vec![16]).unwrap()),
            ),
            _ => rt.set_local(format!("x{}", i % 7), Value::Int(i as i64)),
        }
    } else if i % 2 == 0 {
        rt.push_stack(Value::Int(i as i64))
    } else {
        rt.set_local(format!("x{}", i % 7), Value::Int(i as i64))
    }
}

/// 予算超過になった最初の操作の番号（0 始まり）と種類。準備の段階で失敗したら "setup"。
/// commit / revert / begin_branch は、状態がある程度育った 100 回目の操作の直前に行う。
fn first_exceeded(history_memory: usize, mode: Mode, mixed: bool) -> (Option<usize>, &'static str) {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    if rt.set_budget(budget(history_memory)).is_err() {
        return (None, "setup");
    }
    if rt.push_frame(0).is_err() {
        return (None, "setup");
    }
    let mut anchor = None;
    for i in 0..STEPS {
        match (mode, i) {
            (Mode::AfterCommit | Mode::AfterRevert, 100) => {
                if rt.commit("c").is_err() {
                    return (Some(i), rt.history_budget_kind());
                }
            }
            (Mode::AfterRevert, 150) => rt.revert("c").unwrap(),
            (Mode::InBranch, 100) => anchor = Some(rt.begin_branch()),
            _ => {}
        }
        match one_step(&mut rt, i, mixed) {
            Ok(()) => {}
            Err(rewind::Error::HistoryBudgetExceeded) => {
                drop(anchor);
                return (Some(i), rt.history_budget_kind());
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
    drop(anchor);
    (None, "none")
}

/// write_output と push_stack を交互に呼び、spill が起きる回を数える。
fn spill_run(
    spill_threshold: usize,
    history_storage: usize,
) -> (Vec<usize>, usize, Option<usize>, &'static str) {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    rt.set_budget(ResourceBudget {
        history_memory: 1024 * 1024,
        history_storage,
        spill_threshold,
    })
    .unwrap();
    let mut spills = Vec::new();
    let mut last = 0;
    for i in 0..400 {
        let r = if i % 2 == 0 {
            rt.write_output(
                format!(
                    "line {i:04} {}
",
                    "y".repeat(40)
                )
                .as_bytes(),
            )
        } else {
            rt.push_stack(Value::Int(i as i64))
        };
        match r {
            Ok(()) => {}
            Err(rewind::Error::HistoryBudgetExceeded) => {
                return (spills, last, Some(i), rt.history_budget_kind());
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
        let now = rt.state().stdout.storage_bytes();
        if now > last {
            spills.push(i);
            last = now;
        }
    }
    (spills, last, None, "none")
}

/// 試す history_memory。
const BUDGETS: [usize; 24] = [
    0, 250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000,
    5500, 7000, 8500, 10000, 11500, 13000, 14500,
];

const PLAIN: [Option<usize>; 24] = [
    Some(0),
    Some(44),
    Some(106),
    Some(170),
    Some(232),
    Some(294),
    Some(356),
    Some(420),
    Some(482),
    Some(544),
    Some(606),
    Some(670),
    Some(732),
    Some(794),
    Some(856),
    Some(920),
    Some(982),
    Some(1356),
    Some(1732),
    Some(2106),
    Some(2482),
    Some(2856),
    None,
    None,
];

const PLAIN_MIXED: [Option<usize>; 24] = [
    Some(0),
    Some(2),
    Some(6),
    Some(6),
    Some(10),
    Some(10),
    Some(14),
    Some(14),
    Some(18),
    Some(18),
    Some(45),
    Some(80),
    Some(113),
    Some(145),
    Some(177),
    Some(209),
    Some(241),
    Some(436),
    Some(628),
    Some(821),
    Some(1013),
    Some(1201),
    Some(1389),
    Some(1577),
];

const AFTER_COMMIT: [Option<usize>; 24] = [
    Some(0),
    Some(44),
    Some(100),
    Some(100),
    Some(114),
    Some(176),
    Some(240),
    Some(302),
    Some(364),
    Some(426),
    Some(490),
    Some(552),
    Some(614),
    Some(676),
    Some(740),
    Some(802),
    Some(864),
    Some(1240),
    Some(1614),
    Some(1990),
    Some(2364),
    Some(2740),
    None,
    None,
];

const AFTER_COMMIT_MIXED: [Option<usize>; 24] = [
    Some(0),
    Some(2),
    Some(6),
    Some(6),
    Some(10),
    Some(10),
    Some(14),
    Some(14),
    Some(18),
    Some(18),
    Some(45),
    Some(80),
    Some(100),
    Some(100),
    Some(100),
    Some(102),
    Some(102),
    Some(102),
    Some(253),
    Some(449),
    Some(641),
    Some(836),
    Some(1029),
    Some(1216),
];

const AFTER_REVERT: [Option<usize>; 24] = [
    Some(0),
    Some(44),
    Some(100),
    Some(100),
    Some(114),
    Some(226),
    Some(290),
    Some(352),
    Some(414),
    Some(476),
    Some(540),
    Some(602),
    Some(664),
    Some(726),
    Some(790),
    Some(852),
    Some(914),
    Some(1290),
    Some(1664),
    Some(2040),
    Some(2414),
    Some(2790),
    None,
    None,
];

const AFTER_REVERT_MIXED: [Option<usize>; 24] = [
    Some(0),
    Some(2),
    Some(6),
    Some(6),
    Some(10),
    Some(10),
    Some(14),
    Some(14),
    Some(18),
    Some(18),
    Some(45),
    Some(80),
    Some(100),
    Some(100),
    Some(100),
    Some(102),
    Some(102),
    Some(102),
    Some(308),
    Some(501),
    Some(693),
    Some(889),
    Some(1080),
    Some(1268),
];

const IN_BRANCH: [Option<usize>; 24] = [
    Some(0),
    Some(44),
    Some(100),
    Some(100),
    Some(114),
    Some(176),
    Some(240),
    Some(302),
    Some(364),
    Some(426),
    Some(490),
    Some(552),
    Some(614),
    Some(676),
    Some(740),
    Some(802),
    Some(864),
    Some(1240),
    Some(1614),
    Some(1990),
    Some(2364),
    Some(2740),
    None,
    None,
];

const IN_BRANCH_MIXED: [Option<usize>; 24] = [
    Some(0),
    Some(2),
    Some(6),
    Some(6),
    Some(10),
    Some(10),
    Some(14),
    Some(14),
    Some(18),
    Some(18),
    Some(45),
    Some(80),
    Some(100),
    Some(100),
    Some(100),
    Some(102),
    Some(102),
    Some(102),
    Some(253),
    Some(449),
    Some(641),
    Some(836),
    Some(1029),
    Some(1216),
];

fn check(mode: Mode, mixed: bool, expected: &[Option<usize>]) {
    let got: Vec<Option<usize>> = BUDGETS
        .iter()
        .map(|&hm| {
            let (at, kind) = first_exceeded(hm, mode, mixed);
            assert_eq!(
                kind,
                if at.is_some() {
                    "HistoryMemory"
                } else {
                    "none"
                },
                "hm={hm}"
            );
            at
        })
        .collect();
    assert_eq!(got, expected, "{got:?}");
}

#[test]
fn scalar_positions_plain() {
    check(Mode::Plain, false, &PLAIN);
}
#[test]
fn scalar_positions_after_commit() {
    check(Mode::AfterCommit, false, &AFTER_COMMIT);
}
#[test]
fn scalar_positions_after_revert() {
    check(Mode::AfterRevert, false, &AFTER_REVERT);
}
#[test]
fn scalar_positions_in_branch() {
    check(Mode::InBranch, false, &IN_BRANCH);
}
#[test]
fn mixed_positions_plain() {
    check(Mode::Plain, true, &PLAIN_MIXED);
}
#[test]
fn mixed_positions_after_commit() {
    check(Mode::AfterCommit, true, &AFTER_COMMIT_MIXED);
}
#[test]
fn mixed_positions_after_revert() {
    check(Mode::AfterRevert, true, &AFTER_REVERT_MIXED);
}
#[test]
fn mixed_positions_in_branch() {
    check(Mode::InBranch, true, &IN_BRANCH_MIXED);
}

/// (spill_threshold, history_storage, spill 回数, 最初の 6 回の位置, 最終の storage バイト, 超過位置, 種類)
type SpillRow = (
    usize,
    usize,
    usize,
    &'static [usize],
    usize,
    Option<usize>,
    &'static str,
);
const SPILLS: [SpillRow; 9] = [
    (64, 100, 1, &[2], 51, Some(4), "HistoryStorage"),
    (
        64,
        1000,
        19,
        &[2, 4, 6, 8, 10, 12],
        969,
        Some(40),
        "HistoryStorage",
    ),
    (64, 100000, 199, &[2, 4, 6, 8, 10, 12], 10149, None, "none"),
    (200, 100, 1, &[6], 51, Some(8), "HistoryStorage"),
    (
        200,
        1000,
        19,
        &[6, 8, 10, 12, 14, 16],
        969,
        Some(44),
        "HistoryStorage",
    ),
    (
        200,
        100000,
        197,
        &[6, 8, 10, 12, 14, 16],
        10047,
        None,
        "none",
    ),
    (1000, 100, 1, &[38], 51, Some(40), "HistoryStorage"),
    (
        1000,
        1000,
        19,
        &[38, 40, 42, 44, 46, 48],
        969,
        Some(76),
        "HistoryStorage",
    ),
    (
        1000,
        100000,
        181,
        &[38, 40, 42, 44, 46, 48],
        9231,
        None,
        "none",
    ),
];

#[test]
fn spill_positions_with_interleaved_output_and_push() {
    for &(t, st, count, first, last, at, kind) in &SPILLS {
        let (spills, last_bytes, got_at, got_kind) = spill_run(t, st);
        assert_eq!(spills.len(), count, "t={t} st={st}");
        assert_eq!(&spills[..spills.len().min(6)], first, "t={t} st={st}");
        assert_eq!(
            (last_bytes, got_at, got_kind),
            (last, at, kind),
            "t={t} st={st}"
        );
    }
}
