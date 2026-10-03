# REWIND v1.9.9

## 名前付きネイティブウィンドウ

`std.guiWindows` は `std.gui.View` を複数の Win32 / X11 surface へ表示する。`present(id,&view)` と `close(id)` は未公開処理をステージし、`publish;` で確定する。ID は空でなく、128 UTF-8 bytes 以下、NUL を含まない。同じ ID の present は既存画面を更新する。close を publish した後に同じ ID を present すると新たな画面を作る。

| API | 契約 |
| --- | --- |
| present(id,&view) | Result<Unit,GuiError>。scene 検証・容量検査の後にステージ |
| close(id) | Result<Unit,GuiError>。公開済みまたはステージ済み ID を終了。未知の ID は GuiWindowNotPublished |
| poll(id) / nextEvent(id) | 指定画面の Option<Event> / Event。poll は待たず、nextEvent は待つ |
| pollAny() / nextEventAny() | Option<WindowEvent> / WindowEvent。window ID と gui.Event を返す |
| continueInput() | revert 後、その実行で配信済みの名前付き入力だけを読み飛ばす |

native の pollAny は各画面を順番に検査し、最後に入力を返した画面の次から再開する。一つの画面の close は他の画面を終了させない。旧 gui.present / gui.nextEvent の単一画面と入力 cursor は独立しており、既存コードは変更不要。

## 巻き戻しと外部境界

View、未公開 scene、入力 cursor は checkpoint の対象。公開済みの画面・閉じた画面・配信済み観測は外部側に保持する。revert begin だけでは画面を消さない。Undo はモデルの revert と continueInput の後、必要な画面を present / publish する。公開済み操作 ID は checkpoint 復元時にも再送しない。

各画面への適用が成功した時点で操作 ID を確定する。後続画面や stdout が失敗した場合、成功分は残り、通常の PublishPartiallyApplied に適用済み画面を含める。publish の失敗は実行の終了状態であり、失敗後に再試行できる API を追加したものではない。

## 記録と費用

名前付き画面は最大16、canvas の合計は旧単一画面を含めて16,777,216 pixels。各 scene は既存 gui の寸法・widget・text 契約を守り、最大1 MiB。公開前に画面数・pixels と描画用容量を検査し、準備に失敗した場合はその publish で新たな画面を表示しない。公開済み画面の保持容量も履歴予算へ算入する。scene の JSON 生成や native rendering は費用を持ち、保持予算と process RSS は同一ではない。

観測と未消費 fixture の容量は増分集計し、入力ごとに全観測を再走査しない。入力を native / fixture から取り出す前に最大イベントの記録分を検査する。予算不足で未配信イベントを消費しない。観測は最大100万件で resident に保持し、GUI 入力を disk spill したとは扱わない。poll の空結果、要求 ID、待機 / poll の区別も記録する。

`--gui-window-events FILE.json` は `{ "window":"counter", "event":{...} }` の配列を読む。fixture は最大1 MiBで、指定すると native 画面を開かない。scoped poll は先頭イベントが別の ID なら None を返し、イベントは残す。scoped nextEvent で先頭 ID が違う場合は GuiEventTapeEnd。複数 ID の fixture には pollAny / nextEventAny を使う。

source-free replay は fixture・display・native host を使わない。新しい観測欄は旧 trace では空として扱い、要求 ID や poll / wait の相違、観測不足は ReplayMismatch とする。gui effect の明示許可は compile / run / replay で必要。

## 検証と継続項目

実 OS surface 二つへの scoped / any 入力振り分け、片方の close 後の継続、再オープンと host 解放、16画面の事前検査、revert / published operation、入力の予算拒否、stdout の後続失敗を検証する。SDK の gui-windows 例は source-free artifact、debug / compact replay、fixture を削除した再実行まで確認する。

GUI は main task で実行する。nextEvent / nextEventAny の待機中は VM task を進めないため、task を使うアプリケーションでは pollAny と yield を使う。clipboard、dialog、メニュー、表、IME / accessibility、native kernel の公平性、残る数値・通信の統合条件は継続する。v1.9 全工程をこの版で完了とはしない。
