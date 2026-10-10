# 追加構想：ヒープ外メモリ・arena・native連携

状態：採用判断前。版番号は未割当です。[全体計画](ROADMAP_v2-next.md)に追加する8候補です。JavaのProject PanamaにあるFFMのMemorySegment/Arenaや、Rustの所有権・slice等を参考にします。JVM互換を実装する案ではありません。

## 最初に区別すること

「ヒープ外」は保存場所の区分であり、巻き戻し可否の区分ではありません。現行REWINDのnumeric配列やBytesにもnative側のpayload・共有領域があります。新機能を単なる「native allocationの追加」としません。

| 領域 | 所有者・寿命 | checkpoint/revert | 書込み・費用 |
|---|---|---|---|
| VM管理のnative領域 | VM rootと共有owner | immutable版やCOWを保持できる | VM値として費用を計上 |
| 一時scratch | 実行中operation/task | checkpointから参照させない | 上限内で再利用、終了時回収 |
| 外部mutable segment | 明示arena/physical owner | 内容・close済み寿命を復元しない | external境界、borrow/lease、容量計上 |
| 外部read-only領域 | physical ownerまたは取得済みsnapshot | read-onlyであるだけでは再現性を保証しない | 観測/copy/freezeの方針を明示 |

OSが管理するmemoryやnative libraryのpointerを取得しても、REWINDの予算やcleanup責任から除外しません。物理addressはrecord/artifactへ保存しません。

## MEM-01：MemorySegmentとArena

候補はaffineなArena ownerと、範囲検査付きsegment viewです。API名と構文は未決定です。allocation、slice、read/write、closeを個別の契約にします。

初期はtask-confined arenaを優先します。別taskへ渡す場合はowner transferか、同期規則を持つ共有arenaの別型にします。裸addressをIntとして扱うpublic APIは初期範囲に含めません。

arenaの状態はopen/closing/closed、segmentにはowner identityと世代を持たせる案です。physical registryのclose済み世代はrevertで戻しません。borrow中のcloseを型で拒否できる範囲を定義し、runtime検査も残します。native call中はleaseを保持し、close要求があっても利用中bufferを解放しません。

**受入：** use-after-close、二重close、範囲外、size overflow、borrow逃避、別taskからの不正利用を拒否する。revert beginでもclose済みarenaが復活しない。cleanup終了時にnative memoryの会計が一致する。

## MEM-02：layout・alignment・endian

FFI/binary protocol用に、整数/浮動小数点、配列、struct layoutを明示する案です。REWINDの通常structの配置をそのままC ABIとして約束しません。

byte offset、alignment、padding、endianness、pointer width、platform ABIを記録します。packed layoutのunaligned accessは安全なcopyまたは明示的な拒否を選びます。sizeof計算にchecked算術を使います。unionやbitfieldは初期範囲を限定します。

**受入：** 独立したnative testのsizeof/offsetと照合する。Windows/LinuxのABI差を明示する。異なるlayoutのsegmentアクセスと未初期化値の読取りを検出する。

## MEM-03：明示freeze/copyによる境界越え

外部mutable segmentから、checkpointで保持できるBytes/native配列のsnapshotへ変換する案です。snapshotの取得は外部観測として扱い、得た値の保持はVM内にします。

逆方向はVM値を外部bufferへcopyするか、契約付きのread-only loanにします。zero-copyという名前のために、native側からmutable共有pageを書き換えられるようにしません。容量、copy量、work、Secret/redaction、取得中の他writerを規定します。

**受入：** snapshot後にexternal bufferを書き換えても保存値が変わらない。snapshotをrevertで読んでもnative読取りを再実行しない。loan中のVM更新はCOW等で隔離する。

## MEM-04：pinningとnative callの借用

GCが動くかどうかだけでなく、共有storageの版・寿命・連続性を固定するloan候補です。既存page-backed配列は、必ずしも一つの連続C bufferではありません。

非連続viewはcopyするか拒否するかを明示します。read-onlyとexclusive mutable loanを分け、初期mutable loanは外部segmentに限定する案を優先します。VM値のmutable loanを許す場合は、終了時の新しい版の生成とcheckpoint隔離を別設計にします。

**受入：** GC/取消/close中もnative利用中bufferが有効。revertで過去の値を変更しない。loanをcaptureしてscope/task外へ逃がせない。隠れた全copyの有無をprofileに出す。

## MEM-05：mmapと大きなfileのview

read-only/private/shared mappingの違いを調べ、まずread-only mappingから検討します。ただし別processによるfile変更、truncate、OS固有のfaultを無視しません。read-only mappingを不変snapshotと呼びません。

再現性が必要な場合はbounded copy、独立snapshot、content verification等の方式を選びます。shared writeはphysical操作であり、flushやdurabilityとVM publishを一括atomic操作にはしません。予約address spaceとresident/committed memoryを分けて計上方針を決めます。

**受入：** close/取消/途中file変更を扱う。untrusted fileのfaultでVM process全体を落とさない実装方針を確認する。安全に成立しなければstream読取りを優先して保留する。

## MEM-06：bounded scratch arena

numeric/codec/parserの一時bufferを使い回す内部最適化候補です。ユーザー向けの任意object arenaとは分けます。既存scratch予約と実測を先に調べます。

operationごとに容量を予約し、chunk後の再利用、task隔離、最大保持容量、trim、Secret消去を決めます。cancel/失敗後に未初期化bufferを別taskへ渡しません。scratchから返却値へ移すときにownerと会計を移管します。

**受入：** PERF計画のcaseでallocation削減を確認する。固定rootの長時間運転で保持容量が際限なく増えない。既存work/取消応答性を保つ。

## MEM-07：外部memory pressureと資源会計

segment、mmap、native library内部buffer、device memoryに種類別counterを持たせる案です。既存`src/shared_payload.rs`の弱いledger、numeric storage、Host resource会計を調べて統合します。

external allocationの前に予約し、allocation失敗で予約を戻します。close後もnative callが利用中なら、実際に解放するまで費用を残します。共有segmentをviewごとに重複課金しません。native library内部の不透明な使用量は推定・上限・未計測を区別します。

**受入：** 部分allocation失敗、close待ち、同時operation、task取消でledgerが過少にならない。heap/live/physical/reservedの数字を別々に説明できる。

## MEM-08：foreign function呼出しの具体化

EXT-01のadapter契約を、layout/arena/loanを使う最小FFIへ具体化する案です。初期は明示登録した固定signatureのC ABI関数と、構造化したerror変換を検討します。任意symbol探索や任意pointer演算を既定にしません。

effectは純粋計算、観測、不可逆操作を区別します。純粋計算と宣言するにはhidden global state、time/random、file/network、memory side effectを検査します。callbackは初期対象外とし、必要ならVM threadへの配送、reentrancy、capture、取消を独立設計にします。

同期native callは、Taskをcancelしただけで実行が停止するとは限りません。戻るまでleaseを保持し、deadlineはVM側の待機終了とphysical完了を分けます。上限や協調停止を提供できないlibraryは、通常の有限予算実行へ無条件に組み込みません。

**受入：** byte codec等の小さな例でABI・所有権・cleanup・会計を確認する。外部例外/panicと不正pointerをVM errorで安全に扱える範囲を明示する。process crashをResultで捕捉できると約束しない。

## 実装を選ぶ順序

MEM-06/07の既存基盤調査 → MEM-01/02 → MEM-03/04 → MEM-08。MEM-05は独立したOS安全性調査が必要です。

VM内の高速化だけが目的なら、既存native配列・COW・bounded scratchで足りる可能性があります。arenaを導入する理由は、外部libraryとの連携や巨大bufferの明示寿命を安全に扱うことです。GCの全面置換や、すべての値を手動freeにする仕様にはしません。
