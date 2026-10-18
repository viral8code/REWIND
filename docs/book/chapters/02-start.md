## 第2章 準備と最初のプログラム

### 処理系を用意する

REWIND の処理系は `rewind` という一つの実行ファイルです。構文の検査、実行、テスト、整形といった作業は、すべてこのコマンドで行います。

GitHub のリリースページから、お使いの OS 用の SDK（Linux x86_64 または Windows x64）を取得して展開し、`bin` ディレクトリを `PATH` に追加してください。署名とチェックサムの確認手順は [getting-started.md](../getting-started.md) にあります。次のコマンドで版が表示されれば準備は終わりです。

```sh
rewind --version
```

ソースコードからビルドすることもできます。その場合は、リポジトリで `cargo build --release --locked` を実行し、できた実行ファイルを使います。

### Hello, REWIND

空のフォルダーに `main.rw` というファイルを作り、次の内容を UTF-8 で保存します。

```rewind
fn main()->Int {
    Out.println("Hello, REWIND!");
    return 0;
}
```

```text
Hello, REWIND!
```

実行します。

```sh
rewind run main.rw
```

この短いプログラムに、REWIND の基本がいくつか入っています。

- ソースファイルに書けるのは **宣言だけ** です。関数、型、定数などを宣言し、実行される文は関数の中に書きます。
- 実行は `fn main` から始まります。`main` は `Int` を返し、`0` は正常終了を表します。
- `Out.println` は、標準出力へ 1 行出力します。ただし、この出力はすぐには画面に出ません。いったん **保留中** になり、`main` が戻るときにまとめて外へ出ます。第12章で詳しく説明します。

### 実行のしかた

`rewind run` のほかに、よく使うコマンドを挙げておきます。

| やりたいこと | コマンド |
|---|---|
| 実行せずに、型や所有権の誤りだけ調べる | `rewind check main.rw` |
| ソースの形を整える | `rewind fmt main.rw` |
| 実行用のファイル（`main.rwc`）を作る | `rewind compile main.rw` |
| 作ったファイルを実行する（ソースは不要） | `rewind run main.rwc` |
| 使い方を見る | `rewind --help` |

プログラムに引数を渡すときは、`--` の後ろに書きます。たとえば `rewind run main.rw -- a b` です。

### エラーの読み方

間違いがあると、`rewind` は実行する前に止まり、場所と原因を表示します。わざと間違えてみましょう。

```rewind-error MissingEffect
fn main()->Int {
    let x: Int = 3;
    Out.println(x);
    return 0;
}

fn helper() -> Unit effects {} {
    Out.println("hi");
}
```

`helper` は「外部とのやりとりをしない」と宣言（`effects {}`）しているのに、出力しています。REWIND はこれを実行前に見つけて拒否します。エラーの表示は次のような形です。

```text
error[MissingEffect]: helper: undeclared effect output
  --> main.rw:7:1
  help: Two separate steps: add the effect to the function's effects clause (a source change), and let the person running the program grant it (--allow-effects or the manifest effects). A declaration alone grants nothing.
caused by[MissingEffect]: operation requires effect output
  --> main.rw:8:5
```

エラー名（`MissingEffect`）、場所（ファイル名:行:列）、対処のヒント（`help`）が並び、原因となった操作が `caused by` に続きます。ここでは、8 行目の出力が原因だと分かります。この本では、エラーを「怖いもの」ではなく、間違いを早く教えてくれる仕組みとして扱います。`effects` の意味は第13章で説明するので、いまは「宣言と実際が食い違っていた」と読めれば十分です。
