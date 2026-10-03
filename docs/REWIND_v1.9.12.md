# REWIND v1.9.12

## フォームと表の標準ライブラリ

`std.guiForm` は入力 schema と編集モデル、全項目の検証、型付き Json、表示範囲だけの View 構築を提供する。`std.guiTable` は frozen の行優先データを共有し、見えている行だけを View にする。どちらも pure な VM 内の処理であり、画面への反映は既存の `gui.present` / `guiWindows.present` と `publish` で行う。モデル、値、cursor / anchor、選択行、表示範囲は checkpoint の対象になる。公開済み画面・配信済み入力は従来の外部領域に属する。

### guiForm

`Field(id, caption, kind, required, maxBytes, initial)` を借用 List で `create` に渡す。Kind は Text / Multiline / Integer(low, high) / Real(low, high) / Check / Timestamp。1..64 項目、1..32 表示行、値は 1..4096 UTF-8 bytes を上限に指定する。ID は1..48 bytes、一意、NUL不可。Check は `"true"` / `"false"`、maxBytes は5以上。数値範囲は順序の正しい有限値である必要がある。schema の caption / 初期値も NUL 不可、4096 bytes 以下。

`setValue` と編集は文字列を保持する。数値の `"-"`、範囲外、空欄を入力途中で拒否しない。`validate` は全項目を検証し `error` で取得できる code を保存する。`toJson` は読み取り専用で、最初の不正項目を StdError の offset（項目 index）で返す。Integer は signed64、Real は有限 binary64。数値 / Timestamp の前後の空白は除く。optional の空欄は Json::Null、Text / Multiline は空文字、Check は Bool。required の文字列は非空、Check は true を要求する。Timestamp は `std.datetime` の parser と UTC 表示を使う。入力値をエラーに含めない。

`render(form, title, width, height, rowHeight, prefix)` は新しい owned View を返す。幅240..4096、高さ144..4096、rowHeight64..128、`16 + visibleRows * rowHeight <= height - 48` が必要。prefix は1..32 bytes、NUL不可。caption / editor / error と Submit / Reset を配置する。grapheme 編集、scalar cursor / anchor、textarea の表示行、項目の focus をモデルから復元する。

`dispatch` は gui.Event を受け取り Edited / Submitted(Json) / Invalid / Scrolled / Resize / Close を返す。Submit は全項目を検証してから Json を作る。バイト上限を超えた編集は元の値と選択・表示行へ戻し Err を返す。Tab の focus はモデルに反映する。textarea に focus があれば wheel はその本文、それ以外はフォームの表示範囲を動かす。`reset` は初期値と表示範囲・focus・検証結果を戻す。表示範囲変更や dispatch の後は render し直して publish する。画面とモデルを別々に編集した場合は、モデルを更新して再構築する。

schema の構築・validate / toJson は全項目を走査する。render は表示項目数とその文字数に比例する。List / Map の既存永続 storage と GC を使い、native resource を保持しない。

### guiTable

`create(headers: Frozen<List<String>>, cells: Frozen<List<String>>, visibleRows)` を使う。cells は行優先の平坦な List。1..16 列、0..65536 行、1048576 cells 以下、1..64 表示行。各文字列4096 bytes 以下、NUL不可、総文字数16 MiB以下。列数で割り切れない cells は拒否する。create の検証は全データを走査し、保存時には frozen storage を共有する。

`cell` / rows / columns / position / selection、`select` / `scroll` を提供する。scroll は Int 全範囲の差分を加算前に clamp し、overflow しない。select は範囲を検査して必要なら表示範囲を動かす。render は見えている行と等幅の列だけを作る。幅128..4096、高さ96..4096、rowHeight16..128、`16 + (visibleRows + 1) * rowHeight <= height` が必要。prefix の規則はフォームと同じ。行をクリック・Enter で選択し、Up / Down / Home / End と wheel で移動できる。Resize / Close は呼出側へ渡す。モデルを変更したら render し直す。

データの行 index と Unicode scalar の文字位置を区別する。日本語の prefix も扱う。render の費用は表示行×列数と表示文字数に比例し、全行の widget を生成しない。scene は既存2048 widget / 1 MiB制限に従い、長い caption で上限を超えた場合は present まで進めず Err とする。

## 既存 View の編集

`std.gui` に focus / focused / setChecked / remove / style / scrollPosition / setScrollPosition を追加する。未知の ID・不正な色・不正な textarea 行位置は事前に拒否する。remove は残りの widget の順序・選択・scroll・focus を維持して index を再構築する。削除された control の focus は解除する。focused は無効化された control を返さない。textarea の wheel は極端な signed delta でも overflow しない。

## 配布と継続項目

SDK に guiForm / guiTable、API snapshot、フォームと表を別々の window に描画する `gui-controls` 例を含める。クリック、確定文字入力、送信、行選択、Escape の revert、fixture を削除した compact source-free replay を検証する。pure model の失敗・型変換・選択保持と native surface の present / close を別々に確認する。

clipboard / dialog / menu、IME composition / accessibility、文字 shaping と pointer による caret 位置指定、GUI と HTTP / DB の長時間統合、協調的な追加数値 kernel と v2.0 の到達条件は継続する。この版でそれらを完了扱いにしない。

import した module 内の `alias.Enum::Variant` pattern を module 固有の alias へ修飾する。呼出側が同じ alias を import しなくても nested pattern が正しく型検査・実行される。

import alias と同名の local String でも byteLen / charLen 等の primitive method を型検査できるようにする。local binding は module alias より優先する。
