# REWIND libraries

compiler 0.9.2 / `language = "0.9.2"` の標準ライブラリ。collection storage、ownership、型、Checkpoint は処理系が担い、上位の処理は REWIND source module として提供する。ライセンスは MIT。

| module | 公開 API | 契約 |
|---|---|---|
| json | parse, parseBytes, stringify, get, keys, kind, integer, floating, text, boolean, array | 不変 Json AST、pure、1 MiB/深さ64/65,536 node |
| config | Field, ConfigError, resolve | arguments > environment > file > fallback、schema 検証、128 field |
| args | ArgError, parse | `--name VALUE`、未知/重複/欠落拒否、256 token/128 name |
| collections | filter, map, any, all, fold, find, count | borrowed List、Share 要素、同期 pure Send+Share callback、入力順 |
| math | min, max, clamp, mulMod | signed Int、clamp 範囲検査、mulMod は正の modulus と i128 中間積 |
| option | valueOr, map | Share 値、pure callback |
| result | valueOr, map, mapError | Share の成功/失敗値、pure callback、未選択分岐を維持 |
| map | getOr, contains | borrowed Map、Ord key、Share value、key equality は既存 Map と同じ |
| text | length, trim, split, tokens, slice, find | Unicode scalar index、slice は半開区間、split は空要素保持、tokens は ASCII whitespace |
| bytes | length, encode, decode, get, slice | UTF-8 codec、byte index、get は0..255、slice は半開区間 |
| number | integer, decimal, floating, formatInt, formatFloat | signed64、radix2..36、小文字 digit、有限 binary64 |
| bits | and, or, xor, not, count, shiftLeft, shiftRight, shiftUnsigned | 64-bit 二の補数、shift0..63、left は wrapping、right は符号拡張/zero fill を区別 |

## SDK から導入

配布者の公開鍵を別経路で確認し、development project に導入する。

```sh
rewind sdk-verify --sdk /path/to/sdk --public-key PUBLIC_KEY_HEX
rewind sdk-install --root /path/to/project --sdk /path/to/sdk --public-key PUBLIC_KEY_HEX
rewind check --root /path/to/project
```

```rewind
import std.number as number;
assert_eq(number.integer("-ff",16),Ok(-255));
```

署名付き std source を project の vendor に固定する。ネットワーク、自動 trust、global import、SDK 自動更新は使わない。旧 language を使う既存 project はその版の module source を保持する。`examples/v091/lib` は v0.9.1 の固定例である。

## 位置・失敗・予算

codec/parse/format/slice/shift/mulMod は `Result<...,StdError>`。StdError は immutable record `{code:String,offset:Int}`。入力値をエラーへ含めない。code は Utf8 / Range / Radix / InvalidNumber / NumberRange / EmptyDelimiter / ShiftRange / Modulus / Limit。offset は UTF-8 decode の不正部分開始 byte、それ以外は0。整数は空白や prefix を受理せず、符号と radix の digit を解析する。Float は Rust の有限数解析/最短 round-trip 表示に従い、decimal 精度や JSON canonicalization を保証しない。

text trim/split/tokens/slice、UTF-8 encode/decode、数値 parse の入力は1 MiBまで。split/tokens の出力は65,536要素・合計1 MiBまで。bytes slice の出力は1 MiBまで。find は1 MiBを超える入力で通常の実行エラーとなり、成功時は Option<Int>。length と bytes get はサイズ上限を追加せず既存の値を読む。pure は Host effect がないことを意味し、計算量や heap allocation が無制限という意味ではない。Secret は明示 reveal なしに通常の型へ渡せない。

List/Map の更新、snapshot、Checkpoint、replay は既存 runtime の契約に従う。callback は別 task へ spawn しない。新しい List を返す処理は caller が所有する。mutable payload の Result/Option は move/borrow/freeze を必要とする。

## 開発と検証

```sh
rewind update --root libraries/std
rewind test --root libraries/std
rewind doc libraries/std/text.rw --root libraries/std --output /tmp/text-api.md
rewind api-snapshot --root libraries/std --output /tmp/std-api.json
```

5契約テストで codec、Unicode index、bit/number 境界、Result の両分岐、collections、設定優先順位と引数失敗を検証する。SDK の組立てもこのテストを実行する。Host I/O は app で許可・観測・publish を明示する。SDK の例は sum と JSON 保存 app。配布手順は [v0.9.2仕様](../docs/REWIND_v0.9.2.md)、主要なアルゴリズムと性能改善は [v0.9.3草案](../docs/REWIND_v0.9.3.md) を参照。
