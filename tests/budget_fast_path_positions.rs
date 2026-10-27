// enforce_budget の速い経路が、全体検査と同じ位置で予算超過を返すことを固定する。
// 期待値は、速い経路を入れる前の実装で出したもの。壁時計には依存しない。
// 操作は、スタックと局所変数の追加・削除、ヒープの書き換え、checkpoint の commit と revert、
// 出力の追記を決定的に混ぜる。
use rewind::{ResourceBudget, Runtime, Value};

const STEPS: usize = 1500;

fn budget(history_memory: usize) -> ResourceBudget {
    ResourceBudget {
        history_memory,
        history_storage: 8 * 1024 * 1024,
        spill_threshold: 512,
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

/// 予算超過になった最初の操作の番号。超過しなければ None。
fn first_exceeded(history_memory: usize, accounting: bool, seed: u64) -> Option<usize> {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    if accounting {
        rt.enable_container_accounting();
    }
    if rt.set_budget(budget(history_memory)).is_err() {
        return Some(usize::MAX);
    }
    let mut rng = Lcg(seed);
    if rt.push_frame(0).is_err() {
        return Some(usize::MAX);
    }
    let mut heap_ids: Vec<u64> = Vec::new();
    let mut checkpointed = false;
    for i in 0..STEPS {
        if i == 200 {
            if rt.commit("c").is_err() {
                return Some(i);
            }
            checkpointed = true;
        }
        if i == 700 && checkpointed {
            rt.revert("c").unwrap();
        }
        let n = rng.next();
        let result: rewind::Result<()> = match n % 13 {
            0 | 1 => rt.push_stack(Value::Int(n as i64)),
            2 => rt.push_stack(Value::Text(
                format!("t{n}-{}", "x".repeat((n % 23) as usize)).into(),
            )),
            3 => rt.pop_stack().map(|_| ()),
            4 => rt.push_frame(i),
            5 => rt.pop_frame().map(|_| ()),
            6 | 7 => rt.set_local(format!("l{}", n % 9), Value::Int(n as i64)),
            8 => rt.set_local(
                format!("s{}", n % 5),
                Value::Text(format!("v{}", "y".repeat((n % 31) as usize)).into()),
            ),
            9 => rt.remove_local(&format!("l{}", n % 9)),
            10 => match rt.alloc(Value::Int(n as i64)) {
                Ok(id) => {
                    heap_ids.push(id);
                    Ok(())
                }
                Err(e) => Err(e),
            },
            11 if !heap_ids.is_empty() => {
                let id = heap_ids[(n as usize / 13) % heap_ids.len()];
                rt.heap_set(id, Value::Int((n % 1000) as i64))
            }
            12 if !heap_ids.is_empty() => {
                let id = heap_ids[(n as usize / 13) % heap_ids.len()];
                rt.heap_set(
                    id,
                    Value::Text(format!("h{}", "z".repeat((n % 40) as usize)).into()),
                )
            }
            _ => rt.write_output(format!("out {i}\n").as_bytes()),
        };
        match result {
            Ok(()) => {}
            Err(rewind::Error::HistoryBudgetExceeded) => return Some(i),
            // 空のスタックや存在しない局所変数の操作は、予算とは無関係なので読み飛ばす。
            Err(rewind::Error::InvalidOperation(_)) => {}
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
    None
}

const BUDGETS: [usize; 12] = [
    0, 400, 800, 1200, 1600, 2400, 3200, 4800, 6400, 9600, 14000, 20000,
];
/// 容器の会計では 1 つの値の費用が大きいので、予算も大きく取る。
const ACCOUNTING_BUDGETS: [usize; 12] = [
    0, 3000, 6000, 10000, 16000, 24000, 36000, 54000, 80000, 120000, 180000, 270000,
];
const SEEDS: [u64; 3] = [1, 7, 2026];

fn positions(accounting: bool) -> Vec<Vec<Option<usize>>> {
    SEEDS
        .iter()
        .map(|&seed| {
            let budgets = if accounting {
                &ACCOUNTING_BUDGETS
            } else {
                &BUDGETS
            };
            budgets
                .iter()
                .map(|&hm| first_exceeded(hm, accounting, seed))
                .collect()
        })
        .collect()
}

#[test]
fn positions_without_accounting() {
    let got = positions(false);
    assert_eq!(got, expected_plain());
}

#[test]
fn positions_with_container_accounting() {
    let got = positions(true);
    assert_eq!(got, expected_accounting());
}

fn expected_plain() -> Vec<Vec<Option<usize>>> {
    vec![
        vec![
            Some(4),
            Some(141),
            Some(202),
            Some(235),
            Some(416),
            Some(1114),
            Some(1333),
            None,
            None,
            None,
            None,
            None,
        ],
        vec![
            Some(0),
            Some(113),
            Some(203),
            Some(204),
            Some(300),
            Some(506),
            Some(1426),
            None,
            None,
            None,
            None,
            None,
        ],
        vec![
            Some(0),
            Some(68),
            Some(150),
            Some(204),
            Some(217),
            Some(426),
            Some(1255),
            None,
            None,
            None,
            None,
            None,
        ],
    ]
}

fn expected_accounting() -> Vec<Vec<Option<usize>>> {
    vec![
        vec![
            Some(4),
            Some(9),
            Some(27),
            Some(56),
            Some(100),
            Some(187),
            Some(223),
            Some(328),
            Some(554),
            Some(1466),
            None,
            None,
        ],
        vec![
            Some(0),
            Some(20),
            Some(64),
            Some(102),
            Some(123),
            Some(203),
            Some(219),
            Some(354),
            Some(610),
            Some(1368),
            None,
            None,
        ],
        vec![
            Some(0),
            Some(5),
            Some(25),
            Some(42),
            Some(70),
            Some(117),
            Some(200),
            Some(227),
            Some(362),
            Some(652),
            None,
            None,
        ],
    ]
}

// ---- ここから下は、超過のあとも続ける経路、spill の時機、会計の組み合わせ、branch / heap 共有 ----
// 期待値は、速い経路を入れる前の実装（HEAD）で採ったもの。REWIND_DUMP=1 で現在の値を表示する。
use rewind::numeric::{Array, DType};

#[derive(Clone, Copy, Default)]
struct Cfg {
    numeric: bool,
    shared: bool,
    bytes: bool,
    alloc: bool,
    spill_threshold: usize,
}

/// (拒否された区間 [始点, 長さ] の一覧, spill の回数, 最初の 4 回の位置)
type Trace = (Vec<(usize, usize)>, usize, Vec<usize>);

fn run_scenario(
    cfg: Cfg,
    history_memory: usize,
    seed: u64,
    steps: usize,
    structural: bool,
) -> Trace {
    let mut rt = Runtime::new(std::env::temp_dir()).unwrap();
    if cfg.numeric {
        rt.enable_numeric_accounting();
    }
    if cfg.shared {
        rt.enable_shared_payload_accounting();
    }
    if cfg.bytes {
        rt.enable_byte_payload_accounting();
    }
    if cfg.alloc {
        rt.enable_allocation_accounting();
    }
    if rt
        .set_budget(ResourceBudget {
            history_memory,
            history_storage: 8 * 1024 * 1024,
            spill_threshold: if cfg.spill_threshold == 0 {
                512
            } else {
                cfg.spill_threshold
            },
        })
        .is_err()
        || rt.push_frame(0).is_err()
    {
        return (vec![(0, usize::MAX)], 0, vec![]);
    }
    let mut rng = Lcg(seed);
    let mut heap_ids: Vec<u64> = Vec::new();
    let mut anchor = None;
    let mut have_checkpoint = false;
    let mut rejected: Vec<usize> = Vec::new();
    let mut spills = Vec::new();
    let mut last_storage = 0;
    for i in 0..steps {
        let n = rng.next();
        let result: rewind::Result<()> = if structural && i % 97 == 40 {
            // checkpoint が heap を共有している状態を作る。
            have_checkpoint = true;
            rt.commit("c")
        } else if structural && i % 97 == 41 && anchor.is_none() {
            anchor = Some(rt.begin_branch());
            Ok(())
        } else if structural && i % 97 == 70 && anchor.is_some() {
            rt.end_branch(format!("b{i}"), anchor.take().unwrap())
        } else if structural && i % 97 == 85 && have_checkpoint {
            rt.revert("c")
        } else if structural && i % 5 == 3 {
            rt.set_program_counter(i);
            Ok(())
        } else {
            match n % 11 {
                0 | 1 => rt.push_stack(Value::Int(n as i64)),
                2 => rt.pop_stack().map(|_| ()),
                3 => rt.set_local(format!("l{}", n % 7), Value::Int(n as i64)),
                4 => rt.write_output(
                    format!("out {i} {}\n", "o".repeat((n % 30) as usize)).as_bytes(),
                ),
                5 => {
                    let v = match n % 3 {
                        0 if cfg.numeric => Value::NumericArray(
                            Array::zeros(DType::Int64, vec![(n % 12 + 1) as usize]).unwrap(),
                        ),
                        1 => Value::Bytes(std::sync::Arc::new(vec![7u8; (n % 50) as usize])),
                        _ => Value::Int(n as i64),
                    };
                    rt.alloc(v).map(|id| heap_ids.push(id))
                }
                6 | 7 if !heap_ids.is_empty() => {
                    let id = heap_ids[(n as usize / 11) % heap_ids.len()];
                    let v = if cfg.numeric && n % 2 == 0 {
                        Value::NumericArray(
                            Array::zeros(DType::Int64, vec![(n % 20 + 1) as usize]).unwrap(),
                        )
                    } else {
                        Value::Text(format!("h{}", "z".repeat((n % 40) as usize)).into())
                    };
                    rt.heap_set(id, v)
                }
                8 => rt.push_frame(i),
                9 => rt.pop_frame().map(|_| ()),
                _ => rt.set_local(
                    format!("t{}", n % 4),
                    Value::Text("q".repeat((n % 25) as usize).into()),
                ),
            }
        };
        match result {
            Ok(()) => {}
            Err(rewind::Error::HistoryBudgetExceeded) => rejected.push(i),
            Err(rewind::Error::InvalidOperation(_)) => {}
            Err(other) => panic!("unexpected error: {other}"),
        }
        let now = rt.state().stdout.storage_bytes();
        if now > last_storage {
            spills.push(i);
        }
        last_storage = now;
    }
    drop(anchor);
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for i in rejected {
        match ranges.last_mut() {
            Some((s, l)) if *s + *l == i => *l += 1,
            _ => ranges.push((i, 1)),
        }
    }
    let count = spills.len();
    spills.truncate(4);
    (ranges, count, spills)
}

fn check_trace(name: &str, got: Vec<Trace>, expected: &str) {
    // 区間の一覧は長いので、(区間数, 拒否の総数, 先頭 3 区間, 全区間の FNV-1a) に縮めて固定する。
    let summary: Vec<_> = got
        .iter()
        .map(|(ranges, spills, first)| {
            let mut hash: u64 = 0xcbf29ce484222325;
            for &(s, l) in ranges {
                for v in [s as u64, l as u64] {
                    hash = (hash ^ v).wrapping_mul(0x100000001b3);
                }
            }
            let total: usize = ranges.iter().map(|r| r.1).sum();
            (
                ranges.len(),
                total,
                ranges.iter().take(3).copied().collect::<Vec<_>>(),
                hash,
                *spills,
                first.clone(),
            )
        })
        .collect();
    let text = format!("{summary:?}");
    if std::env::var_os("REWIND_DUMP").is_some() {
        println!("DUMP {name} = {text}");
        return;
    }
    assert_eq!(text, expected, "{name}");
}

fn traces(cfg: Cfg, budgets: &[usize], steps: usize, structural: bool) -> Vec<Trace> {
    let mut out = Vec::new();
    for &seed in &[3u64, 11] {
        for &hm in budgets {
            out.push(run_scenario(cfg, hm, seed, steps, structural));
        }
    }
    out
}

const CONT_BUDGETS: [usize; 4] = [600, 1500, 3000, 6000];

#[test]
fn continue_after_first_exceeded_plain() {
    let got = traces(Cfg::default(), &CONT_BUDGETS, 500, false);
    check_trace("continue_plain", got, EXPECT_CONTINUE_PLAIN);
}

#[test]
fn spill_timing_small_threshold() {
    let mut got = Vec::new();
    for &t in &[64usize, 200, 1000] {
        got.extend(traces(
            Cfg {
                spill_threshold: t,
                ..Cfg::default()
            },
            &[100000],
            400,
            false,
        ));
    }
    check_trace("spill_timing", got, EXPECT_SPILL);
}

#[test]
fn numeric_accounting_alone() {
    let got = traces(
        Cfg {
            numeric: true,
            ..Cfg::default()
        },
        &CONT_BUDGETS,
        500,
        false,
    );
    check_trace("numeric", got, EXPECT_NUMERIC);
}

#[test]
fn shared_and_byte_payload_accounting() {
    let got = traces(
        Cfg {
            numeric: true,
            shared: true,
            bytes: true,
            ..Cfg::default()
        },
        &CONT_BUDGETS,
        500,
        false,
    );
    check_trace("shared", got, EXPECT_SHARED);
}

#[test]
fn allocation_accounting_alone() {
    let got = traces(
        Cfg {
            alloc: true,
            ..Cfg::default()
        },
        &CONT_BUDGETS,
        500,
        false,
    );
    check_trace("alloc", got, EXPECT_ALLOC);
}

#[test]
fn branch_checkpoint_and_shared_heap() {
    let got = traces(
        Cfg {
            numeric: true,
            ..Cfg::default()
        },
        &CONT_BUDGETS,
        600,
        true,
    );
    check_trace("structural", got, EXPECT_STRUCTURAL);
}

const EXPECT_CONTINUE_PLAIN: &str = "[(49, 95, [(181, 1), (190, 4), (227, 1)], 16237899216768556007, 40, [119, 135, 137, 144]), (0, 0, [], 14695981039346656037, 32, [211, 214, 231, 237]), (0, 0, [], 14695981039346656037, 21, [211, 214, 231, 237]), (0, 0, [], 14695981039346656037, 21, [211, 214, 231, 237]), (34, 71, [(273, 1), (275, 3), (279, 3)], 15740531083129295650, 42, [152, 153, 170, 172]), (0, 0, [], 14695981039346656037, 28, [261, 268, 282, 289]), (0, 0, [], 14695981039346656037, 20, [261, 268, 282, 289]), (0, 0, [], 14695981039346656037, 20, [261, 268, 282, 289])]";
const EXPECT_SPILL: &str = "[(0, 0, [], 14695981039346656037, 30, [32, 39, 43, 57]), (0, 0, [], 14695981039346656037, 23, [75, 78, 105, 122]), (0, 0, [], 14695981039346656037, 25, [73, 88, 97, 109]), (0, 0, [], 14695981039346656037, 21, [145, 186, 199, 203]), (0, 0, [], 14695981039346656037, 0, []), (0, 0, [], 14695981039346656037, 0, [])]";
const EXPECT_NUMERIC: &str = "[(80, 162, [(13, 1), (38, 1), (77, 1)], 7229245854905688066, 43, [69, 71, 73, 83]), (55, 95, [(87, 1), (112, 1), (135, 1)], 4862609180109802815, 37, [86, 88, 117, 119]), (46, 60, [(147, 1), (150, 1), (178, 1)], 5307533879607228901, 29, [135, 137, 144, 145]), (19, 29, [(324, 1), (363, 1), (366, 1)], 16797362713893582428, 22, [211, 214, 231, 237]), (66, 99, [(22, 1), (26, 1), (36, 2)], 13409742313760874901, 44, [85, 86, 105, 123]), (53, 90, [(37, 1), (45, 1), (52, 1)], 13375569513585771079, 41, [166, 167, 187, 199]), (32, 41, [(217, 1), (230, 1), (236, 1)], 3093220667191440329, 27, [196, 201, 214, 215]), (6, 7, [(401, 1), (410, 1), (453, 1)], 4278815752603025184, 26, [261, 268, 282, 289])]";
const EXPECT_SHARED: &str = "[(84, 166, [(3, 1), (13, 1), (38, 1)], 6563737883322207676, 41, [65, 66, 69, 71]), (75, 117, [(77, 1), (86, 2), (89, 1)], 6096914044404686851, 31, [69, 72, 73, 112]), (49, 70, [(137, 1), (147, 2), (150, 1)], 10374695179209023110, 28, [89, 127, 132, 135]), (29, 47, [(244, 2), (247, 5), (254, 1)], 2442576535685905083, 28, [160, 178, 180, 181]), (69, 132, [(22, 1), (26, 1), (36, 2)], 4428449238548170695, 40, [133, 135, 145, 150]), (45, 71, [(37, 1), (42, 1), (45, 1)], 7282708421542954346, 24, [108, 109, 122, 129]), (36, 39, [(123, 1), (132, 1), (217, 1)], 7171717235684872310, 29, [109, 205, 212, 214]), (19, 26, [(360, 1), (366, 1), (374, 1)], 18072810477859400488, 24, [261, 268, 282, 289])]";
const EXPECT_ALLOC: &str = "[(114, 246, [(1, 1), (13, 1), (22, 1)], 9979518400464679242, 35, [17, 61, 65, 132]), (123, 206, [(13, 1), (22, 1), (40, 1)], 7193712667910485843, 40, [37, 38, 39, 43]), (115, 203, [(40, 1), (42, 1), (44, 1)], 13708277930788544085, 34, [43, 45, 65, 72]), (92, 150, [(59, 1), (69, 1), (78, 1)], 2060597777200312353, 27, [66, 67, 73, 133]), (105, 223, [(7, 1), (10, 2), (14, 1)], 2141120874855411459, 43, [20, 52, 102, 105]), (88, 140, [(11, 1), (16, 3), (20, 1)], 17400733298520469430, 31, [72, 76, 121, 122]), (87, 140, [(33, 1), (37, 1), (42, 1)], 10590141602756517546, 33, [84, 105, 121, 122]), (78, 127, [(73, 1), (88, 1), (110, 1)], 9892170443284127723, 33, [115, 122, 123, 145])]";
const EXPECT_STRUCTURAL: &str = "[(118, 203, [(69, 1), (82, 1), (86, 2)], 12415505189276609056, 10, [57, 59, 65, 66]), (61, 85, [(87, 1), (112, 1), (132, 1)], 12257561076425978339, 23, [77, 81, 117, 151]), (54, 81, [(147, 1), (150, 1), (160, 2)], 4396226485603138544, 16, [139, 142, 144, 149]), (24, 29, [(360, 1), (370, 1), (402, 1)], 13307255836683770745, 9, [260, 277, 409, 425]), (129, 237, [(22, 1), (26, 1), (36, 2)], 1764570667635121840, 9, [50, 72, 76, 186]), (111, 176, [(37, 1), (42, 1), (45, 1)], 2789823109805765234, 28, [139, 140, 145, 149]), (102, 173, [(131, 2), (159, 2), (169, 3)], 11997641776940306439, 2, [150, 152]), (48, 83, [(262, 1), (317, 1), (335, 1)], 10008010019313129340, 13, [251, 257, 259, 270])]";
