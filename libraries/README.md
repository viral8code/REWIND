# REWIND libraries

v0.9.1 の最初の標準ライブラリ。compiler の collection storage、ownership、効果、JSON syntax を土台にし、開発で繰り返し使う処理を **REWIND の source module** として提供する。ライセンスは repository の MIT。compiler 0.9.1 / `language = "0.9.1"` を使う。

| module | 公開 API | 契約 |
|---|---|---|
| std/json.rw | parse, parseBytes, stringify, get, keys, kind, integer, floating, text, boolean, array | 不変 Json AST、Result/Option、pure |
| std/config.rw | Field, ConfigError, resolve | args > environment > file > fallback、必須/型/未知項目/重複 schema、値を含まない失敗 |
| std/args.rw | ArgError, parse | `--name VALUE`、明示した option、未知/重複/欠落拒否、256 token/128 name |
| std/collections.rw | filter, map, any, all | `&List<T>` の同期読取り、Share 要素、pure Send+Share callback、map/filter は新しい List |
| std/math.rw | min, max, clamp | Int、範囲逆転は Result::Err、overflow を伴う演算を増やさない |
| std/option.rw | valueOr, map | Share 値、None の既定値、pure callback |

## 利用

現在は選択した `.rw` を project の `lib/` へコピーし、`import lib.json as json;` のように明示的に読み込む。SDK の自動探索・global auto-import・公式署名済み std package はまだ実装していない。project へ固定した source は build の source digest と source-free artifact に含まれる。`examples/v091/lib` は JSON/config/args の同じ source の実行例であり、回帰テストで両方の内容を照合する。

```rewind
import lib.json as json;
match json.parse("{\"count\":3}") {
    Ok(value) => {assert_eq(json.get(value,"count"),Some(Json::Int(3)));},
    Err(error) => {assert_eq(error.code,"Syntax");}
}
```

## 検証

```sh
rewind update --root libraries/std
rewind test --root libraries/std
rewind doc libraries/std/json.rw --root libraries/std --output /tmp/rewind-json-api.md
rewind api-snapshot --root libraries/std --output /tmp/rewind-std-api.json
```

`tests.rw` はアルゴリズム、空 collection、Option、設定優先順位、未知項目、引数重複の契約を実行する。Host I/O はライブラリへ隠さず、アプリで Args/Env/File の許可・観測・publish を明示する。callback を別 task へ spawn しない。

SDK / runtime image の配布構成、stdlib の版と API 互換性、文字列・codec・path・日時・decimal・CSV などの次のライブラリは [v0.9.2 草案](../docs/REWIND_v0.9.2.md) にまとめる。
