# 次版の性能・メモリ・長時間運転の測定計画

状態：未実装・未測定。対象版はv2.3候補です。[全体計画](ROADMAP_v2-next.md)と[v2.1のprofile計画](REWIND_v2.1-plan.md)に従います。性能値を測る前に、最適化対象や改善率を確定しません。

## 判断基準

同じ値・観測・公開結果・数値精度・取消契約を保ったまま、仕事量、不要コピー、peak memory、待機時間を減らします。ユーザーが残したcheckpointの到達可能値は保持対象です。checkpointを消して速く見せたり、native workの会計を省略したりしません。

まず既存benchmarkを再利用します。追加scriptは既存caseで不足する境界だけに絞ります。計画の段階ではbenchmarkも公開CIも実行しません。

## PERF-01：比較できる測定記録

各測定に以下を保存します。

| 項目 | 必須情報 |
|---|---|
| 実装 | baseline/candidate commit、binary hash、release/debug、compilerとbuild option |
| 環境 | OS、CPU、RAM、実行制限、同時負荷、backend version |
| 入力 | seed、形状、件数、byte数、chunk/batch、fixture hash |
| 実行設定 | language、steps/native/history/memory予算、profile、記録mode、checkpoint数と保持方針 |
| 正しさ | 期待出力、digest、誤差基準、物理操作数、終了状態 |
| 費用 | wall time、VM steps、native work、peak RSS、ledger、GC回数/時間、結果サイズ |
| 未取得 | 取れないcounterと理由。測定していない値をゼロにしない |

同じmachineでbaseline/candidateを交互に実行します。既定はwarm-up 3回以上と測定30回以上とし、median、p95、min/max、件数を記録します。長時間caseは回数を減らして理由と全測定値を残します。30件で推定したp95を厳密な尾部保証とは扱いません。

起動を含むCLI時間と、処理区間だけの時間を分けます。native input作成、kernel、結果変換、record生成、replayを分離したcaseを用意します。profileのon/offも比較し、観測による費用を本体の回帰と混同しません。compact/debug/no-recordの結果を一つの数字へ混ぜません。

Windowsのworking set/private bytesとLinuxのRSSは定義を併記します。予約address space、allocatorの保持、到達可能live bytes、Host resource、record fileのサイズを区別します。OS間の数値を無条件に直接比較しません。

**受入条件：** 第三者が入力・binary・設定から同じcaseを再実行できる。改善報告にはbaselineと正しさ確認があり、測定不能項目は明記される。

## PERF-02：既存caseと不足する測定

| case ID | 調べる内容 | 既存の出発点 |
|---|---|---|
| MEM-01 | List/Mapの保持、更新、slice、freeze、iteratorとCOW | `benchmark-container-admission.py`、`benchmark-gc-pressure.py` |
| MEM-02 | Bytes/Textの共有、slice、一時変換、出力 | `benchmark-bytes-admission.py`、`benchmark-text-storage.py` |
| NUM-01 | native入力view/コピー、配列index/scan、page更新 | `benchmark-numeric-input.py`、`benchmark-numeric-index.py`、`benchmark-numeric-pages.py` |
| NUM-02 | kernel、一時領域、戻り値変換、取消 | `benchmark-numeric-memory.py`、`benchmark-least-squares-cancellation.py` |
| NUM-03 | AD/training graph保持、iteration後の解放 | `benchmark-graph-large.py`、`benchmark-model-training.py` |
| SCHED-01 | heavy numericと他task/inputの応答性 | `benchmark-numeric-cooperation.py`、`benchmark-eigen-cooperation.py` |
| GUI-01 | View構成、部分更新、eventからpublishまで | `benchmark-gui-model.py` |
| IO-01 | stream HTTP、CSV/JSON chunk、DB cursor/batch | `benchmark-http-clients.py`とstream smoke。DB用比較caseは追加候補 |
| REC-01 | debug/compact記録、index、replay、閲覧cache | `benchmark-recording.py` |

scriptはrepositoryの`scripts/`以下です。表は既存scriptの全内容が各目標をすでに検証しているという意味ではありません。引数・counter・caseを読み、不足する測定を追加します。

checkpoint条件は「なし」「少数固定」「同数を保持して更新」「dropして再作成」を分けます。「保持数を増やし続ける」は意図した増加量と増加以外の漏れを区別するためのcaseです。beginが保持するrootも含めます。

MEM-01/02では共有したままの更新と所有が独立した更新を比較します。1回の更新で全payloadを複製していないか、iteratorの構築で全件materializeしていないかを調べます。各APIが保証する順序・snapshot・借用寿命は維持します。

NUM-01/02では小入力と大入力を分けます。大入力の速度だけのために、小入力の開始費用やcancel応答性を悪化させない判断をします。誤差基準はcaseごとに絶対/相対誤差、残差、収束条件等を明示します。浮動小数点の演算順序を変える最適化は、互換方針と再現性を先に判断します。

IO-01ではUTF-8の多byte境界、長いfield、空chunk、途中cancel、failure、cleanupを含めます。DBではlocal SQLiteと明示的なPostgreSQL test環境を分け、1行/小batch/大batch、prepared reuse、TLSありの費用を比較します。実DBへの書込みは使い捨てschemaに限定します。fixture/replayだけの結果をphysical DBやnetworkの性能と呼びません。

SCHED-01/GUI-01では処理総量と別taskの応答時間を両方測ります。fake入力の予定eventから処理までのlogical遅延と、native入力取得からpublishまでのwall timeは別項目です。測定のためにscheduler規則を変えません。

## PERF-03：保持領域とGCの調査

主な調査先は`src/lib.rs`、`src/shared_payload.rs`、`src/storage.rs`、`src/map_storage.rs`、`src/text_storage.rs`、`src/v2/vm/scheduler/gc.rs`です。所有権の問題か、共有pageの会計か、弱いcache/indexの掃除か、allocatorの保持かを先に分けます。

1. 同数の到達可能rootを維持し、一定サイズの値を繰り返し作成・破棄する。
2. Task完了/失敗/取消、cursor/window終了、記録session切断を含める。
3. warm-up後のlive ledgerとOS memoryの傾向を測る。
4. 増加が続く場合は保持rootを特定し、checkpoint由来か意図しない強参照かを確認する。
5. 最小再現caseと費用を残してから修正する。

GCのpauseを計測するためのinstrumentationにも有効/無効設定と上限を設けます。v2.1のretainer調査を通常GCに常時組み込みません。参照count導入やcollector全置換を初手にせず、現状の所有権とroot契約で解決できる部分から直します。

**受入条件：** 固定rootの繰返しcaseで、測定誤差・allocator保持を超える未説明の増加がない。checkpoint由来の増加を消す変更がない。GC/cancelで使用中の共有領域を解放しない。

## PERF-04：最適化候補の選別

候補は次の順で調べます。

1. 同じ入力・shape・encodingを重複検査する箇所。
2. native配列とList/Bytesの境界で不要に全コピーする箇所。
3. 一時bufferをoperation/chunkごとに作り直す箇所。
4. GUI modelやdebug viewを毎event全走査する箇所。
5. record・profileの巨大なformatとcacheの無制限保持。

検査削減には「誰がいつ検証したか」を証明する型/内部token等の契約が必要です。単にチェックを省略しません。buffer再利用では上限、cancel後の解放、別taskの隔離、Secret消去、revert時の共有を検証します。部分更新は無効化条件を列挙してから導入します。

各候補は以下を揃えて独立commitにします。

- hot pathと費用の証拠、最小入力と大入力の比較。
- 対象APIの値・effect・所有権・予算・エラーの不変条件。
- 修正前後の正しさと、native work/step計上が妥当である根拠。
- 意味のある改善、または回帰を許容する具体的理由。
- 他backendやprofile/record modeへの影響。

一律の「何倍高速化」や根拠のない固定memory上限を達成条件にしません。最初のbaselineを取得後、対象caseごとの回帰許容幅と目標を記録します。実行時間がノイズ幅以内の変更を成功と数えません。

## PERF-05：長時間運転

GUI event、DB query/cursor、HTTP stream、numeric batch、記録sessionをそれぞれ独立して繰り返し、最後に組合せを確認します。固定checkpoint数で少なくとも10,000 cycleのローカルcaseを初期候補にし、時間制限と処理量を記録します。backend負荷によって回数を変えた場合は理由を残します。

注目するのは、VM live bytesだけでなく、file descriptor/handle、worker、pending request、native connection、cursor、window、cacheの残存です。各cycleの成功・失敗・cancelを混ぜ、shutdown後にcleanupの完了を確認します。外部serverの資源保持はVMのGCと別に観測します。

**受入条件：** 終了した資源の累積増加が説明なく残らない。unknown結果をretryして負荷を増幅しない。設定上限到達時に明確な診断とcleanupを行い、次の処理が不正stateを利用しない。

## 版の判断と記録

PERF-01 → PERF-02 → 問題が確認されたPERF-03/04 → PERF-05の順です。新しいlibraryを追加するだけではこの段階の完了にしません。結果はcaseごとの「測定済」「改善済」「差なし」「保留」と、再現command・binary・入力への参照で記録します。

大規模な表/数値libraryの追加は別判断です。既存stream、numeric、stats、AD等で書ける処理を重複実装せず、不足操作と具体的な例を確認してから設計します。

公開CI/CD停止中はローカル検証のみです。測定や修正を今実行する依頼ではなく、将来の実装判断に使う文書です。
