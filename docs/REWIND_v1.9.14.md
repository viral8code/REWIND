# REWIND v1.9.14

## manifest を残したコンパイル済み配布

`rewind compile src/main.rw --output app.rwc` で生成したアプリケーションを、`rewind.toml` / `rewind.lock` を残したまま `rewind run app.rwc` で実行できる。元の entry、source directory、vendor package は配布に不要。必要な asset は保持する。`rewind replay trace.json` も同じ runtime policy を使う。

従来の source 実行・check・compile・update は、実在する source graph と package の署名・依存関係を検証する。compiled 実行は埋め込まれた typed IR / bytecode を検証し、同じ graph を disk から開き直さない。manifest を無視したり、すべての effect を許可したりする変更ではない。

## 実行時の検証

manifest がある場合は quoted values、既知の section / key、language / dependency mode を検査する。重複 key は拒否し、manifest の容量は1 MiB。source_root / entry は相対パスの制約を保つが、実在を要求しない。

lock は必須で最大32 MiB。format、language、compiler、effect、assets と dependencies object の形式を検査する。runtime では source や vendor を読み直して lock の dependency graph を再計算しない。コンパイル時に取り込んだ program と一致する IR / bytecode の検証、必要に応じた artifact signature の検証を保持する。

manifest の effect が実行の許可範囲となり、CLI の `--allow-effects` で不足を上書きしない。lock と許可が違えば、出力前に `compiled runtime lock mismatch`。program の必要な effect が許可されなければ既存の effect 診断で拒否する。

asset の path、symlink、容量、byte 数と SHA-256 を従来どおり検査する。manifest / lock / artifact の inventory と実ファイルの整合を要求し、欠落・変更・別 inventory の差替えを拒否する。manifest の language と embedded program の language も一致させる。manifest のない単体実行の許可規則は従来どおり。

`.rwc` と trace は compiler の完全な版に対応する。別の compiler 版へ移行する際は source から再 compile する。任意の旧 compiler の artifact をそのまま実行できる保証は追加しない。

## 定数と source map

source directory のない artifact の純粋な定数評価では、source map の最寄りの実在する親 directory を VM の基点にする。定数の effect 禁止、有限 work / memory、virtual publish を維持する。source map が元の directory の存在を要求する問題を解消し、元の位置を指す診断は保持する。

## 検証

entry と source directory、compile cache を削除し、manifest / lock / asset を残した module + const の実行、debug / compact 記録と replay を検証する。source の check が引き続き source を要求することも確認する。権限の不足、lock の欠落・不正 JSON・不整合、重複 key、path traversal、language の不一致、asset の欠落・改変・追加 inventory を拒否する。

両 OS の展開済み SDK では std package を使う最短路の例を compile し、source / vendor を削除して `.rwc` を実行・記録・replay する。既存の artifact 単体実行と、実 HTTP / SQLite / PostgreSQL / GUI / numeric の SDK 検証も継続する。GUI / 通信 / DB の長時間統合、追加の GUI 機能、協調的 kernel、sparse / transform / 勾配と v2.0 の到達条件は引き続き実装する。
