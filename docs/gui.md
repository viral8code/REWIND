# ネイティブ GUI（REWIND 1.4.0）

`std.gui` は一つのネイティブウィンドウにラベル、ボタン、チェックボックス、色付き矩形を描画します。REWIND のイベントループでクリックとキー入力に応答できます。Windows x64 は Win32/GDI、Linux x86_64 は X11 を使います。ブラウザ、JVM、外部 GUI toolkit は不要です。

## 実行

SDK の `share/rewind/examples/gui/main.rw` を作業用フォルダにコピーし、次を実行します。

```sh
rewind run main.rw --allow-effects gui
```

「Add」で数字が増え、「Enable counting」で加算を有効・無効にします。「Undo all」で初期表示へ戻ります。Tab でフォーカスを移し、Enter / Space でボタンやチェックボックスを操作できます。閉じると最終値を標準出力へ確定します。

コンパイル済みコードでも使えます。

```sh
rewindc main.rw --allow-effects gui
rewind main.rwc --allow-effects gui
```

Linux では `DISPLAY`、24-bit 以上の X11 display、`libX11.so.6` と core font `fixed` が必要です。Debian/Ubuntu では `libx11-6`、`xfonts-base` が該当します。Wayland の場合は Xwayland を利用します。通常の CLI プログラムは X11 をロードしません。Windows は OS に含まれる DLL を利用し、追加ランタイムは不要です。表示・キーの文字対応は OS とインストール済みフォントに依存します。

## API と状態

`import std.gui as gui;` で読み込みます。

| API | 動作 |
| --- | --- |
| `window(title,width,height)` | `Result<View,GuiError>`。ウィンドウモデルを作るだけで画面は開かない |
| `label`, `button`, `checkbox`, `rectangle` | `&mut View` に ID、内容、ピクセル座標・寸法を指定して追加 |
| `setText`, `text`, `checked`, `setEnabled`, `background` | 内容・選択状態・有効状態・背景色を変更または取得 |
| `scene(&view)` | 検証用の JSON scene を生成 |
| `present(&view)` | 描画内容をステージ。**続く `publish;` で表示** |
| `nextEvent()` | `Result<Event,GuiError>`。公開済みウィンドウの入力を待つ |
| `dispatch(&mut view,event)` | `Option<Action>`。ヒット判定、フォーカス、チェック切替 |
| `continueInput()` | 巻き戻し後、その実行ですでに配信された入力を読み飛ばす |
| `close()` | 終了をステージ。続く `publish;` で閉じる |

`Event` は `kind,x,y,key,width,height` の immutable record です。kind は `pointer`（左クリック）、`key`、`resize`、`close`。主要な key は `Tab,Enter,Space,Escape,Backspace,Left,Right,Up,Down`、または文字です。Action は `Activate(id)`, `Changed(id,checked)`, `Key(id,key)`, `Resize(width,height)`, `Close` です。

View と widget は通常の REWIND データであり、`commit` / `revert` の対象です。ステージ済み scene も戻ります。一度 publish した画面はそのまま残るため、Undo の後には復元した View を `present` し、もう一度 publish します。すでに公開済みの描画を再度 publish しても古い描画は再送されません。

イベントの観測記録は巻き戻りませんが、読み取り cursor は戻ります。対話的な Undo では `revert checkpoint; gui.continueInput();` とします。これがない場合は、同じ入力を再度受け取ります。continueInput は replay 中にも、すでに配信したイベントだけを読み飛ばし、未来の入力は飛ばしません。

`revert begin;` は View 自体も消す完全な作業状態のリセットです。GUI 全体を初期化する場合は、その後に View を作り直します。公開済み画面を消すには `gui.close(); publish;` が必要です。

## 決定的なテストと記録

サンプルの `events.json` は Add×2、Undo、Add、Close の順です。最終出力は `1` です。

```sh
rewind run main.rw --allow-effects gui --gui-events events.json --record trace.json
rewind replay trace.json --root . --allow-effects gui
```

`--gui-events` を指定するとネイティブウィンドウを開かず、指定した JSON 配列を入力として利用します。Replay もウィンドウを開きません。GUI 許可は別途必要です。イベントが不足すると `GuiEventTapeEnd`、改変や不足のある replay は `ReplayMismatch` になります。

## 制限と失敗

ウィンドウの canvas は各辺 64..4096 pixels、最大 2048 widgets、ID は一意で 128 UTF-8 bytes 以下、text は 4096 bytes 以下、scene は最大 1 MiB（JSON 側の予算も適用）。widget は canvas の内側に収めます。text は widget の矩形でクリップします。重なった操作可能 widget は後から追加したものが優先されます。色は `0xRRGGBB` です。イベントは最大 100 万件、fixture は最大 1 MiB。履歴予算も適用されます。

GUI 操作は main task に限ります。`gui` effect が必要な処理を async 関数へ置くと検査で拒否されます。初回 publish 前の入力は `GuiNotPublished`、表示環境がない場合は publish が `GuiUnavailable` になります。公開中に OS の描画が失敗した場合、通常の publish と同じく部分適用として扱われる可能性があります。X11 server の切断のようなプロセス外の障害も巻き戻せません。

1.3 は固定座標の単一 canvas です。resize イベントは通知しますが自動レイアウトは行わず、次の present は View の寸法を適用します。clipboard、メニュー、file dialog、複数ウィンドウ、アクセシビリティ連携、macOS/native Wayland は未対応です。

## 1.4 の文字編集と配置

`textBox` は一行、`textArea` は複数行の文字編集を行います。`dispatch` が focus と確定文字の挿入を処理し、`Action::Key(id,"TextChanged")` を通知します。`edit` を直接呼ぶ場合は Result で上限・選択のエラーを扱えます。選択は `selection` / `setSelection`、Unicode scalar 単位です。Shift+矢印、Home/End、Backspace/Delete、Ctrl+A、複数行の上下移動に対応します。IME の preedit は OS の状態であり、確定した文字だけが `Event("text",...,key,...)` として VM に入ります。OS の IME service と fonts が必要です。

`resize` で canvas 寸法を変え、`arrange(view,ids,Rect(...),columns,gap)` で grid を配置できます。寸法と ID をすべて検査してから配置するので、エラー時は元の配置を保持します。scroll は focus した textarea を wheel で動かし、編集時は caret の行を表示します。長い行は caret が見えるよう横方向の表示を調整します。

`pollEvent` は入力がなければ `Ok(None)` を返します。短い計算の間に入力を処理できます。入力がない結果も journal に残るため replay は同じ polling の順序で実行します。fixture では `kind:"idle"` を使います。無制限な busy loop は避け、実行・入力記録の予算を指定してください。協調 task の自動進行を、この API だけで保証しません。

SDK の `share/rewind/examples/notes` は編集、Save、Undo all、resize のサンプルです。`--allow-effects gui,fileRead,fileWrite` で利用できます。Save は publish してファイルを確定し、Undo は表示モデルだけを戻します。Undo 後にも確定済みファイルは残ります。record/replay、fixture、コンパイル済み配布にも対応します。

GUI の `gui.graphemeEditing(&mut view, true)` は結合文字・絵文字を cluster 単位で編集する。既定値は false。選択位置は scalar offset のままで、有効化時に境界へ切り上げ、有効化後は cluster 内部の `setSelection` を拒否する。モードも checkpoint の対象になる。[v1.9.7](REWIND_v1.9.7.md) と SDK の gui-grapheme 例を参照。
