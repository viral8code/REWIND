# REWIND v0.9.2 草案 — SDK 配布と標準ライブラリ

作成日: 2026-10-01。状態: **草案・未実装**。基準は [v0.9.1 実装状況](v0.9.1-status.md)。OpenJDK のように、言語処理系だけでなく、開発ツール・標準ライブラリ・API 文書・source・license を揃えた配布を目指す。Java/JVM の互換実装、OpenJDK の license の採用を意味しない。

## 1. いまある土台

v0.9.1 で JSON AST/codec、config/args、collection algorithms、Int math、Option の REWIND library module を実装した。[libraries/README.md](../libraries/README.md) に API と利用方法を記載する。設定検証・引数解釈・map/filter の方針を compiler へ閉じ込めず、検査・文書化・artifact 化できる `.rw` として保つ。

List/Map の storage、ownership、型、効果、Checkpoint、Secret、native JSON parser のような基盤は compiler/runtime の責務。便利な API を全て builtin として増やす必要はない。必要な UTF-8/数値変換/OS adapter の primitive は小さく公開し、上位 API と policy を library に置く。

## 2. 配布物を分ける

| 配布物 | 内容 | 用途 |
|---|---|---|
| REWIND SDK | compiler/runner、build/test/doc/LSP/debug/profile、std source、API docs、examples、license、release manifest | 開発・CI |
| application runtime image | verified artifact、必要な std の compiled code、allowlist asset、runner、effect/budget policy、release/signature | アプリ利用者 |
| source distribution | compiler/runtime/std の source、lock、生成手順、tests、第三者 license | 再ビルド・保守・監査 |

初版は現在の同じ `rewind` binary を SDK/runtime image へ配置する。compiler を除いた小さい runner を既に持つと説明しない。compiler と runtime の分離、静的リンク、embedding は後の実装単位にする。stdlib は artifact の typed IR/bytecode に取り込めるので、アプリ実行時に source を必須としない。

SDK の候補 layout:

```text
rewind-sdk-0.9.2-<target>/
  bin/rewind
  lib/rewind/std/            # .rw source + public API snapshot
  share/rewind/doc/          # API / language / diagnostics
  share/rewind/examples/
  licenses/                 # MIT + dependency licenses
  release.json              # compiler, language, std, target, file hashes
  release.json.signature
```

初回の target は CI で実行検証できた Linux x86_64 を採用する。Linux aarch64 / Windows / macOS は実機または CI の smoke test が通った後に追加する。cross-compile に成功しただけでは「対応済み」としない。

## 3. std の探索・版・信頼

- `import std.json as json;` を SDK の std に解決する。project が指定する mirror/package と SDK std の衝突はエラーとし、探索順で静かに置き換えない。
- SDK を明示する option / REWIND_SDK_ROOT と default SDK path を定義する。project の source root から自由に外へ import する緩和とは分ける。
- std の version、compiler compatibility、public API snapshot、source/package digest、signer を project lock と artifact の fingerprint に残す。SDK 変更だけで過去の lock を書き換えない。
- SDK 全体の署名と、app artifact/release の署名を分ける。official key は release 時に管理し、private key を repository / SDK に含めない。利用者は許可した signer と失効・rotation を指定できる。
- offline mirror を第一にし、ネットワーク fetch は別途 opt-in の設計にする。未署名 fallback、自動 trust、無通知の std 更新を導入しない。

現在の project 内 source-copy を維持しながら、まず一つの std package を SDK へ同梱する。細かな std module の個別 version 解決は初版で要求しない。

## 4. 開発で使うライブラリを揃える

| 優先 | module 候補 | 最初の API と検証 |
|---|---|---|
| P0 | text / bytes / number | UTF-8 decode、split/trim/search/slice、数値 parse/format。byte offset と Unicode scalar index を区別し、overflow/invalid input/出力上限を Result で返す |
| P0 | collections / option / result | fold/find/count、Map の getOr/entries/update、Result の map/mapError/valueOr、immutable view。空 collection、順序、所有権、callback 効果を検証 |
| P0 | config / cli | 長短 option、bool flag、help、必須/default、未知項目、schema の重複名、秘密項目。既存の小さな API を壊さず拡張 |
| P0 | path / io / storage | root 内 path の検証・join、bounded read、単一文書 store の schema version/migration、optimistic conflict。process lock と atomic replacement の契約を追加 |
| P1 | json / csv | Json record/enum の明示 codec、CSV quoting/newline、streaming/予算、位置付きエラー。任意 reflection は不要 |
| P1 | time / decimal | Instant/Duration と観測、timezone の版、decimal 精度/丸め/overflow。金額を Float へ暗黙変換しない |
| P1 | diagnostics / logging / testing | 構造化 log、機密値の伏せ方、typed failure の表示、fixtures/property generators、retention |
| P2 | HTTP / database / UI adapter | Host protocol と別 package。capability、journal、timeout、idempotency、publish の不可逆性を先に定義 |

全 module に、公開 signature、効果、所有権、予算、失敗、順序、秘密値、Checkpoint/replay への影響を記載する。純粋なアルゴリズムは `.rw` で実装し、Host native code は必要な境界に限定する。外部 API の retry や DB commit を普通の関数へ隠さない。

## 5. library を便利にする compiler の補修

v0.9.1 の整備で、`None` の generic 推論と import alias 経由の generic borrow parameter を補修した。次は generic Frozen collection の要素読取り、enum の bound、callback の効果 parameter、module API の namespace と資料生成を実例で確認する。

借用戻り値は library が必要とする具体的な用例から採否を決める。初版は新しい owner/immutable snapshot を返せば実装できる API を優先する。動的 trait object、FFI、macro、reflection を SDK 配布の前提にしない。

## 6. SDK を作る・更新する手順

1. 固定した Rust dependency と source から offline build し、compiler/runtime/stdlib の契約テストを実行する。
2. std の API snapshot/doctest/doc を生成し、公開 API の破壊を diff で検査する。
3. allowlist で SDK ファイルを列挙し、相対 path・hash・size・target・license・compiler/std version を release manifest へ固定する。
4. 同じ input の二回の組立てで同じ manifest と内容を得る。archive の timestamp/permission/order と executable bits を正規化する。binary の完全再現性は toolchain/target の条件と分けて検証する。
5. staging directory を検証してから新規 version directory へ配置する。既存 SDK を上書きせず、current の切替えと rollback を別操作にする。
6. 公式公開は署名鍵と配布先を設定した release 手順で行う。今回の source push を SDK の公開と扱わない。

SDK smoke test は別 directory の利用者 project を作り、SDK std import、型検査、test/doc/build、asset 付き install、署名検証、source-free run、record/replay まで実行する。秘密鍵/cache/trace/開発依存の混入、改変、symlink、path traversal、版不一致を拒否する。

## 7. 初回の完了条件

v0.9.2 の最初の実装単位を「署名可能な SDK directory を生成し、同梱 std を固定して CLI アプリを作れる」とする。

- SDK layout/manifest/検証/新規 directory への組立てを実装する。
- `std.*` の明示探索と lock/compiler/std 互換検査を実装する。
- text/bytes/number と collections/Option/Result の日常的な API を揃え、schema 付き CLI と JSON 保存アプリの例に使う。
- std source/API docs/examples/license を配布し、target の smoke test と API の互換差分を CI へ追加する。

HTTP/DB/GUI と runtime-only binary の分離は別段階。v0.9.1 の publish は複数ファイル/stream を globally atomic にしないため、SDK や library の名称で transaction 保証を広げない。
