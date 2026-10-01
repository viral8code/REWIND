# REWIND v0.9.4 草案 — 言語とライブラリの抜けを埋める

作成日: 2026-10-01。状態: **草案・未実装**。基準は[v0.9.3実装状況](v0.9.3-status.md)。言語の表現力、実用的なlibrary、実行の費用を揃える。既存のgeneric/trait/closure/async、効果/ownership、Checkpoint/replay、SDKを再実装項目にしない。

## 1. 他言語から見た不足

特定言語の互換実装を目指さず、開発者が実装を組み立てるための機能を比較する。

| 比較の観点 | REWINDの土台 | 残る不足と採用方針 |
|---|---|---|
| Rust・Swiftの型と安全な抽象化 | generic、trait/default/associated type、enum、Option/Result、borrow/move/Frozen | 複数bound/where、期待型によるfactory推論、module alias経由の明示型引数、opaqueなcollectionの表現。借用戻り値は必要なlifetime用例を限定して設計 |
| C/C++の数値・データ配置 | checked Int64、Float64、Bytes、List/Map、64-bit bit API | UInt64/Int128またはBigIntの用途別選択、fixed array/bitset、slice/view、contiguousなbuffer。FFIはcapabilityとreplay不能区間を設計してから別段階 |
| Java・C#の標準libraryと配布 | SDK、API docs、署名/lock、application entry、JSON/config/args | 標準Error型、日時/Duration、decimal、CSV、path/storage、package upgrade、cross-platform配布。class hierarchyの追加を前提にしない |
| Python・JavaScriptの文字列とデータ処理 | UTF-8、scalar/byte index、number codec、collections、JSON | incremental decoder/tokenizer、regexまたは限定pattern、Unicode正規化/grapheme、streaming CSV/JSON。便利さのために型/失敗/予算を暗黙化しない |
| Go・RustのI/Oと並行処理 | effect付きasync/task/channel/cancellation、観測journal | bounded byte input/output adapter、backpressure、deadline/Duration、構造化shutdown。network/process adapterは権限・timeout・publish契約を先に定義 |
| 関数型言語の合成可能な抽象化 | higher-order pure callback、effect parameter、enum/match、immutable record | fallible comparator/monoid、iteratorの借用期間、標準Result combinator/typed Error、recursive immutable dataのShare推論の検証 |

v0.9.3で、selected importの明示generic factoryは使えるが、`module.create<T>(...)` と期待戻り型による推論は揃っていない。sort等はShare要素とpure比較callbackを利用する。既存trait機構があることと、複数boundを自然に書けることを区別する。

## 2. P0: 実行時の費用と型の基盤

### memory・collection

v0.9.3のListとheapはpersistent paged treeで、root複製はO(1)、indexed access/updateは木のpathと最大64slotのコピーを伴う。Bytesのcloneも構造共有する。この補修をやり直さず、次の残る費用を検証する。

- primitive MapはBTreeMapだが更新はpayload複製を伴う。user Ord Mapは線形な比較/挿入である。persistent ordered tree、必要ならhash mapの別型を追加し、orderとhash seedの契約を分ける。
- heapのallocationは現在実行終了まで保持される。到達不能ownerの回収を、Checkpoint・branch・task・closure・観測/inspectionのrootを考慮して導入する。resourceのcloseとは分ける。
- historyMemoryはlogical payload中心であり、Arc/tree node/allocator overheadやcompiler/nativeの全allocationを正確に表さない。physical resident budgetとnative workの課金を追加し、logical history budgetを維持する。
- snapshot共有pageの物理計測、profileへのclone/allocation/native-work指標、n/2n/4nのregressionを追加する。壁時計だけでperformanceを保証しない。
- mutable構造の公開fieldを利用者が直接壊せる。module内private fieldまたはopaque typeを設計し、constructor/操作関数を不変条件の境界にする。

nativeなsort/graphを増やす前に、storageとbudgetの費用を説明できることを優先する。既存executionSteps/task-steps/historyMemoryの指定を重複して作らない。

### generic・error・API契約

1. 複数bound/where、module aliasの明示generic call、期待型からのgeneric factory推論を追加する。既存alias visibility/capture/cacheの回帰を維持する。
2. builtin Option/Result、user recursive immutable enum、Frozen element、borrowed generic fieldの型/Share/Sendを実例で検証する。missing variantの型をruntimeの存在値から復元する方式へ戻さない。
3. `AlgorithmError {code, position?, operation}`等の標準errorを決める。現在のString errorとStdErrorをadapterで移行し、元入力やSecretを含めない。Result propagationとCLI診断の原因を繋げる。
4. fallible comparator/monoidを別APIにする。callbackの途中失敗がmutable構造を壊さないようpreflight/new owner/Checkpoint方式を選び、transaction保証の範囲を明記する。
5. std API snapshotのbreaking diffに型・効果・所有権・capacity/order/budgetの変更を含める。公開generic関数を使ったcompile例をSDK組立てに含める。

## 3. P0: bounded streamingとよく使うデータ型

v0.9.3のscannerは最大1 MiBのBytesをcursorで読む。appはIn.readLineを繰り返せるが、stdinのbounded byte chunk、tokenがchunkをまたぐ場合の継続、buffered writerはまだない。

- `Reader`/`Writer` adapterをpureなparser/bufferから分離する。effectを持つ関数が同梱されているだけで、pure module利用者へinput等の権限を要求しないpackage/到達性の契約を検証する。
- readChunk、EOF、max token/byte、incremental UTF-8 decoder、signed integer scanner、writer flushを実装する。observationは実際に読み取ったchunk順序へ固定し、Checkpointでcursorを戻してもHostを再読込しない。
- borrowed slice/read-only viewを先に設計し、部分文字列やBytesの読み出しのために全体を複製しない。borrowed returnを導入する場合はescaping capture/task/Checkpointの禁止条件を明示する。
- fixed-size array、bitset、checked UInt/wide integerの必要な演算を揃える。BigIntは桁/演算work予算を必須にする。decimalはscale/丸めを持つ別型とする。

初回はbounded byte I/O、chunk境界をまたぐUTF-8/整数とwriter、Map費用改善、memoryの観測、generic/error基盤までを完了単位とする。下記P1/P2は独立した小さな追加単位として管理する。

## 4. P1: 主要なアルゴリズムの残り

v0.9.3でsort/search/sequence、heap/deque/DSU、数論の基本形、Fenwick/segment tree、BFS/iterative DFS/Dijkstraを実装した。次はその組合せを利用する。

| module | 処理 | 先に固定する契約 |
|---|---|---|
| range | lazy segment tree、sparse table、rollback DSU | monoid/actionの法則、update/query区間、失敗時の状態 |
| graph | topological sort、Bellman-Ford、SCC、MST、LCA | negative cycle、未到達、tie order、distance overflow |
| string | KMP/Z、trie、rolling hash、suffix array/LCP | byte/scalarの選択、hash collisionとexact matchingの区別 |
| matrix/dp | product/power、LIS、knapsack、bitset DP | dimension、memory、generic arithmetic、overflow |
| advanced graph | max flow/min-cost flow、matching | residual update、iteration budget、解の検証 |
| polynomial/geometry | NTT/convolution、条件付きFFT、整数幾何 | modulus/root/size、丸め誤差、cross product overflow |

全項目をv0.9.4初回へ詰め込まず、基本依存→参照実装→property test→費用測定→公開APIの順に進める。大きなmodulusの逆元は現在のextendedGcdの中間Int64範囲を超える場合があり、wide arithmetic/API整備と一緒に検証する。

## 5. P1/P2: 通常のアプリと配布

CSV quoting/newlineとstreaming、明示record JSON codec、Instant/Duration/Calendar、structured logとretention、typed command schemaとhelpの拡充、atomicな単一文書storeを整備する。既存JSON/config、bool/短いoption、path validation、File競合検査を未実装扱いにしない。

storeはprocess lock、schema version/migration、crash recovery、power-loss durabilityを追加する。複数file/streamのpublishを一括atomicにする保証は別に設計する。network/DB/process/UIはcapability、timeout、idempotency、journalへの秘密情報、不可逆なcommitの境界を決めたadapter packageとする。

SDKはRust toolchainのpin、API互換proposal、明示std upgradeとrollback、archiveのtimestamp/permission正規化、source distributionの再ビルド、Linux aarch64/Windows/macOSの実行試験を追加する。公式署名鍵/rotationと公開先を用意するrelease操作は実装・テストから分ける。Rust embedding/FFIとruntime-only binaryはhost境界が固まった後に扱う。

## 6. 受入試験と開発手順

- expected type/generic/borrow/alias/recursive typeを正常例と拒否例で検証し、source-free artifactとcache invalidationを確認する。
- Mapとheap回収をn/2n/4n、checkpoint有無、old rootへの復元で測る。allocation失敗とnative work超過はpartial mutationを残さない。
- streamingを1-byte chunk、UTF-8分割、符号/数字/EOF境界、token超過、短いwrite、cancel、replayで検証する。
- typed errorのcode/location/causeをsourceからCLIまで照合し、Secret/入力値を伏せる。
- アルゴリズムは小さい参照実装との照合、invariant/property、capacity/overflow、Checkpointで検証する。
- std snapshot、SDK署名/導入/例/改変検知、既存language modeをCIで維持する。

作業は`codex/develop`で継続し、mainへの統合はreview可能な単位にする。版ごとの実装状況と草案をdocsに残し、ブランチを版ごとに増やさない。
