# REWIND 3.4.0

言語構文は3.0.0以降と同じです。3.4.0は標準ライブラリに7つのpure moduleを加えた版で、既存APIは変えていません。

## 追加

- **`std.url`**（NET-01）：URLのparse・format・resolve（RFC 3986）、queryの順序保持parse/format、percent encoding。成分はencodedのまま保持し、decodeは一度だけ。`display`はuserinfoを隠し、errorは入力を含みません。
- **`std.issues`**（LANG-06）：複数の検証問題を上限付き（1..256）で集める。`Validated<T>`は問題が無いときだけ`Valid`。`std.schema`のerrorを取り込めます。
- **`std.units`**（LANG-03）：`Quantity<Second>`・`Quantity<Milli>`、`Id<UserTag>`など、型検査で混ざらない単位と不透明ID。拡大変換は桁あふれを検査します。
- **`std.httpHeaders`**（NET-02）：header名と値の検査、繰返しheaderの保持、Cookie headerのparse、Set-Cookieの属性検査、`redacted`（Authorization・Cookie等の値を`***`に）。
- **`std.retry`**（NET-03）：明示retry policyの判定のみ（待機も送信もしない）。`Unknown`（送信後の結果不明）は冪等と宣言したときだけ再試行します。
- **`std.proptest`**（QUAL-01）：seed付きの決定的なproperty検査と縮小（整数list）。
- **`std.logfmt`**（SVC-03）：1 entry 1行の構造化ログ、credentialらしいfieldの`***`化、上限付きqueue（満杯なら新しい行を捨てて件数を報告）。

## 未完了

各planの「実装済み」「未実施」を参照してください（[service構想](../archive/v2-next-services.md)・[言語構想](../archive/v2-next-language.md)・[品質構想](../archive/v2-next-ecosystem.md)）。
