# REWIND

REWIND は、`commit` / `revert` / `publish` で VM の状態と入出力を扱う、Rust 製のプログラミング言語です。名前付きの保存点、決定的な replay、仮想的な入出力を組み合わせ、外部へ出る操作を `publish` まで保留できます。

現在の版は 4.1.0 で、開発は `codex/develop` branch で行っています。

## クイックスタート

[GitHub Releases](https://github.com/viral8code/REWIND/releases/latest) から Linux x86_64 / Windows x64 の SDK を取得し、導入手順は [docs/getting-started.md](docs/getting-started.md) を参照してください。

3.0.0 以降、プログラムには `fn main() -> Int` が必要です。次の例を `readme_example.rw` として保存し、`rewind run readme_example.rw` で実行すると `[15]` が表示されます。`revert base;` で `99` の追加が取り消され、`publish;` で保留中の出力が確定します。

```rewind
fn sum_to(n: Int) -> Int {
    var total = 0;
    for x in 1..(n + 1) {
        total += x;
    }
    return total;
}

fn main() -> Int {
    let items = List<Int>();
    items.add(sum_to(5));
    commit base;
    items.add(99);
    revert base;
    Out.println(items);
    publish;
    return 0;
}
```

## コマンド

| 目的 | 形式 |
|---|---|
| 実行 | `rewind run FILE.rw [--allow-effects EFFECTS]` |
| 中間形式へのコンパイル | `rewind compile FILE.rw [--output FILE.rwc]` |
| 中間形式の実行 | `rewind run FILE.rwc [--allow-effects EFFECTS]` |
| コンパイル専用の実行ファイル | `rewindc FILE.rw` |
| プロジェクト操作 | `rewind check\|test\|run\|build --root DIR` |
| 補助ツール | `rewind update`、`doc`、`fmt`、`replay`、`sdk-build`、`sdk-install`、`sdk-verify` |
| 実行予算 | `--steps N`、`--native-work N`、`--task-steps N`、`--memory-mib N`（Linux） |
| 履歴の保存量 | `--history-memory SIZE`、`--history-storage SIZE`、`--spill-threshold SIZE`（単位は bytes / KiB / MiB / GiB） |

標準では、入力・出力・引数・ロケール・乱数・タスクだけを使えます。ファイル、環境変数、時刻、GUI、外部ネットワークへのアクセスには、`--allow-effects` で明示的に許可が必要です。

## 言語の主な機能

- `commit 名前;` で状態を保存し、`revert 名前;` で戻します。外部への出力は `publish;` まで保留されます。
- 時刻、入力、環境変数などの観測は記録され、replay で同じ結果を再現できます。
- ファイルへの書き込みと `Out` / `Err` の出力は、`publish` まで仮想の状態にとどまります。
- `move`、`&`、`&mut` で値の持ち主を明示し、借用の規則を実行前に検査します。
- `let` で束縛した名前には再代入できず、`var` で束縛した名前には再代入できます。`Int`、`Float`、`String`、`Bytes`、`List<T>`、`Map<K,V>`、`Option<T>`、`Result<T,E>` を使えます。
- `Result` と `Option` の値に `?` を付けると、失敗を呼び出し元へ伝えます。
- 関数が行う外部操作は効果として型に表れ、呼び出し元で許可されているかを実行前に確かめます。
- `async fn`、`spawn`、`await`、`Channel<T>` で並行処理を書けます。チェックポイントはタスクの状態も保存します。
- `trait`、ジェネリクス、列挙型を使えます。`match` は網羅しているかを検査します。
- `import` で `text/format.rw` のようなファイルを読み込めます。循環する import は拒否されます。
- 命令数やメモリ量の上限を設定でき、無限ループや過大な使用を防げます。

## 標準ライブラリ

標準ライブラリは `libraries/std/` にあり、`import std.名前 as 名前;` で使います。主な分類は次のとおりです（テスト用のファイルを除く 125 module）。

| 分類 | 代表的な module |
|---|---|
| 値と変換 | `option`、`result`、`number`、`integer`、`bigint`、`decimal`、`bytes`、`bits` |
| 文字列とデータ形式 | `text`、`regex`、`json`、`csv`、`config`、`base64`、`image` |
| コレクションとアルゴリズム | `collections`、`map`、`sort`、`heap`、`graph`、`flow`、`matrix` |
| 数値計算と機械学習 | `numeric`、`fft`、`autodiff`、`optimize`、`training`、`models` |
| 暗号と認証 | `crypto`、`auth` |
| タスクと通信 | `task`、`http`、`httpServer`、`tcp`、`web` |
| データベース | `db`、`dbTransaction`、`dbMigration` |
| GUI | `gui`、`guiForm`、`guiTable`、`guiWindows` |

各 module の公開 API と制限は [libraries/README.md](libraries/README.md) にまとめています。

## ドキュメント

- [入門書（HTML）](docs/book/index.html) / [入門書（PDF）](docs/book/REWIND-4.1.0-book.pdf): 4.1.0 を対象に、基本から実用までを 25 章で説明します。
- [言語リファレンス](docs/language-reference.md)（[PDF 版](docs/language-reference.pdf)）: 言語の正確な定義です。
- [使い方](docs/getting-started.md)、[GUI](docs/gui.md)、[4.1.0 の変更点](docs/REWIND_v4.1.md)、[4.0.0 の変更点](docs/REWIND_v4.0.md)、[エディタ作成ガイド](docs/editor-guide.md)
- [入門書の編集と再生成](docs/book/README.md)、[ロードマップ](docs/ROADMAP_v2-next.md)

## 開発

作業は `codex/develop` で行います。変更の前後に次のコマンドを通します。

```sh
cargo test --release --no-fail-fast
python scripts/check-docs-examples.py docs/language-reference.md docs/book/chapters/*.md
```

文書を変えたときは、次のコマンドで HTML と PDF を再生成します。

```sh
python scripts/build-book.py
python scripts/build-reference-pdf.py
```

Linux での確認には WSL（`Ubuntu-24.04`）を使い、ビルド出力は `CARGO_TARGET_DIR=$HOME/rw-target` に置きます。

## ライセンス

MIT License です。詳しくは [LICENSE](LICENSE) を参照してください。

## 履歴

v0.1 から v2.x までの README の説明は [docs/readme-history.md](docs/readme-history.md) に移しました。現在の仕様は [言語リファレンス](docs/language-reference.md) を参照してください。
