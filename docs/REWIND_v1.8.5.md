# REWIND v1.8.5 — Unicode

`std.unicode` を追加する。既存の String equality、Map key、scalar indexing と `std.text` は変更しない。正規化と単位の選択は明示的に行う。GUI の既存 cursor は scalar 単位のままで、grapheme 対応の編集と cursor 移行は v1.9 で扱う。

## API と単位

- `graphemeCount` / `graphemeSlice` / `graphemes` は UAX #29 の extended grapheme cluster。CRLF、結合文字、国旗、ZWJ emoji、Indic conjunct を扱う。slice は 0-based cluster index、start inclusive / end exclusive。cluster の途中で切断しない。
- `graphemeOffsets` は各 cluster の UTF-8 byte offset と終端を返す。空 String は `[0]`。offset と scalar / cluster index を混同しない。
- `words` は UAX #29 で分割したうち、Alphabetic / Number を含む word のみ。`wordBounds` は空白・句読点等も保持した全 segment、`sentences` は sentence boundary ごとの全 segment。全 segment を連結すると元の String に戻る。
- `normalize(text,Form::NFC/NFD/NFKC/NFKD)` / `isNormalized` は明示的な canonical / compatibility 正規化。compatibility 正規化は見た目の異なる字形や区別も変え得る。String の保存・比較で自動実行しない。
- `lower` / `upper` は full default Unicode casing。lower は Greek final sigma の文脈を扱い、upper は一文字から複数文字への展開を扱う。locale 依存の Turkish 等の特殊方針、case folding、正規化を暗黙に適用しない。
- `versions()` は normalization / segmentation / case の Unicode データ版を返す。normalization 0.1.25 / segmentation 1.13.3 のデータは Unicode 17.0.0。case は固定された Rust toolchain の `char::UNICODE_VERSION` を表示し、将来の更新でも版を確認できるようにする。既存の transitive normalization dependency を downgrade しない。

引数 String は borrow で受ける。pure な API とし `Result<...,StdError>` を返す。結果 String / List を freeze / thaw、task、checkpoint、record / replay で扱える。

## 上限と費用

入力は 1 MiB、変換出力は 8 MiB、分割 List は 65,536 entries まで。offset List は終端も含めて同じ上限とする。`UnicodeInputLimit` / `UnicodeOutputLimit` / `UnicodeItems` / `UnicodeIndex` を区別する。

通常の走査は入力に対する線形 work を、正規化は長い combining sequence の安定ソートも含む保守的な O(n log n) work を実行前に課金する。分割結果の VM object、正規化の展開・並べ替え、String buffer を含め scratch memory も事前に検査する。変換途中で出力上限を超えた時点で打ち切る。公開上限までの全入力が既定の VM 予算で実行できるという意味ではなく、fatal な native work / memory budget も適用する。予算超過を Result に隠さない。

正規化結果は独立した immutable String。checkpoint の文字列を in-place に書き換えず、失敗で元の入力を変更しない。入力サイズを超えることのある展開や、多数の小さい文字列への分割を無課金にしない。

## 検証と配布

結合・分解・compatibility・canonical ordering・Hangul、grapheme boundary、大小展開・final sigma、空・range・入力 / List cap、作業量の事前拒否、List の freeze / task 移動、checkpoint と source-free replay を検証する。SDK に Unicode サンプル、API baseline を収録し、公開 module は 47 個と `tests`。Linux / Windows SDK を CI で検証してから main 統合・Release 公開する。

regex、incremental JSON / CSV は次の patch。v1.8 全体と v1.9 / v2.0 の到達条件は継続して既存計画に従う。
