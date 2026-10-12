# ネイティブ GUI（REWIND 2.0.0）

`std.gui` は一つのネイティブウィンドウにラベル、ボタン、チェックボックス、色付き矩形を描画します。REWIND のイベントループでクリックとキー入力に応答できます。Windows x64 は Win32/GDI、Linux x86_64 は X11 を使います。ブラウザ、JVM、外部 GUI toolkit は不要です。

現在の機能を順に読むには[HTMLの本のGUI章](book/index.html#ch18) / [PDF](book/REWIND-2.0.0-book.pdf)を利用できます。以下には各版で追加した機能の説明も含みます。

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

1.3 は固定座標の単一 canvas です。resize イベントは通知しますが自動レイアウトは行わず、次の present は View の寸法を適用します。2.0.0ではclipboard、メニュー、file dialog、アクセシビリティの提供範囲を追加済みです。macOS/native Wayland は配布対象ではありません。各機能のOS条件と保証範囲はHTMLの本と後述の契約を参照してください。

## 1.4 の文字編集と配置

`textBox` は一行、`textArea` は複数行の文字編集を行います。`dispatch` が focus と確定文字の挿入を処理し、`Action::Key(id,"TextChanged")` を通知します。`edit` を直接呼ぶ場合は Result で上限・選択のエラーを扱えます。選択は `selection` / `setSelection`、Unicode scalar 単位です。Shift+矢印、Home/End、Backspace/Delete、Ctrl+A、複数行の上下移動に対応します。IME の preedit は OS の状態であり、確定した文字だけが `Event("text",...,key,...)` として VM に入ります。OS の IME service と fonts が必要です。

`resize` で canvas 寸法を変え、`arrange(view,ids,Rect(...),columns,gap)` で grid を配置できます。寸法と ID をすべて検査してから配置するので、エラー時は元の配置を保持します。scroll は focus した textarea を wheel で動かし、編集時は caret の行を表示します。長い行は caret が見えるよう横方向の表示を調整します。

`pollEvent` は入力がなければ `Ok(None)` を返します。短い計算の間に入力を処理できます。入力がない結果も journal に残るため replay は同じ polling の順序で実行します。fixture では `kind:"idle"` を使います。無制限な busy loop は避け、実行・入力記録の予算を指定してください。協調 task の自動進行を、この API だけで保証しません。

SDK の `share/rewind/examples/notes` は編集、Save、Undo all、resize のサンプルです。`--allow-effects gui,fileRead,fileWrite` で利用できます。Save は publish してファイルを確定し、Undo は表示モデルだけを戻します。Undo 後にも確定済みファイルは残ります。record/replay、fixture、コンパイル済み配布にも対応します。

GUI の `gui.graphemeEditing(&mut view, true)` は結合文字・絵文字を cluster 単位で編集する。既定値は false。選択位置は scalar offset のままで、有効化時に境界へ切り上げ、有効化後は cluster 内部の `setSelection` を拒否する。モードも checkpoint の対象になる。[v1.9.7](REWIND_v1.9.7.md) と SDK の gui-grapheme 例を参照。

## 1.9.9 の複数ウィンドウ

`import std.guiWindows as windows;` を加え、`windows.present("counter",&view); publish;` で ID ごとに表示する。`windows.pollAny()` / `windows.nextEventAny()` は window と event を持つ WindowEvent を返す。イベントを対応する View の gui.dispatch へ渡す。`windows.close("counter"); publish;` はその画面だけを終了する。

入力と画面は旧 gui の単一画面から独立する。名前付き画面の Undo は `revert saved; windows.continueInput();` として復元した View を再表示する。待機 API は VM task を進めないため、外部 I/O の完了確認との併用は pollAny と task.isDone() を使う。現在は `std.task.yieldNow` と `windows.nextEventAnyAsync` を利用できます。

最大16画面、合計16,777,216 client pixels（旧単一画面を含む）、各 scene 1 MiB。ID、入力 fixture、記録・予算・部分公開の詳細は [v1.9.9](REWIND_v1.9.9.md)、実行例は [gui-windows](../examples/gui-windows/README.md)。`--gui-window-events` を使う fixture と replay は native 画面を開かない。

## 1.9.10 の task handoff

通信や計算 task を持つ GUI loop では、pollAny で入力を確認し、task.isDone() で完了を確認して `std.task.yieldNow()` で VM 処理を渡す。tasks effect が必要。nextEventAny の待機中に VM task が進む変更ではない。dot / matmul の協調 API は [v1.9.10](REWIND_v1.9.10.md)。

## 再利用部品のID scope

`gui.componentScope(name)`は1〜24 UTF-8 byteの名前から`ComponentScope`を作る。`gui.componentId(&scope, local)`はlocal IDを1〜64 byteに制限し、scope名のbyte長を含むprefixで子widget IDを一意に符号化する。同じscope/localの組は同じIDになり、異なる組は衝突しない。`gui.componentPrefix(&scope)`は`guiForm.render` / `dispatch`に渡せる最大27 byteのprefixを返す。

scopeはID名前空間だけを持ち、modelやnative handleは保持しない。componentの状態は呼出側が通常のVM値として管理し、入力eventを一度dispatchして返ったIDを対応するmodelへ振り分ける。画面内でscope名を重複させると同一部品のIDになるため、各instanceには別名を付ける。利用例は[gui-component-scope](../examples/gui-component-scope.rw)。

フォームと表

`std.guiForm.createWithRules`は最大128個の宣言的なrelationを検査する。`Equal(a,b)`は同じscalar kindのtyped valueの一致、`IntLessEqual(a,b)`は整数field間の大小、`RequiredWhenChecked(target,checkbox)`はcheckboxがtrueのときtargetを必須にする。参照fieldとkindは作成時に検証する。relation validationはpureで、invalid textは上書きせず、`validate`と`toJson`の両方がrelation errorを返す。editで関係fieldのerrorを無効化して次のvalidateまで古い診断を残さない。例は[gui-form-relations](../examples/gui-form-relations.rw)。

## フォームと表

1.9.12 の `std.guiForm` / `std.guiTable` はモデルと View の構築を提供する。モデルを変更したら render して present / publish する。入力・型検証・容量・viewport の契約は [v1.9.12](REWIND_v1.9.12.md)、動く例は [gui-controls](../examples/gui-controls/README.md) を参照する。GUI table編集例は [gui-table-edits.rw](../examples/gui-table-edits.rw) を参照する。

`guiForm.toValue` first validates the form (including declared relations), then passes its typed JSON object to one pure mapper supplied by the application. The mapper can construct an application record explicitly. `SubmitError` keeps form validation errors separate from mapper errors, and the mapper is not called when form validation fails. See [gui-form-typed.rw](../examples/gui-form-typed.rw).

`std.guiTable.createWithIds` accepts unique caller-owned row IDs (1..64 UTF-8 bytes). `rowId`, `rowIndex`, `selectId`, and `selectionId` map between those IDs and the current data positions; rendered row buttons carry the row ID, so dispatch resolves identity through the table index. The legacy `create` overload synthesizes `row:<index>` IDs for compatibility; those are positional and do not survive external reordering. `editCell` and `editCellId` stage cell changes in a bounded overlay keyed by row ID and column. `cell` and rendering read staged values first; `changes` returns a frozen snapshot in first-edit order, while `clearEdits` discards the overlay. The model retains frozen base cells and caps edits at 4096 cells, 4 MiB of staged value text, and 4096 bytes per cell. `sortByText` performs a stable whole-table lexical sort using staged cell values when present. Pass `descending=false` for ascending order and `true` for descending. It supports at most 8192 rows and keys up to 256 UTF-8 bytes. Sorting only changes row-order indexes: row IDs, selection, staged edits, and frozen base cells remain attached to the same row. `rowIndex`, `selection`, and the `Action::Selected` index report the current display position. `filterByText` applies a case-sensitive substring match to one column; an empty query clears the filter. `rows` remains the total source count, while `visibleRows` reports the current view. `rowIndex` returns `None` for hidden rows; selecting a hidden ID returns `GuiTableFilteredRow`. Hiding the selected row clears selection. Stable-ID edits can target hidden rows, and edits to the active filter column immediately refresh membership. A non-empty query scans at most 8192 source rows and is limited to 256 UTF-8 bytes. `setColumnWeights` accepts one positive integer weight per column (up to 65,535); each render distributes the available inner width proportionally, keeps every column at least one pixel wide, and uses the entire width without gaps. The default weight is 1 for every column. See [gui-table-sort.rw](../examples/gui-table-sort.rw), [gui-table-filter.rw](../examples/gui-table-filter.rw), and [gui-table-column-weights.rw](../examples/gui-table-column-weights.rw). Native text-input/event wiring remains separate work.

## Native clipboard (selected language 1.9.52)

Enabled, focused textboxes and textareas support Ctrl+C / Ctrl+V / Ctrl+X.
Copy uses the selection in the published scene. Paste is ordinary recorded text
input and the view can be restored with revert. OS clipboard state remains
external; replay never reads or writes it. Use continueInput after reverting if
you want subsequent input rather than previous observations.

Windows uses Unicode clipboard text and Linux uses X11 CLIPBOARD UTF8_STRING.
Only valid, NUL-free text of at most 4096 UTF-8 bytes is accepted. X11 transfers
are asynchronous with a two-second reply deadline; unsupported formats and
failed transfers cause no edit. X11 copy ownership expires when the window
closes. See [v1.9.52](REWIND_v1.9.52.md) and the shipped gui-clipboard example.

Windowsの未確定入力はIMM対応のOSサービスを利用します。システムの入力contextを
借用する場合と自前で作る場合の寿命を区別し、公開済み入力欄の文字位置問い合わせに
回答します。確定文字だけがVMの記録へ入ります。実Microsoft日本語IMEの
preedit / convert / commit、focus取消、終了の受入と検証範囲は
[v1.9.65](REWIND_v1.9.65.md)を参照してください。SDK起動はOSの言語設定を変更しません。
