# REWIND プログラミング言語設計案 v0.2

作成日: 2026-09-29
状態: 設計案。v0.1のTransactional Runtimeを土台とし、言語としてプログラムを書くための構文と意味を定める。実装はブランチ `codex/rewind-v0.2` のコミット `17f895c` を参照。

## 1. 目的と範囲

v0.1は実行状態、Checkpoint、仮想I/O、`publish`境界を扱う。v0.2では、実用的なプログラムを記述するために次を追加する。

1. 型と演算、真偽値、条件分岐、反復。
2. 関数、引数、戻り値、再帰、ローカルスコープ。
3. コレクション、構造体、モジュール。
4. 実行時エラーの処理とリソースの安全な解放。
5. `revert`後の制御位置を含む、Checkpoint操作の明確な意味。
6. 型検査、診断、テストの最低限の開発体験。

ネットワーク、データベース、子プロセス、GPU、デバイスI/Oはv0.2の通常APIに含めない。これらを導入するときは別途Transactional化または明示的なunsafe境界を設計する。

## 2. v0.1から引き継ぐ原則

- `commit name;`は名前付きRuntimeStateを保存し、Hostへ反映しない。
- `revert name;`は選択したCheckpointの計算状態と仮想I/Oを復元する。
- `publish;`だけが通常コードからHostファイル、stdout、stderrへ反映する。
- 入力・時刻などの外部観測は不変のJournalに記録し、RuntimeState側のカーソルを復元する。
- Randomの内部状態、Heapオブジェクト、Call Frame、Program CounterはCheckpoint対象とする。
- v0.1実装と同様、未対応の外部操作はデフォルトで拒否する。

## 3. 型システム

### 3.1 基本型

| 型 | 用途 | v0.2の規則 |
| --- | --- | --- |
| `Bool` | 条件 | `true` / `false`。数値から暗黙変換しない。 |
| `Int` | 整数 | 符号付き64 bit。オーバーフローは実行時エラー。 |
| `Float` | 実数 | IEEE 754 binary64。決定性が必要な箇所では演算順序を固定する。 |
| `String` | 文字列 | UTF-8。連結は`+`。 |
| `Bytes` | バイナリ | ファイルAPIで文字列との混同を避ける。 |
| `Unit` | 値を返さない操作 | `()`で表す。 |

`List<T>`、`Map<K,V>`、`Option<T>`、`Result<T,E>`を標準の型として提供する。`Map`の反復順は挿入順またはキー順のどちらかに固定し、同じCheckpointからの再実行で順序が変わらないようにする。v0.2ではキー順を採用する。

### 3.2 型注釈と推論

ローカル変数の初期化式から型を推論できる。公開関数の引数と戻り値には型注釈を必須とする。`let`は再代入不可、`var`は再代入可とする。変更可能なHeapオブジェクトは参照を共有するため、`let list = List<Int>();`でも`list.add(1)`は可能とする。参照の別名とCheckpoint時のCopy-on-WriteはRuntimeが管理する。

## 4. 式と制御構文

演算子は`+ - * / %`、比較`== != < <= > >=`、論理`&& || !`を提供する。`&&`と`||`は短絡評価する。条件式は必ず`Bool`とする。

```rewind
let limit: Int = 10;
var total = 0;
for n in 0..limit {
    if n % 2 == 0 {
        total += n;
    }
}
Out.println(total);
```

`if/else`、`while`、`for`、`break`、`continue`を追加する。無限ループや再帰の制限には実行Budgetを設け、超過時は`ExecutionBudgetExceeded`を返す。これは履歴Budgetとは別である。

## 5. 関数、スコープ、Call Frame

```rewind
fn sum_to(n: Int) -> Int {
    var total = 0;
    for x in 1..(n + 1) {
        total += x;
    }
    return total;
}

let answer = sum_to(5);
```

- 呼び出し時に引数、ローカル変数、戻り先PCを含むCall Frameを積む。
- `return`でFrameを外し、戻り値を呼び出し元に渡す。
- ブロックスコープを採用し、内側の`let`/`var`は外側をシャドーイングできる。重複宣言には警告を出す。
- 再帰を許可する。Call FrameとStackはCheckpointで共有され、復元後は当時の内容に戻る。
- 関数内で作ったCheckpointの名前はRuntime全体で一意とする。動的な名前生成と名前空間は将来検討する。

## 6. Checkpointと制御位置

v0.1の例は`revert base;`の次の文から実行を続ける。一方、RuntimeStateには保存済みPCが含まれる。v0.2では次の二つを区別する案を採用する。

- `revert X;`: XのRuntimeStateを復元した後、現在の`revert`文の次から続行する。復元イベント直後のPCはXの値であり、次文を開始するときに続行位置へ設定する。v0.1の例との互換性を保つ。
- `resume X;`: XのRuntimeStateとPCを復元し、そのPCから実行を再開する。再実行・探索用の明示操作とする。

`revert`の続行位置が復元後のCall Frameに存在しない場合は`InvalidContinuation`とする。`resume`は保存されたCall Frameをそのまま使う。`publish`後の`revert`/`resume`は内部状態を復元できるが、既にHostへ出た効果は消せない。

この制御位置の区別はv0.1仕様の不明瞭な箇所を補うため、実装前にテストを先に確定する。

## 7. エラー処理

通常の失敗は`Result<T,E>`で表し、`?`で呼び出し元へ伝播させる。プログラムの型エラー、整数オーバーフロー、予算超過、不正なCheckpoint、外部状態競合は診断情報を持つ実行時エラーとする。

```rewind
fn load(path: String) -> Result<String, FileError> {
    return File.readText(path);
}

match load("config.txt") {
    Ok(text) => Out.println(text),
    Err(error) => Err.println(error),
}
```

`defer`を追加し、関数の終了やエラー伝播時に仮想ファイルハンドルなどのリソースを閉じる。`revert`/`resume`時は古いFrameを復元するので、`defer`にHostへの不可逆操作を許さない。

## 8. データ構造とモジュール

`struct`で名前付きデータを定義する。値の変更はCopy-on-WriteのHeap操作として記録する。`List<T>`と`Map<K,V>`の更新もCheckpointから復元できる。

```rewind
struct Task {
    title: String,
    done: Bool,
}

import text.format;
```

モジュールはファイル単位とし、`import`はプロジェクトルート内のパスだけを解決する。初期化順序を固定し、循環importはコンパイルエラーとする。標準ライブラリのI/Oは必ず仮想I/O層を通す。

## 9. I/Oと決定性

文字列用の`File.readText`/`File.writeText`とバイナリ用の`File.readBytes`/`File.writeBytes`を分ける。ファイルハンドルは`read`、`write`、`seek`、`close`を持つ。`Out.flush()`はJournalへの確定であり、外部表示は行わない。

`Time.now()`と`In.readLine()`は観測Journalを使用する。`Random.next()`はCheckpointに保存された生成器状態から計算する。実行結果に影響する環境変数、ロケール、ファイル列挙順については、v0.2で仮想化または禁止を明文化する。

## 10. 診断と開発ツール

- パーサーは行・列、原因、修正候補を含む診断を返す。
- `rewind check <file>`で構文・型検査だけを行い、Hostを変更しない。
- `rewind run <file> --root <dir>`で実行する。`publish`を含まないプログラムはHostへ出力しない。
- `rewind test`でテスト関数を実行し、各テストに独立した仮想I/O状態を与える。
- トレースにはCheckpoint名、親、現在のPC、仮想変更一覧、観測カーソルを表示する。トレース自体はRuntimeのstdoutと分離する。

## 11. v0.2完了条件

1. `if/while/for`と関数呼び出し・再帰・戻り値が動き、Stack/Frame/PCをCheckpointから復元できる。
2. 型エラーと`Result`の伝播に対し、位置付き診断が出る。
3. List、Map、structの変更を分岐・復元しても別の経路へ漏れない。
4. `revert`と`resume`の制御位置を別々にテストし、公開済み副作用の扱いを明示する。
5. `publish`前のファイル・標準出力・標準エラーがHostへ出ない。
6. 同じCheckpointと同じ観測Journalからの再実行結果が一致する。
7. v0.1のスクリプト例が互換モードなしで通る。

## 12. 実装順序

1. 字句解析、AST、位置付き診断と型表現。
2. 式、条件分岐、反復、変数スコープ。
3. 関数とCall Frame、`Result`、`defer`。
4. コレクション、構造体、モジュール。
5. `revert`/`resume`の制御フローと決定性テスト。
6. CLIの`check`/`run`/`test`と互換性確認。

この文書はv0.2の設計案である。実装済みの機能と使い方はリポジトリの `README.md`、検証項目は `tests/language_v02.rs` を参照。
