# REWIND プログラミング言語設計案 v0.3

作成日: 2026-09-29
状態: 提案書。実装済みの v0.2 を土台に、ライブラリと複数ファイルのアプリケーションを安全に書くための仕様を定める。

## 1. 背景と目的

v0.2 は基本型、関数、制御構文、List・Map・struct、Result と Option、モジュール、仮想 I/O、Checkpoint の制御位置を提供する。次に言語として必要な要素は、再利用できる型抽象、状態を持つ関数値、閉じたデータ型、ライブラリの公開境界、実行環境の決定的な観測である。

v0.3 では次の六つを優先する。

1. 型パラメーターと trait による汎用 API。
2. enum と網羅性を検査するパターンマッチ。
3. クロージャーと、その捕捉状態の Checkpoint 復元。
4. モジュールの公開範囲と依存関係を固定するプロジェクト設定。
5. コマンド引数・環境値・ファイル列挙の仮想化。
6. エラー診断、テスト、整形、API 文書生成の開発体験。

v0.2 のソースはそのまま解釈できることを互換性条件とする。

## 2. 型パラメーターと trait

### 2.1 宣言と単相化

```rewind
fn identity<T>(value: T) -> T {
    return value;
}

struct Pair<A, B> {
    first: A,
    second: B,
}
```

関数、struct、enum に型パラメーターを導入する。型引数は呼び出し元の値から推論できる場合に省略可能とし、公開 API では注釈された型を基準に検査する。v0.3 の実装方式は単相化とし、同じ型引数の組に対するコードを一度だけ生成する。再帰的な型の展開と生成コード量には Budget を設ける。

### 2.2 trait と実装

```rewind
trait Render {
    fn render(self: Self) -> String;
}

impl Render for Pair<Int, Int> {
    fn render(self: Self) -> String {
        return "(" + self.first + ", " + self.second + ")";
    }
}

fn show<T: Render>(value: T) -> Unit {
    Out.println(value.render());
}
```

trait は必要なメソッドの型だけを定義し、状態を持たない。実装の探索はコンパイル時に確定し、同じ呼び出しで再実行ごとに別の実装が選ばれない。あいまいな実装と重複実装は型エラーとする。公開 trait の実装は、定義元のパッケージまたは実装対象の型の定義元に限る。

`Eq`、`Ord`、`Hash`、`Display`を標準 trait とする。`Map<K,V>` のキー型 K は `Ord` を満たす必要がある。順序は trait 実装が返す全順序に従い、同じキーと同じ Checkpoint から常に同じ列挙順になる。v0.2 の基本型キーは従来の順序を保つ。

## 3. enum とパターンマッチ

```rewind
enum Command {
    Write(String, Bytes),
    Quit,
}

fn describe(command: Command) -> String {
    match command {
        Command::Write(path, _) => return path,
        Command::Quit => return "quit",
    }
}
```

enum は閉じた variant 集合を持つ値型とする。位置引数と名前付きフィールドの両方を認める。`match` は式としても使え、各 arm の型を統一する。`_`、値、範囲、struct、List、enum、`Some`/`None`、`Ok`/`Err` のパターンを提供する。到達不能 arm は警告、欠けた variant はコンパイルエラーとする。ガード式は Bool で、副作用は仮想 I/O までに限る。

既存の v0.2 の `match` 文と `Result`/`Option` の表記は有効なままにする。

## 4. 関数値とクロージャー

```rewind
fn make_counter(start: Int) -> fn() -> Int {
    var current = start;
    return || -> Int {
        current += 1;
        return current;
    };
}
```

関数を値として渡し、返せるようにする。クロージャーは作成時の環境を捕捉する。`let` は値を捕捉し、`var` は VM 管理の可変 cell を捕捉する。cell と関数値は Heap 上の Copy-on-Write オブジェクトとして Checkpoint 対象に含める。同じ Checkpoint への `resume` は捕捉された可変値と Call Frame を一緒に復元する。

クロージャーから直接 Host 操作は呼べない。仮想 I/O は通常の API と同じ規則で使える。`defer` はクロージャーを登録できるが、登録後の `publish` など不可逆操作は静的に拒否する。捕捉参照が関数の寿命を超える場合も VM 管理の cell に保持し、参照先が消えることはない。

## 5. モジュール、公開範囲、パッケージ

### 5.1 公開範囲

`pub` を関数、struct、enum、trait、定数に適用する。省略時はモジュール内だけに公開する。`import text.format as fmt;` と選択的 import を追加する。同名の公開項目が衝突した場合、暗黙に片方を優先せず明示的な別名を要求する。循環 import は v0.2 と同様にコンパイルエラーとする。

### 5.2 プロジェクト設定

リポジトリルートの `rewind.toml` に言語版、エントリーポイント、ソースルート、依存パッケージを記す。依存解決結果は lockfile に固定する。実行時はネットワークからコードを取得しない。パッケージ解決とビルドは `check` または明示したビルド段階で完了させる。依存物のハッシュを記録し、同じ lockfile とソースから同じ命令列を生成する。

v0.2 のファイル単位 import と `rewind run FILE --root DIR` は引き続き利用可能とする。

## 6. 決定的な実行環境

v0.2 で提供していない外部観測を、Host 直読みではなく仮想 API として追加する。

| API | v0.3 の扱い |
| --- | --- |
| `Args.all()` | 起動時の引数列を Journal に記録し、再実行で同じ列を読む。 |
| `Env.get(name)` | 許可した名前だけを観測し、値または欠落を Journal に記録する。 |
| `Directory.entries(path)` | 正規化したパスの一覧をキー順で返し、観測結果と Host 版を保存する。 |
| `Locale.current()` | 明示された固定ロケールだけを返す。暗黙の Host ロケールは使わない。 |

Checkpoint は各観測カーソルを保存する。Journal 本体は v0.2 と同様に不変で、`revert`/`resume` による再実行では同じ値を再生する。`publish` は観測したディレクトリ状態と書き込み対象を再検証する。シークレットに相当する環境値は通常のトレースに出さず、明示的な権限でのみ取得する。

## 7. エラーとリソースの扱い

通常の失敗は `Result<T,E>` と `?` を使う。エラー型には原因の連鎖と発生位置を保持する。`FileError` はコード、パス、原因を持つ構造化型へ拡張する。`panic` は回復不能なプログラム不変条件違反に限定し、仮想状態の破棄と `defer` の実行順序を仕様化する。

`using resource = File.open(...)` を導入し、スコープ終了時に `close` を自動登録する。`defer` と `using` は LIFO 順で実行し、`return`、`?`、実行時エラーのいずれでも仮想リソースを閉じる。`revert`/`resume` では保存した Frame の cleanup 登録状態へ戻す。cleanup から `publish` は呼べない。

## 8. 開発ツール

- `rewind fmt`: 安定した整形規則でソースを整形する。
- `rewind doc`: 公開 API と例を HTML または Markdown に生成する。
- `rewind test`: テストごとに独立した RuntimeState を使う現行方式を維持し、`assert_eq`、失敗位置、テスト名フィルターを追加する。
- `rewind check`: 複数ファイルのエラーを収集し、位置、期待型、実際の型、修正候補を表示する。
- `rewind trace`: Checkpoint の親、PC、Frame、仮想変更、観測カーソルを機械可読 JSON にも出力する。トレースはプログラムの仮想 stdout/stderr へ混ぜない。

フォーマッタと文書生成はコードを実行しない。テストは明示的な `publish` を禁止し、Host の書き込みを行わない。

## 9. v0.3 完了条件

1. 型パラメーター付き関数と struct、trait bound、複数の実装を正しく型検査する。あいまいな実装は拒否する。
2. enum の値が Checkpoint の分岐・復元後も漏れず、`match` の欠落と到達不能 arm を診断する。
3. クロージャーが捕捉する可変 cell を `commit`、`revert`、`resume` で復元し、再実行結果が一致する。
4. モジュール公開範囲、依存 lockfile、初期化順を固定し、循環と未許可パスを拒否する。
5. 引数、環境値、ディレクトリ一覧が Journal から決定的に再生される。
6. `defer`/`using` が正常終了とエラー伝播で仮想リソースを解放する。
7. v0.1 と v0.2 の既存スクリプトおよびテストを変更せずに通す。

## 10. 実装順序と対象外

1. 型表現と単相化、trait 解決、Map キーの順序規約。
2. enum、パターン、網羅性検査。
3. クロージャーの環境 cell と VM 命令、Checkpoint テスト。
4. 公開範囲、`rewind.toml`、依存 lockfile。
5. 実行環境の観測 Journal、`using`、診断とツール。
6. 互換性、予算、決定性、Host 競合の統合試験。

並列実行、async、ネットワーク、データベース、FFI、子プロセス、GPU、デバイス I/O は v0.3 の通常 API に含めない。導入時には観測と不可逆効果の境界を別途設計する。

この文書は v0.3 の提案であり、実装済みであることを意味しない。
