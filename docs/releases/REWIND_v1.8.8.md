# REWIND v1.8.8 — 増分 JSON event

`std.jsonStream` を追加する。既存 `std.json.parse` / Json DOM は変更しない。`reader`、`feed`、`next`、`finish`、`cancel`、`position` は pure な Result API。巨大な配列を一つの List として構築せず、構文 event ごとに処理できる。

## Event

`Event` は標準 enum `JsonStreamEvent` の alias。次の variant を返す。

| variant | payload |
| --- | --- |
| ObjectStart / ObjectEnd / ArrayStart / ArrayEnd | position:Int |
| Key / Text | decoded text:String, position:Int |
| Number | original token:String, position:Int |
| Bool | value:Bool, position:Int |
| Null | position:Int |

position は stream 全体の 0-based byte offset。string / key は開き quote、number / literal は最初の byte、container は該当の括弧を指す。Number は JSON 数値文法を検査した元の文字列を保ち、Float に変換しない。Int / Float / BigInt / Decimal の変換と range / rounding は呼出し側で明示する。大きい integer と exponent も token 上限内で保持できる。

UTF-8、escape、Unicode surrogate pair、number、true / false / null は任意の chunk boundary をまたげる。comma / colon、container の対応、末尾 comma、trailing data、未完 token を検査する。入力は一つの JSON document で、前後の JSON whitespace を許可する。object key の重複は順番通りに event として返す。重複を拒否する既存 DOM parser と同じ政策だとは扱わず、利用側が key の選択・重複検査を行う。

## 終了と失敗

feed は成功時だけ Reader を更新する。queue limit 等の失敗で当該 chunk は消費せず、既存 pending / queue を保持する。drain 後に同じ chunk を再送し、一つの chunk の event 数が上限を超える場合は小さく分割する。

next は一つの event を移譲する。None は現在の queue が空で、EOF ではない。finish は末尾の number を確定し、文法・token が完結したことを確認して閉じる。閉じた後も drain できる。cancel は pending / queue を捨てて閉じる。

event は prefix が解析できた時点で返す。後続の入力が不正なら document 全体は不正となるため、取り出した event を全体の検証成功と混同しない。既に行った外部作用を parser error が取り消すことはない。

error offset は absolute byte offset。文法は問題の byte、EOF は入力終端、number / decoded string の検査は token の開始位置。chunk / token / depth / queue / closed / position / incomplete / syntax を区別する。

## 上限と storage

chunk は 65,536 bytes、raw token は quote / escape を含め 1 MiB、container depth は 64、queue は 256 events / payload と metadata の admission 計 4 MiB まで。parser は完了・取得済み token や全 document を保持しない。pending は CSV と共通の immutable 4 KiB page、queue は Arc、grammar stack は depth 上限内で共有 state からコピーする。

feed の scan、末尾 page / grammar / queue metadata、token 変換、event 出力の native work / scratch を実行前に検査する。fatal budget を typed parser error に隠さない。Runtime の累積予算は別に適用され、大きい処理は設定が必要。

Reader は checkpoint、freeze / thaw、task、record / replay に対応する。serialization は pending token と文法・queue を保存し、復元でサイズ・個数・position・文法状態の整合性を検査する。checkpoint が参照する storage は保持する。

これは parser の保持契約である。Runtime の観測 journal / trace や `File.openSnapshot` の全 file snapshot が解放されることを意味しない。begin からの再実行を保つための観測 spill と bounded file reader は v1.9 の memory / I/O 監査で扱い、全体の RAM が bounded と主張しない。

## 検証・配布

全 split point の UTF-8 / escape / surrogate / number、文法・EOF・atomic error、数値 token の精度保持、queue / token / depth / wire 上限、drain する大きい配列、pending checkpoint と task、source-free replay を検証する。実 HTTP download の chunk を JSON / CSV に渡し、source と server を除いた replay も検証する。

SDK は公開 50 modules と tests、API baseline、json-stream sample を収録する。両 OS の検証成功後に main 統合・Release 公開する。v1.8 の標準型・text / parser 範囲を揃え、v1.9 の GC・費用・GUI・言語監査と v2.0 の統合条件は継続する。
