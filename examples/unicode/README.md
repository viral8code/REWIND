# Unicode

`rewind run main.rw`。Extended grapheme cluster（結合文字・国旗・ZWJ emoji）、NFC / NFD / NFKC、Unicode の大小変換と checkpoint を試します。slice の引数は cluster index、offsets は UTF-8 byte index です。正規化は利用者が明示して呼び出し、String や既存の scalar indexing を暗黙に変更しません。
