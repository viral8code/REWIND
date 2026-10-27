# REWIND 3.1.0

言語構文は3.0.0と同じです（`fn main()->Int`、top-level文禁止）。3.1.0は道具とライブラリの追加版で、3.0.0のprogramはそのまま動きます。

## 追加

- **CLI**：project root解決の一本化、全commandの`--help`と機械可読な引数エラー（[CLI-01/02](../archive/REWIND_v2.1-plan.md)）。
- **診断**：関連位置（use-after-moveの移動元など）、checkpoint継続の理由、wait graph。JSON診断に`code`と`related`。LSPの`relatedInformation`にも反映。
- **デバッガ**：`DebugEngine`、値の遅延展開とwatch、`rewind dap`（stdio DAP）。詳細は[debugger.md](../debugger.md)。
- **profile**：`rewind profile`にtask/function/行別の仕事量帰属（exclusive/inclusive/native_work）とcheckpoint保持の説明（`retention`）。`--profile-format text|json`。logical workでありミリ秒ではない。
- **標準ライブラリ**：`std.plot`、`gui.line`、`guiField`、tableのcell編集、`std.dbTransaction`、`std.dbMigration`、DB row mapping（SQLite確認済み）。

## 未完了（次版以降）

- PostgreSQLでの transaction / migration の実backend確認、同時runnerの排他。
- GUI版の統合例、tableのネイティブ編集の仕上げ。

詳細は[v2.1計画](../archive/REWIND_v2.1-plan.md)と[v2.2計画](../archive/REWIND_v2.2-plan.md)の「実装済み」「部分実装」の記述を参照。
