## 付録A 標準ライブラリの地図

標準ライブラリの module は、`import std.名前 as 名前;` で使います。各関数の型、費用、失敗条件は、SDK の `share/rewind/doc/std/` と [libraries/README.md](../../libraries/README.md) にあります。ここでは、どんな module があるかの地図だけを示します。

| 分類 | module |
|---|---|
| 値と変換 | option、result、error、failures、issues、number、integer、modular、bigint、decimal、math、bits、bytes、units、layout |
| 文字列とテキスト | text、unicode、textIndex、textEdit、regex、stringSearch、url、path |
| 日時 | datetime、clock |
| コレクションとアルゴリズム | collections、map、orderedMap、sequence、sort、search、heap、deque、bitset、disjointSet、rollbackSet、fenwick、segment、lazySegment、range、trie、suffix、dp、table |
| グラフと幾何 | graph、graphLarge、graphLargeAsync、flow、matching、geometry |
| データ形式と解析 | json、jsonStream、csv、csvStream、config、args、scanner、stream、parser、schema、logfmt、compress、image |
| 履歴と状態 | snapshot、history、historyGraph、timeline、changes、persistentSet、memo、lazy、lens |
| 数値計算 | numeric とその Async 系、matrix、sparse、sparseAsync、fft、fftAsync、distributions |
| 機械学習 | autodiff 系、optimize、training、models、evaluation、plot |
| タスクと通信 | task、taskError、retry、external、http、httpHeaders、httpRouter、httpServer、tcp |
| データベース | db、dbDatetime、dbDecimal、dbMigration、dbTransaction |
| GUI | gui、guiDialog、guiField、guiForm、guiMenu、guiTable、guiWindows |
| テスト支援 | proptest |

「履歴と状態」の module は、REWIND ならではの道具です。たとえば、`std.timeline` は Undo と Redo の履歴を、`std.history` は名前付きのスナップショットの登録簿を、`std.snapshot` は一部のデータの独立した複製を扱います。チェックポイントは、名前を実行前に決めておく必要があるので、実行中に増えていく履歴には、これらの module を使います。

## 付録B よくあるエラーと対処

実行前に出るエラーと、実行中に止まるエラーのうち、とくに出会いやすいものを挙げます。

| 表示 | 原因 | 対処 |
|---|---|---|
| `cannot assign to let binding` | `let` の名前に代入した | `var` にする。あるいは新しい名前を使う |
| `duplicate binding` | 同じスコープで同じ名前を二度作った | 名前を変えるか、内側のスコープに移す |
| `function returning Int needs a return on every path` | 値を返す関数に、`return` を通らない経路がある | すべての分岐で `return` する |
| `non-exhaustive match` | `match` で扱っていない場合がある | 抜けている variant を足すか、`_` で受ける |
| `match arm expects` | 分岐の値の型がそろっていない | すべての分岐を同じ型にする |
| `passing a mutable owner requires move or freeze` | 変更できる値を、`move` なしで渡した | `move` する、`&` や `&mut` で貸す、`freeze` する |
| `UseAfterMove` | `move` した値をまた使った | 渡した後は使わない。必要なら先に `freeze` する |
| `cannot mutate through a shared borrow` | `&` で借りた値を変更しようとした | 引数の型を `&mut` にする |
| `conflicting borrows` | 同じ値を二つの `&mut` で貸そうとした | 貸す期間を分ける |
| `mutable owner alias` | `List` や `struct` を、代入で別の名前に持たせようとした | `move`、借用、`freeze` のいずれかを選ぶ |
| `MissingEffect` | 効果の宣言か、実行時の許可が足りない | `effects { … }` に足す。実行時は `--allow-effects` で許可する |
| `IntegerOverflow` | `Int` の範囲（64 ビット）を超えた | `BigInt` を使うか、範囲を確かめる |
| `DivisionByZero` | 0 で割った | 割る前に確かめる |
| `IndexOutOfBounds` | 添字が範囲の外 | `len()` と比べてから使う |
| `InvalidContinuation` | `revert` が戻れない保存点を指した | 同じ関数の、有効なスコープの保存点に戻す |
| `ExternalBoundary` | `external` の中で禁止された操作をした | `await` や `publish` などを、領域の外へ出す |
| `ExecutionBudgetExceeded` | 実行ステップの予算を使い切った | 無限ループでないか確かめ、意図した仕事なら `--steps` を引き上げる |
| `unknown name` | 存在しない、または公開されていない名前を使った | 綴りと `pub` を確かめる |

## 付録C 用語集

| 用語 | 意味 |
|---|---|
| 束縛 | 名前と値の結び付き。`let` と `var` で作る |
| 所有者 | 変更できる値（`List`、`Map`、`struct` など）を持っている束縛 |
| 共有値 | 自由にコピーして共有できる値（`Int`、`String`、`record` など） |
| 借用 | 所有権を渡さずに、一時的に値を貸すこと。`&`（読むだけ）、`&mut`（変更する） |
| チェックポイント | 計算状態に付ける名前付きの保存点 |
| 保留中（pending） | まだ外へ確定していない出力や書き込み |
| 公開（publish） | 保留中の入出力を外へ確定すること。確定したものは戻せない |
| 効果（effect） | 関数が外に対して行う操作の種類。`effects { … }` で宣言する |
| 外部領域 | 外の世界から得る値や操作を閉じ込め、結果を記録する `external { }` |
| 予算 | 実行ステップなど、実行の量の上限 |
| 記録と再生 | 外から得た値を記録し、同じ実行を再現すること |
| 診断 | エラーの表示。コード、位置、呼び出しの経路、対処のヒントを持つ |

## 付録D 現状の制約

この本の版（REWIND 3.6.0）の実装で、期待と異なる動作をする点を挙げます。今後の版で変わる可能性があります。

- **`?` のエラー型の不一致は、組み込みの型どうしなら実行前に拒否されます。** `record` や標準ライブラリの型が絡むときは、`Err` の経路を通ったときの実行時エラーになります（第10章）。
- **`List` と `Map` を `==` や `assert_eq` で比べると、中身ではなく同一性を調べます。** 比べたいときは `freeze` します（第3章）。
- **`return` を `branch` の中で使うと、そこで関数から戻りますが、状態は戻りません。** 状態を元に戻したいときは、ブロックを最後まで実行して終えてください（第12章）。

## 付録E 次に読むもの

| 知りたいこと | 読むもの |
|---|---|
| 言語の正確な定義 | [language-reference.md](../language-reference.md) |
| インストールと SDK の検証 | [getting-started.md](../getting-started.md) |
| 標準ライブラリの各関数 | [libraries/README.md](../../libraries/README.md) |
| ネイティブ GUI | [gui.md](../gui.md) |
| デバッガ | [debugger.md](../debugger.md) |
| 版ごとの変更点 | `docs/REWIND_v*.md` |
