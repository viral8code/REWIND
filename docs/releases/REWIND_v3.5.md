# REWIND 3.5.0

言語構文は3.0.0以降と同じです。既存APIは変えていません。

## 追加

- **`std.failures`**（QUAL-02）：実backendと同じ型・code・phaseのDbError/HttpError値を作るfixtureと、用意した結果を順に返す`Script`。接続もnetworkも作らず、使い切ると`FixtureExhausted`で止まります。
- **`std.httpRouter`のcompiled router**（SVC-01）：`compile`/`resolve`。`{name}`・末尾`{*rest}`のparameter、左からの優先順位（literal > `{name}` > `{*name}`）、同じmethodと形のrouteをcompile時に拒否、404と405（許可methodを返す）の区別、HEADはGETへ。
- **`std.collections`**（LANG-04）：`tryMap`・`tryFold`（最初のErrで止まり入力を消費しない）、`partition`、`collectUpTo`。
- **`rewind compat [--json]`**（DIST-03）：compiler・language・artifact・trace・session・lock・API document等を別々の軸で表示し、不一致時の挙動を示します。別compiler/別formatのartifactとtraceが明示的に拒否されることをテストで確認しました。

## 検査の追加

- API文書の継続照合（DOC-01）：公開関数が費用を書いていなければテストが失敗します（既存の未記載は一覧に残し、減る方向にしか変えられません）。標準moduleにAPI baselineが無くても失敗します。履歴・変更操作・parser・schemaなど14moduleのbaselineを追加し、`api-diff --deny-breaking`の対象にしました。
- 論理仕事量の予算（PERF-01の決定的な部分）：小さな基準program5本のVM命令数とstorage workが予算を20 %超えて増えるとテストが失敗します。

詳細は[service構想](../archive/v2-next-services.md)・[言語構想](../archive/v2-next-language.md)・[品質構想](../archive/v2-next-ecosystem.md)・[性能計画](../archive/v2-next-performance.md)を参照してください。
