# 履歴・変更操作libraryの実装記録

更新日：2026-10-11。状態：**source実装済み・実行検証省略**。ユーザーの方針により、環境起動失敗を待たずに実装を進める。compile、test、性能、API生成を確認済みとは扱わない。

既存[std.snapshot / std.history](v2-next-snapshot-implementation.md)から、[永続collection構想](visions/19-persistent-collections.md)と[履歴graph構想](visions/48-history-graphs.md)のdata操作へ進める。全VM checkpointや外部操作を扱うものではない。

## std.timeline：bounded Undo/Redo

初期値とcapacityを受け、現在位置とsnapshotの列を保持する。capacityは現在版を含めて1〜65,536。current、get、seek、undo、redo、length、position、canUndo、canRedo、push、pushSnapshot、clearを提供する。

undo/redo/seekは選択位置を動かしてsnapshotを返す。呼出側modelの更新はsnapshot.fork等で明示する。実行位置、外部ledger、global変数を自動的に戻さない。

pushはcapture後にRedo側を破棄して新しい版を末尾へ置く。fullなら最古版を一つ外す。取得済みsnapshotには影響しない。pushSnapshotは既存rootを保持してcaptureの再実行を避ける。clearは現在版を一件だけ残す。

保存領域はMapのring indexを使う。前方の全要素を移し替えずにevictする。Map操作の費用は既存実装に従う。Redoをk件捨てるpushはk件の登録解除、captureは既存freezeの費用を持つ。fatal budget failureのatomic recoveryは追加しない。

indexは最古の保持版を0とした位置で、evict後は変わる。永続identityや他timelineへのhandleとして使わない。Timeline自身は通常modelで、commit/revertの対象になる。

### 確認用source

libraries/std/tests.rwにring周回、容量1、undo/redo境界、Redo破棄、保持済みroot、clear、不正capacity/indexを用意した。実行は省略。

## 保存・検証方針

codex/developだけへ小さくcommit/pushする。commitには[skip ci]を付ける。公開CI/CD、main統合、tag、Release、版更新を行わない。

将来の実行確認はcargo run -- test --root libraries/stdと、追加example、API snapshot生成から始める。実行を省略した事実を、検証済みや全構想完了へ読み替えない。
