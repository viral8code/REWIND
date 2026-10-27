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

**SQLite loop（INSERT OR REPLACE → SELECT → `db.next` → `closeCursor`、:memory:、固定16行）**

| cycles | 変更前 wall s / peak commit MiB | 現在 wall s / peak commit MiB |
|---|---|---|
| 250 | 3.7 / 36.5 | 2.3 / 36.7 |
| 500 | 12.7 / 42.9 | 7.0 / 43.3 |
| 1000 | 49.3 / 52.4 | 25.6 / 52.7 |
| 2000（現在のみ） | — | 100.0 / 74.7 |

10,000 cycleは完走させていません（上の傾向では数十分かかる）。原因を切り分けた結果:

1. **時間が回数の約1.6〜2乗**（1000 → 2000回で3.9倍）。`external`操作の結果（journal segment）は`external_entries`として実行中ずっと保持され（`src/external.rs`、上限100万件）、予算確認が全segmentを毎回走査するためです（INSERTだけのloopでも1000 → 2000回で3.2倍）。軽量hashで約1.5〜1.9倍速くなりましたが、O(件数)の走査は残っています。
2. **peak commitが約20 KiB/cycle増える（原因特定・修正済み）**。`db.next`だけで増えたのは、非同期external開始時（`begin_async_external`、`src/external.rs`）に`max_result`分のバッファを`try_reserve_exact`で実確保していたためです。`db.next`の予約は8 MiBで、完了時に`shrink_to_fit`で縮めます。この8 MiBの確保・縮小の繰り返しでWindowsのheapが回ごとに伸びていました（他のdb操作は64 KiB予約なので目立たない）。切り分け: SQLite worker単体（`Command::Execute`/`Query`/`Next`/`CloseCursor`を4,000回、`next`有り・無し）はpeak commit 2.1 MiBで一定だったため、SQLite/CRT側ではありません。予約は`external_memory_bytes`の会計に残し、バッファ自体は結果に応じて伸ばす形にしました（`Vec::new()`）。結果上限は書き込み側の`Bounded`が従来どおり守ります。修正後の`sqlite_cycle`のpeak commitは250 / 500 / 1000回で29.3 / 29.2 / 29.1 MiBの一定（修正前37.1 / 42.9 / 53.5 MiB）。同じ予約の実確保が同期・live経路（`begin_live_external`、`src/external.rs`の2か所）にも残っていますが、今回は測定していません。

**項目1も修正済み**。予算確認は、outcomeが揃った`external_entries`先頭部分の(memory, disk)合計をキャッシュし、新しく完了した分だけ足す。segmentがspillした時（全体のspill世代が変わる）とreplay読込時に作り直し、spillが必要な時だけ従来どおり全segmentを候補にする。他の保持元と共有するsegmentは二重計上しない。台帳自体（revert/replayの内容）は変えていない。修正後の`sqlite_cycle`（項目2の修正込み）は500 / 1000 / 2000回で0.88 / 1.6 / 2.9 s（修正前25.6 s @1000、100.0 s @2000）と回数に比例し、peak commitは29 MiBで一定。10,000回は未測定。

## 未実施・未確認

- Linux、他のmachine、GUI（native）、HTTP、PostgreSQL、`record`の閲覧cache。GC pause時間の上限・常時計測は追加していません。
- NUM-02（kernel・solver）、SCHED-01（応答性）、IO-01（stream）、REC-01の定量測定はこの記録に含みません。
- バイナリサイズ・起動時間の比較は取得していません（起動は約25 ms、差なし）。

## 次の作業（候補）

2. **同期・live経路の予約バッファ**：`src/external.rs`の同じ`try_reserve_exact`を、会計だけ残す形にするか検討する。
3. **VM命令あたりの費用**：現在も空loopで約6 µs/反復。残りは`enforce_budget`の全状態走査が中心（globals・heap・stack・frameを毎回集計）。増分化には予算の失敗原子性の設計確認が必要。
4. **`valuesFloat`**：要素ごとのメタデータ計算を、型付きscalar listではまとめて行う。

### SQLite loopの10,000 cycle（修正2件の後）

`62ceb6b`・`f204620`適用後のbinaryで、同じloopを2,500 / 5,000 / 10,000回: wall 3.5 / 6.9 / 14.2 秒（回数に比例）、peak commit 28.6 / 31.8 / 41.0 MiB（約1.2 KiB/cycle。cycleあたり4件のexternal台帳entryに相当し、台帳を保持する設計どおり）、handle最大164で一定。完走を確認しました。

### 命令あたりの費用が状態サイズでどう変わるか

空ループ10万回（`s+=i`）の時間を、状態を変えて測りました（単発、秒）。

| 状態 | 変更前 | 変更後 |
|---|---|---|
| ローカル変数 0 / 50 / 200 個 | 1.1 / 2.3 / 7.5 | 1.1 / 1.5 / 2.9 |
| 保持checkpoint 0 / 20 / 100 個 | 1.0 / 2.3 / 8.2 | （未変更） |
| heapのList 0 / 1,000 / 10,000 要素 | 1.1 / 1.1 / 1.2 | （heapは集計をキャッシュ済みで影響なし） |

予算確認は全ローカルを毎回なめるため、ローカルが多い関数ほど遅くなっていました。スカラー値（Int/Float/Bool/Null/参照）は数値・共有・byte・containerの補正がすべて0なので、`retained_bytes`で即returnします（結果は同一）。残る費用はcheckpoint数に比例する部分（各checkpointのstateを2周する）で、増分化が必要です。

### checkpoint数に比例する費用

保持checkpoint 100個で空ループ10万回が約8秒（0個で1秒）だった原因は2つでした。(1) VMの1命令ごとに、全snapshotのscheduler保持量（16 MiB上限の確認）を再計算していた。snapshotは保存後に変更されないので、snapshotごとに1回だけ計算して保持。(2) 予算確認が各checkpointのstateについて5つのArcをhash集合へ入れ直していた。直前のcheckpointと同じ参照なら集合を引かない。結果は同一で、100個の場合 4.7 秒 → 2.4 秒（スカラー補正の前は8.2秒）。117本のテストが通過。

## Linux（WSL2 Ubuntu 24.04）での確認

同じmachine上のWSL2（ext4上のclone、`cargo build --release`、rustc 1.98.1）で、変更前（`a56c22f`）と現在のHEADを、既存の`scripts/benchmark-*.py`（`REWIND_STANDALONE_LANGUAGE=auto`）で各5〜7回測りました。中央値、RSSは`wait4`のru_maxrss。WSL2は仮想化されたLinuxで、実機Linuxの値ではありません。

| script / case | 前 s | 後 s | RSS KiB（前→後） |
|---|---|---|---|
| numeric-memory churn（2万更新） | 1.208 | 0.685 | 19296 → 19440 |
| numeric-memory churn_with_checkpoint | 1.356 | 0.704 | 19276 → 19584 |
| container-admission small_history | 0.103 | 0.061 | 16948 → 16788 |
| container-admission larger_history | 0.218 | 0.140 | 21764 → 21768 |
| text-storage short / large / mixed | 0.409 / 0.059 / 0.131 | 0.225 / 0.039 / 0.072 | 14956 / 15360 / 15400 → 15008 / 15276 / 15320 |
| bytes-admission aliases / ephemeral / longer | 0.426 / 0.058 / 0.199 | 0.313 / 0.040 / 0.129 | 98200 / 98312 / 98312 → 98300 / 98412 / 98412 |
| gc-pressure（512 KiB×200） | 0.163 | 0.153 | 21784 → 22024 |
| recording compact | 0.0181 | 0.0128 | 14532 → 14516 |
| recording debug | 0.521 | 0.526 | 174000 → 174020 |

- Windowsと同じく、時間は約1.3〜1.8倍短縮、RSSは同等です。
- **task増加の修正もLinuxで確認**：`await`するtaskを2万・4万・8万回回すと、変更前のRSSは15.4 / 15.6 / 17.0 MiB相当（15412 / 15576 / 16984 KiB）と増え、変更後は14932 / 15020 / 15124 KiBでほぼ一定。時間は1.36 / 2.84 / 5.66 秒 → 0.80 / 1.73 / 3.27 秒。
- `recording debug`だけ変わりません。1000回のloopで0.5秒・174 MB使うのは、debug記録が1命令ごとに情報を残す設計のためで、今回の変更の対象外です（REC-01の課題）。

### `--record-mode debug`の費用（未対応のREC-01課題）

ループ1,000回（約1万命令）で、Windowsのpeak working setは debug 270 MiB・trace 14 MB、compact 14 MiB・trace 1.3 KB。debug記録は1命令ごとに状態の索引を残す設計で、メモリはtraceファイルの約19倍（JSON値の保持）になり、4,000回ほどで索引が16 MiBに達します。以降は索引を捨てて、イベントとチェックポイントだけを記録し続けます（トレースは `debug.index_dropped` を持ち、再生はできますが、ステップ実行の索引はありません）。compactは回数によらず一定です。長く走らせる記録にはcompactを使い、debugは短い再現用に限る、という使い分けが必要です。traceの保持形式（JSON値ではなく直列化済みbyte列）の見直しは別作業です。

## 打ち切りの判断

VM命令あたりの費用は、現在も空ループで約8 µs/反復（変更前の約27 µs）です。sampling profile（約3.8万sample）は、予算確認の全状態集計（約30%）、確保・解放（約20%）、命令dispatch本体（約15%）に分散していて、単独で大きい箇所は残っていません。これ以上の短縮は (a) 予算確認の結果の増分化、(b) 永続heapの一意所有時の破壊的更新、(c) `Op`の文字列のArc化、のいずれかが必要です。(a)(b)は「失敗時に更新前へ戻る」「checkpointと構造共有しても過去の状態を壊さない」「台帳の登録がnode単位」という既存契約に触れ、全テストで検出できない順序依存を作り得るため、今回は入れていません。(c)は確保を数%減らすだけです。

### debug記録のメモリ内訳（調査のみ・未変更）

1,500 eventの時点で、確保元を追跡してlive分を数えると約7.7 MB（約5 KB/event）で、ほぼ全部が(1) eventごとの`serde_json::json!`のObject（`task`・`pc`・`operation`・`source`・`line`・`column`）と、(2) `index_state`が積む差分（`debug_deltas`）でした。保存したJSONは約400 B/eventなので、`Value`の木（BTreeMapのnodeと文字列確保）が約12倍に膨らんでいます。実測のpeak working set（約18 KB/event）は、これに`inspection()`が毎event作る全状態JSONの一時領域とallocatorの保持が加わった値です。

変更しなかった理由：eventは再生時に`replay["events"][i]`と`Value`で比較され、差分は`mask_debug_json`が`Value`木全体でsecretを伏せてから出力されるため、直列化済み文字列で保持するには比較・伏せ字処理・trace組立ての3箇所を作り替える必要があります（`serde_json`の`raw_value`機能で埋め込む案）。16 MiBのindex上限で頭打ちになるdebug専用の経路なので、効果に対して回帰の危険が大きいと判断しました。

