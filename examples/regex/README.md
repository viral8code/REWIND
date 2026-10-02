# 正規表現

`rewind run main.rw`。一度コンパイルした Regex を共有して使い、named captures・次のマッチ・checkpoint・Map key を確認します。Span / cursor は byte offset です。Text は Unicode、Bytes のパターンは raw byte mode が既定です。backreference / lookaround は非対応で Syntax error、regex は自動的に外部処理を起動しません。
