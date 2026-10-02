# 日時とタイムゾーン

`rewind run main.rw`。整数ナノ秒、DST の重複時刻、タイムゾーン変換、checkpoint と Map key を扱います。変換は pure です。実時計を読む場合だけ `std.clock.now()` を external region で呼び出し、`external,clock` effects を許可します。
