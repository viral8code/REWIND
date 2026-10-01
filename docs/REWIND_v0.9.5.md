# REWIND v0.9.5 草案 — 実用性と実行基盤の継続整備

作成日: 2026-10-01。状態: **初回実装済み・継続項目あり**。範囲は[v0.9.5実装状況](v0.9.5-status.md)。基準は[v0.9.4実装状況](v0.9.4-status.md)。v0.9.4計画の未完了項目と今回の実装で確認した制約を集約する。作業branchは`codex/develop`。既に使える機構を再実装の課題にしない。

## 1. 実装済みの前提

単一ファイルcompile/run、rewindc、source-free .rwc、compiler内蔵std、増分publish、primitive Mapのpersistent AVL storage、module alias経由の明示generic call、bounded stdin byte chunk、pureなtoken/UTF-8/writerはv0.9.4で利用可能。List/heapのpaged storage、generic/trait/closure/async、effect/ownership、record/replay、署名付きSDK、既存26moduleとそのアルゴリズムを維持する。

## 2. P0: memory・work・操作の失敗境界

### heapの回収と物理費用

- 到達不能heap objectを回収する。globals、frame、closure、task/channel、Frozen、Checkpoint、branch、debuggerをrootとして扱う。現在は実行終了までallocationが残り、短命のparser一時Listも蓄積する。closeするresourceとheap回収を分ける。
- historyMemoryはlogical payload中心。Arc/node/allocator、snapshot共有、compiler/nativeの一時allocationを含む物理費用を別指標・予算として設計する。今回追加したMap node生成数とpaged copy数は傾向を測る指標であり、resident byte数ではない。
- 確定操作IDのledgerはpublish後も保持する。既存Checkpointが参照できるIDを残す方式で圧縮・回収し、未公開eventの同一性を失わない。現在のledger課金は保守的概算で、snapshotのmetadataすべてを数える方式ではない。
- native sort/codec/map traversal/parse等の入力依存workをexecution budgetに統合する。言語のstep数だけではnative処理全体を制限できない。超過前の拒否または失敗時復元を必須にする。
- n/2n/4n、Checkpoint有無、旧root復元、owner消滅、task終了でnode/page/heap数を比較する。OSの壁時計だけで保証しない。

### publishと仮想Hostの精査

- file/directoryの操作ID付与と容量検査を一つのtransactionにし、ledger課金失敗時にpending deltaを残さない。publish直前の予算検査はv0.9.4で追加したが、各操作のmetadata予約はまだ完全ではない。
- stdout/stderrの途中失敗は実行全体の再publishを拒否する。file staging、directory作成、rename/deleteの途中失敗も含め、どこまで反映されたかを構造化して診断する。複数file/streamの一括atomic性を約束しない。
- virtual publishのfile baselineは保持できるが、directory treeのmove/delete/走査をHostと完全分離して検証する。inspect/debugで実Hostを変更せず、通常実行と同じ論理結果を得る。
- publish前の古いfile deltaへrevertした場合、外部状態との競合として拒否する現規則を維持し、意図的なrebase/forceの要否を実例で設計する。
- resumeの制御位置、branch/task/channel、File handle、連続publish、cancel、file競合、短いwriteを組み合わせた回帰を増やす。confirmed ledgerはCheckpointに戻さない。

## 3. P0: 型・不変条件・エラー

- 複数generic bound/whereと期待戻り型からのfactory推論。module.create<T>は実装済みなので対象から除く。alias visibility・specialization cache・borrowの拒否規則を維持する。
- module private fieldまたはopaque type。deque/graph/stream等の内部Listやcursorをcallerが直接壊せる現状を解消し、constructor/操作関数を不変条件の境界にする。
- recursive immutable enum、Frozen element、borrowed generic fieldのShare/Sendを正常例・拒否例で検証する。欠けたOption/Result variantをruntimeの現在値から推論しない。
- 標準Errorのcode/position/operation/causeを決め、String error、StdError、StreamErrorをadapterで段階移行する。Secretや入力値を診断に混ぜない。CLIまで原因を保つ。
- fallible comparator/monoidを別APIで提供し、callbackの途中失敗でmutable collectionを壊さない方式を固定する。現在のpure callbackは失敗のtransaction保証を意味しない。
- API diffに型・effect・ownershipに加え、capacity/order/失敗・費用契約の変化を反映する。

## 4. P0/P1: streamingの残りとデータ型

`In.readChunk`は1..65,536 byte、一回の短いreadを許す。現状の観測は実行中のメモリへ蓄積し、履歴予算で停止する。無制限streamではない。

- chunk観測のspill/retention、入力全体の絶対offset、Reader/Writer adapter、backpressure/cancelとtask境界。line/chunk混在の観測順序、EOF再読込、異なるchunk要求、replay時の検証を固定する。
- UTF-8 errorはcarryを含む当該入力の位置、token errorはchunkまたはtoken内の位置。絶対位置へのadapterを追加する。容量・形式エラーのpreflight保証とruntime budget失敗を区別し、必要な操作をatomicにする。
- byte slice/read-only view、固定配列、bitset。escaping capture/task/Checkpointを含む借用期間を定義し、既存Bytesの構造共有cloneとsliceの費用を取り違えない。
- UInt64、wide integer/BigIntを用途から選択する。checked演算、桁/work上限、codec、bit操作を揃える。decimalはscaleと丸めを持つ別型にする。
- CSV quoting/newline、streaming JSONとtyped record codec、Unicode正規化/grapheme、限定patternまたはregex、Instant/Duration/Calendar。仕様と上限を先に固定する。

## 5. P1: 主要アルゴリズムの継続

既存のsort/search/sequence、heap/deque/DSU、整数/剰余、Fenwick/segment、BFS/DFS/Dijkstraは再実装しない。

| 順序 | 追加処理 | 契約・受入条件 |
|---|---|---|
| A | topological sort、Bellman-Ford、SCC、MST、LCA | negative cycle、未到達、tie order、overflow、参照解との照合 |
| A | lazy segment tree、sparse table、rollback DSU | monoid/action法則、半開区間、失敗時の復元、Checkpoint |
| A | KMP/Z、trie、rolling hash、suffix array/LCP | byte/scalar、衝突とexact matchの区別、空入力 |
| B | matrix product/power、LIS、knapsack、bitset DP | dimension、容量、checked arithmetic、work予算 |
| B | max flow/min-cost flow、matching | residual invariant、iteration上限、解の検証 |
| C | NTT/convolution、条件付きFFT、整数幾何 | modulus/root/size、誤差、wide cross product |

大きなmodulusの逆元でextendedGcdの中間Int64がoverflowする場合をwide arithmeticと一緒に改善する。user Ord Mapは線形storageのままなので、persistent ordered treeを別途実装する。HashMapはhash/order/seedの再現性契約を先に決める。

## 6. P1/P2: アプリと配布

- structured log/retention、typed command schema/help、単一文書storeのprocess lock、schema migration、crash recovery、durability。既存args/config/JSON/path/File競合を未実装扱いにしない。
- network/DB/process/UIはcapability、timeout、idempotency、Secretと不可逆確定の境界を定義したadapter packageにする。
- `run`の成果物再利用範囲、CLI helpの全option、manifest初期設定の具体的な診断、std upgrade/rollback。現在はparse/module cacheを使い、runが前回の.rwcを必ず再利用する実装ではない。
- compiler内蔵stdは同一binaryに含まれるsourceを照合してcacheを修復する。外部dependencyは従来の署名/lock方式。SDKの公式署名鍵とrotation、archiveのtimestamp/permission正規化、Rust toolchain pin、source archive再ビルド、Linux aarch64/Windows/macOSの実行試験を揃える。
- Rust embedding/FFI、runtime専用binaryはHost境界とABIの契約を決めてから実装する。rewindcは現在同じ処理系を起動するentryである。

## 7. 実装順と完了条件

初回は2・3を優先し、streamingの失敗・retentionを揃えた後に5のAを進める。B/Cや全platformの完成を初回に含めない。各変更を実装状況文書に記録し、未着手の項目を完了扱いにしない。

受入試験は旧language mode、単一ファイル/manifest付きproject、権限拒否、source-free artifact、record/replay、checkpoint復元、失敗atomic性、n/2n/4n費用、std/API/SDK改変検知を継続する。algorithmは小さな参照実装・invariant/propertyで検証する。publishのledgerやheap回収は正常例だけでなく、予算超過・途中失敗・古いsnapshot参照を必須にする。
