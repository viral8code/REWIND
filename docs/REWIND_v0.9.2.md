# REWIND v0.9.2 — SDK と標準ライブラリ

更新日: 2026-10-01。状態: **初回実装済み**。compiler/package 0.9.2、language 0.9.2。[実装状況](v0.9.2-status.md) と [ライブラリ一覧](../libraries/README.md) を参照。

## 配布単位

compiler/runtime・開発ツール・std source・API 文書・実行例・license を一つの SDK directory として組み立てる。初回の対象は Linux x86_64。同じ `rewind` binary が check/test/doc/build/run/LSP/debug/profile を提供する。runtime-only binary の分離と他 OS/architecture は後続の検証単位とする。

```text
rewind-sdk-0.9.2/
  bin/rewind
  lib/rewind/std/                 # source, manifest, lock, package metadata/signature
  share/rewind/doc/std/           # module API docs
  share/rewind/doc/std-api.json
  share/rewind/examples/{sum,app}/
  share/rewind/Cargo.lock
  licenses/
  sdk.json
  sdk.json.signature
```

`sdk-build --output NEW_DIRECTORY --key SEED_FILE` は同梱 std の契約テストを実行し、API 文書と snapshot を生成して署名する。ファイル一覧・SHA-256・size・実行権限・target・compiler/language/std の版を sdk.json に固定する。同じ binary と同じ seed による二回の組立ては同じ manifest/signature になる。Rust binary 自体の再現ビルド、archive の生成、公式公開は別の手順であり、このコマンドには含めない。

## std の導入と信頼

`sdk-verify --sdk DIRECTORY --public-key KEY` で SDK 全体と std package の署名を検証する。SDK と app release/artifact は署名 domain が異なる。秘密鍵は SDK に含めない。配布者の公開鍵は利用者が別経路で確認する。

`sdk-install --root PROJECT --sdk DIRECTORY --public-key KEY` は明示した SDK を検証し、std を project の `vendor/` へコピーする。`import std.text as text;` などが通常の署名付き dependency として解決される。manifest に exact version・signer/public key を明示し、lock に source/package digest と版を固定する。通常の build/run は SDK の配置場所に依存しない。SDK を置き換えても既存 project の std を更新しない。

この project 内 snapshot 方式を初回の探索仕様として採用する。環境変数/default path による自動探索を導入せず、source root の外への自由な import を許可しない。既存の std namespace、signer entry、コピー先、symlink は衝突として拒否し、探索順で置き換えない。依存更新の失敗時は manifest/lock を復元する。全操作は offline で完結する。

## ライブラリと処理系

既存 json/config/args/collections/math/option に text/bytes/number/bits/result/map を追加した。fold/find/count、Map getOr/contains、Result map/mapError/valueOr は `.rw` の関数で実装する。UTF-8、Unicode scalar の位置、数値変換、64-bit bit 操作、overflow を避ける mulMod は小さな pure native primitive を土台にする。

Option/Result の Share/Send は中身の型に従う。mutable owner の複製は許可しない。generic Result の存在しない分岐の型引数は、型検査した呼出しを bytecode に保持して失われないようにする。generic Frozen collection から読み出した scalar の thaw も扱う。

API の所有権・効果・失敗・予算・順序は [libraries/README.md](../libraries/README.md) に記載する。Host observation、publish、Secret の公開判断をライブラリへ隠さない。

## 継続する設計項目

初回完了条件は、署名可能な SDK を生成し、同梱 std を固定して CLI アプリを build/replay/source-free run できることとする。設定 schema と JSON 保存には v0.9.1 の実装を利用する。

元の候補一覧のうち bool/短い option/help、Map entries/update facade、path join、process 間 lock、schema migration、CSV、日時/decimal、構造化 log、HTTP/DB/UI adapter は未実装。今回の SDK 完了と区別し、[v0.9.3 草案](REWIND_v0.9.3.md) と後続の library 計画へ引き継ぐ。複数ファイル/stream の publish を globally atomic にする保証は追加しない。
