# PERF-02/03/04/05・NUM-01 実測記録（Windows、2026-10-18）

[性能計画](v2-next-performance.md)のうち、PERF-02（既存caseの測定）、NUM-01（数値pipeline）、PERF-03（保持領域）、PERF-05（長時間運転）を実機で測った記録です。測定で見つかった問題のうち、原因が特定できて小さく直せたものはPERF-04として独立commitにしました。**この記録の数値は下の1台のWindows機の値で、Linuxや他のmachineへ一般化しません。**

## 環境と方法

| 項目 | 内容 |
|---|---|
| OS / CPU | Windows 11 (10.0.26300)、AMD64 Family 25 Model 117、論理16 core |
| build | `cargo build --release`（rustc 1.98.1、`target-feature=+crt-static`） |
| 比較binary | baseline = 変更前（HEAD `a56c22f`）のrelease、sha256先頭16桁 `1c32689e29811472`。candidate = 下記commit適用後 |
| 実行 | `rewind run main.rwc`、`--history-memory 1GiB --native-work 100000000000 --steps 1000000000 --task-steps 1000000000`。compile時間は含めない |
| 反復 | warm-up 3、測定30、baseline/candidateを交互に実行。中央値とp95（30件から推定した値で、厳密な尾部保証ではない） |
| メモリ | `peak_ws` = PeakWorkingSetSize（常駐）、`peak_commit` = PeakPagefileUsage（private commit）。Linux RSSとは別物 |
| 取得していない値 | GC時間（`rewind profile`で長時間caseの最後に1回だけ取得）、byte単位のallocator保持、Windowsのprivate working set |

測定用script（Linux専用の既存`benchmark-*.py`は`os.wait4`を使うためWindowsでは動かない）:

| script | 役割 |
|---|---|
| [scripts/perf_win.py](../scripts/perf_win.py) | child processのwall・CPU・peak working set/commit、任意でhandle数の時系列 |
| [scripts/perf_suite_win.py](../scripts/perf_suite_win.py) | PERF-02/NUM-01のcaseを複数binaryで交互に測定 |
| [scripts/perf_retention_win.py](../scripts/perf_retention_win.py) | PERF-03/05。同じcycleをN・2N・4N回、memory・handle・VM ledgerを記録 |
| [scripts/sample_win.py](../scripts/sample_win.py) | 管理者権限なしの簡易sampling profiler（leaf関数と行） |

```bash
python scripts/perf_suite_win.py BASELINE.exe CANDIDATE.exe --warmup 3 --repetitions 30 --output result.json
python scripts/perf_retention_win.py CANDIDATE.exe --cycles 2500 --output retention.json
```

## 見つかった問題と変更（commit）

| commit | 内容 |
|---|---|
| `76d4b1f` | VMの1命令あたりの確保を削減（下記PERF-04-a） |
| `4ffa155` | 非連続view（転置・負stride等）の走査（NUM-01） |
| `776ac40` | 通常runでスケジュール記録が増え続ける問題の修正と、予算確認のhash軽量化 |

いずれも`[skip ci]`でcodex/developに保存しただけで、push・公開CI・版更新・main統合は行っていません。

### PERF-04-a：VMの命令実行が確保だらけだった

空の10万回`for`が、変更前のreleaseで約1.3秒（1反復約13 µs）でした。`REWIND_ALLOC_STATS`相当の計数allocatorを一時的に入れて測ると、1反復で約124回・9.5 KBの確保がありました。原因と対処（意味は変えない）:

- `Runtime::push_stack`/`push_frame`/`set_local`/`set_global`が巻き戻し用にArcをもう1つ保持し、直後の`Arc::make_mut`がスタック全体・全フレームを毎回複製していた。→ 失敗時にpop/元の値の復元で戻す。
- 全状態の予算確認（`enforce_budget`、ほぼ全操作で実行）が毎回`HashSet`を作っていた。→ 24件まで確保なしのPtrSet。24件を超える（checkpointや外部journal segmentが多い）場合だけ軽量hashのHashSet。
- 1命令ごとの`Inst`複製が位置情報の文字列2つを複製していた。→ `Arc<Tok>`。
- `expire_timeouts`が毎命令、位置情報を複製していた。→ timeout taskがあるときだけ。
- `language_at_least`が毎回semverを2回parseしていた。→ 通常の`MAJOR.MINOR.PATCH`は確保なしの比較（それ以外は従来のsemver）。

確保は1反復約124回から約25回になりました。論理仕事量は不変です（`tests/logical_cost.rs`の命令数・出力が一致）。

### PERF-04-b：スケジュール記録の無制限増加

スケジューラが判断ごとに`choice_widths`と`choices_used`へ値を追加し続けていました。読むのはtrace・profile・schedule探索・schedule再生だけなので、それらのoptionがないときは記録しません。**1 taskあたり約60 Bずつ増える実際の増加**で、`await`するtaskを8万回回すとpeak commitが3.9 → 5.4 → 7.8 MiB（2万・4万・8万回）でした。修正後は2.8 / 2.8 / 2.7 MiBで一定です。

## PERF-02：baselineとcandidateの比較（各30回）

入力は1,048,576要素など。`size`以外の定数は`scripts/perf_suite_win.py`のcase定義を参照。

| case | 前 中央値 s | 後 中央値 s | 前 p95 | 後 p95 | 後/前 | peak WS MiB（前→後） | peak commit MiB（前→後） |
|---|---|---|---|---|---|---|---|
| gc_temp_list_string（512 KiB×200） | 0.303 | 0.233 | 0.329 | 0.244 | 0.77 | 17.5 → 17.2 | 8.8 → 8.4 |
| vm_loop_sum_200k | 5.705 | 2.884 | 6.018 | 3.012 | 0.51 | 13.9 → 13.9 | 3.4 → 3.4 |
| mem01_list_churn_65536（2万set） | 3.784 | 2.265 | 4.006 | 2.409 | 0.60 | 28.5 → 29.2 | 19.1 → 19.3 |
| mem01_list_churn_checkpoint | 3.889 | 2.392 | 4.085 | 2.495 | 0.62 | 31.1 → 31.1 | 21.2 → 21.2 |
| mem01_list_128_checkpoints | 2.786 | 1.700 | 2.904 | 1.845 | 0.61 | 33.5 → 33.5 | 23.4 → 23.5 |
| mem02_text_copy_1MiB（2000回） | 0.137 | 0.086 | 0.166 | 0.094 | 0.63 | 14.9 → 15.4 | 5.2 → 5.5 |
| num_withfloat_churn（2万更新） | 2.346 | 1.358 | 2.568 | 1.460 | 0.58 | 17.6 → 17.5 | 7.3 → 7.0 |
| num_withfloat_churn_checkpoint | 2.582 | 1.489 | 2.856 | 1.652 | 0.58 | 17.9 → 17.6 | 7.3 → 7.2 |
| num_pipeline_chain（5段、100万） | 0.196 | 0.176 | 0.220 | 0.221 | 0.90 | 65.2 → 65.2 | 55.4 → 55.6 |
| num_view_sum_only（転置view、20回sum） | 0.605 | 0.244 | 0.689 | 0.286 | 0.40 | 26.7 → 26.7 | 16.2 → 16.3 |
| num_materialize_sum | 0.269 | 0.212 | 0.301 | 0.264 | 0.79 | 36.8 → 36.6 | 26.5 → 26.2 |
| num_list_to_array / num_values_only | 2.511 / 2.371 | 2.518 / 2.360 | — | — | 1.00 | 247.7 → 247.6 | 242.0 → 241.7 |

- メモリは全caseで同等です（上表のpeak列は別に5回ずつ測った中央値で、差は±0.7 MiB以内）。最初に測ったcandidate fileだけ常駐が約2 MiB多く出ましたが、同じ内容を別名でcopyすると消えるOS/ファイル側の差で、コードの差ではありません。
- VM命令が支配的なcaseは約1.6〜2倍速くなりました。数値kernel自体（`num_pipeline_chain`、`num_list_to_array`）は変わりません。
- これらは「`withFloat`の1更新が約70 µs」のように遅く見えますが、中身は更新1回あたりのVM命令と予算確認の費用です（kernelは1ページのコピーのみ）。

## NUM-01：数値pipelineで分かったこと

- **型付き配列のまま渡せる区間は速い**：`zerosFloat → affine → scale → activation → mapFloat → zipFloat → sum`の5段（各100万要素）が約0.18秒、1段あたり約20 ms、peak commit 56 MiB（中間配列が約8 MiBずつ）。追加の`Float`/`List`変換はありません。
- **遅さの本体は`List`との境界**：`valuesFloat`（配列→`List<Float>`）が100万要素で約1.8〜2.4秒、常駐約250 MiB。`fromFloat`（`List`→配列）は同じ`List`から約0.15秒/回です。仕様どおり「要素あたりVM値を作る」費用で、変換をまたぐpipelineを書かないことが一番効きます。`valuesFloat`の内訳は要素ごとの値メタデータ（`value_bytes`/`byte_payload_info`等）と台帳登録で、`List`生成コード自体は単純なcollectです。
- **改善した点**：非連続viewの走査が要素ごとに「div/modで座標→offset」と「根からのpage木の下降」をしていました。座標カウンタ＋現在のleaf pageの保持に変え、転置viewのsumが約20〜25 ms → 約7 ms（100万要素）になりました。要素の順序・丸め順・work計上は同じです（既存の`bits()`契約。cooperative chunkの途中開始は`StridedRange::new`）。単体test 84件、数値/AD/FFT/固有値を使う統合test 29本で一致を確認。
- 追加しなかったもの：elementwise演算のfusion（演算順序・丸め・取消chunk・一時領域の契約を先に決める必要があり、上の測定では1段が20 msで支配的でない）。

## PERF-03：固定rootの繰返し

同じcycleをN・2N・4N回（N=2,500）。各行は1 run。VM ledgerは終了時の値です。

| case | cycles | wall s | peak WS MiB | peak commit MiB |
|---|---|---|---|---|
| temp_text_512KiB（毎回独立な512 KiB） | 2500 / 5000 / 10000 | 0.8 / 1.5 / 2.7 | 14.6 / 15.0 / 15.1 | 5.1 / 5.6 / 5.6 |
| temp_map_100（Map 100件を作って捨てる） | 同上 | 6.1 / 12.2 / 24.3 | 14.0 / 14.2 / 14.2 | 3.6 / 3.5 / 3.6 |
| temp_numeric_1M（100万要素配列を作って捨てる） | 同上 | 2.3 / 4.5 / 8.1 | 26.4 / 26.3 / 27.2 | 16.8 / 16.3 / 16.8 |
| temp_list_1000（1000件のList） | 同上 | 50.5 / 102.3 / 200.0 | 14.6 / 14.7 / 14.9 | 4.2 / 4.3 / 4.5 |
| checkpoint_cycle_fixed（commit/revert/drop） | 同上 | 0.25 / 0.38 / 0.63 | 15.6 / 15.6 / 15.7 | 4.9 / 5.2 / 5.1 |
| task_complete（`await`するtask） | 20000 / 40000 / 80000（修正後） | 1.6 / 3.3 / 6.6 | 14.1 / 14.2 / 14.1 | 2.80 / 2.82 / 2.69 |
| task_cancel_numeric（spawn→cancel→await） | 20000 / 40000 / 80000（修正後） | 5.2 / 10.4 / 21.1 | 19.9 / 19.9 / 20.1 | 9.5 / 9.4 / 9.7 |

- 終了時のVM live ledger（heap objects、shared payload bytes、task数）は**回数に比例して増えません**。`temp_map_4`を2.5万・5万・10万回にすると heap 134 / 84 / 166、shared 436 KB / 272 KB / 541 KB と上下し、GC回数は274 / 549 / 1098で、GCがほぼ2 objects/cycleを回収していました。2,500〜10,000回だけの3点では単調増加に見えることがあり（map_100の heap 4 → 6 → 10、tasks 6 → 10 → 18）、GCのどの時点で終了したかによる変動です。
- handle数は全caseで一定（最大約164、Windowsのthread pool分）。終了時に0。
- 「説明のつかない増加」はPERF-04-bのスケジュール記録だけでした（修正済み）。それ以外のcaseでは測定誤差とallocator保持を超える増加は見られませんでした。checkpointに由来する保持を消す変更は入れていません。
- `temp_list_1000`は`List.add`が約20 µs/回です。これはVM全体の命令コストで、保持の問題ではありません。

## PERF-05：長時間運転（10,000 cycle）

上表のcaseは10,000回（task系は80,000回）まで実行して、memory・handleが一定であることを確認しました。`record_compact_loop`（`--record --record-mode compact`、12万命令/千cycle）も10,000 cycleでpeak commit 3.4 MiB一定です。

**未達：SQLite loop（INSERT OR REPLACE → SELECT → `db.next` → `closeCursor`、:memory:、固定16行）**

| cycles | 変更前 wall s / peak commit MiB | 現在 wall s / peak commit MiB |
|---|---|---|
| 250 | 3.7 / 36.5 | 2.3 / 36.7 |
| 500 | 12.7 / 42.9 | 7.0 / 43.3 |
| 1000 | 49.3 / 52.4 | 25.6 / 52.7 |
| 2000（現在のみ） | — | 100.0 / 74.7 |

10,000 cycleは完走させていません（上の傾向では数十分かかる）。原因を切り分けた結果:

1. **時間が回数の約1.6〜2乗**（1000 → 2000回で3.9倍）。`external`操作の結果（journal segment）は`external_entries`として実行中ずっと保持され（`src/external.rs`、上限100万件）、予算確認が全segmentを毎回走査するためです（INSERTだけのloopでも1000 → 2000回で3.2倍）。軽量hashで約1.5〜1.9倍速くなりましたが、O(件数)の走査は残っています。
2. **peak commitが約20 KiB/cycle増える**。`db.next`を呼ぶ場合だけ増え、`execute`・`query`・`closeCursor`だけでは増えません（`next`を呼ばない変種は約28 MiBで一定）。`closeCursor`の有無・`next`の最大byte指定（1 MiB〜4 MiB）にも依存しません。VM ledger（heap/shared/task）は一定で、**Rustのglobal allocator経由の確保は終了時にほぼ全部解放されている**（確保元の追跡で残存1件）ため、Rust側の生存データではなく、allocatorの断片化かSQLite/CRT側の確保が疑わしい状態です。原因は未特定。

どちらも今回は直していません（次の作業を参照）。`external_entries`は`revert`/replayのために必要な台帳で、走査を減らす変更はreplay契約に触れるため、別の設計確認が必要です。

## 未実施・未確認

- Linux、他のmachine、GUI（native）、HTTP、PostgreSQL、`record`の閲覧cache。GC pause時間の上限・常時計測は追加していません。
- NUM-02（kernel・solver）、SCHED-01（応答性）、IO-01（stream）、REC-01の定量測定はこの記録に含みません。
- バイナリサイズ・起動時間の比較は取得していません（起動は約25 ms、差なし）。

## 次の作業（候補）

1. **external台帳の走査（PERF-05）**：予算確認で全journal segmentを毎回走査しない。追加専用の台帳の合計をキャッシュし、spill時だけ全走査にする案。replay契約とsegmentの重複計上を確認してから。
2. **`db.next`のnative側の増加**：SQLite/CRT側かallocator断片化かを、`next`を呼ぶ最小のRust単体で切り分ける。
3. **VM命令あたりの費用**：現在も空loopで約6 µs/反復。残りは`enforce_budget`の全状態走査が中心（globals・heap・stack・frameを毎回集計）。増分化には予算の失敗原子性の設計確認が必要。
4. **`valuesFloat`**：要素ごとのメタデータ計算を、型付きscalar listではまとめて行う。
