# REWIND v1.9.22

## live の GUI 入力待機

`std.guiWindows.nextEventAnyLiveAsync()` を追加する。返り値は `Task<Result<WindowEvent,GuiError>>`。application Task の `external live` 内で作成し、region 外で await する。`gui,tasks,external,live` の許可が必要で、stage / publish と同様に background Task から GUI 入力を操作できない。

通常の `nextEventAnyAsync` は記録・replay を維持する。live 待機は OS / fixture の新しいイベントを取得するが、保持された既存 Task の checkpoint を復元すれば、その Task が取得済みのイベント・終端エラーを再利用する。新規 factory は次のイベントを消費する。`begin` に戻って新規 factory を実行しても OS の入力を巻き戻さない。

recorded / live を合わせて active any-window waiter は一つ。競合は `GuiWaitBusy`。公開済み window がない場合は `GuiWindowNotPublished`、閉じた backend は `GuiClosed`、fixture 終端は `GuiWindowEventTapeEnd`。キャンセル後は次の入力を消費しない。live Task のキャンセル前 checkpoint を復元した場合、キャンセル済み private receipt を `GuiCancelled` として返す。元のキャンセルした Task の await は従来どおり `TaskError::Cancelled`。

イベント取得は main thread の nonblocking poll とし、待機中は他の VM / HTTP / DB Task を進める。ready な計算 Task がない idle の間だけ既存 scheduler の待機を使う。既定の協調実行なので、未分割の長い native kernel の実行中まで preempt する保証はない。

## 寿命と記録

live の factory はイベント取得前に最大 64 KiB と metadata の予算を予約する。空 poll の tape は作らない。タスクと checkpoint が v1.9.21 の lease を共有し、最後の保持先が消えると結果を解放する。ウィンドウ自体の寿命は publish / close と別であり、入力 Task の解放では閉じない。

GUI effect と必要な live effect は callback / module / API に伝播する。std package の effect metadata も更新する。既存 GUI 関数の effect と permission は維持し、古い使い方へ live 権限を追加要求しない。API baseline は新規関数の追加のみで互換性を確認する。完全な record / replay / inspect の拒否は v1.9.21 と同じで、CLI は物理操作や publish より前に拒否する。

source-free の fixture 例を `gui-live` に同梱する。Windows / Linux SDK では、実 native window、HTTP の pending request、SQLite の parameterized INSERT を組み合わせ、実クリックによるキャンセル、VM revert 後の DB 状態、接続 cleanup、入力・外部操作の観測保持量を確認する。同じ SDK で recorded 方式の source-free / disconnected replay も検証する。

HTTP server / TCP、残る GUI の loading / retry / error の統合、長い kernel の協調分割、一般 container の保持費用・capacity は継続する。v2.0 完了とは扱わない。
