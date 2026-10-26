## 第14章 module とプロジェクト

### import

標準ライブラリや、別のファイルの宣言を使うには `import` します。ファイルの先頭に書き、基本的に別名（`as`）を付けて使います。

```rewind
import std.number as number;
import std.text as text;

fn main()->Int {
    Out.println(number.integer("ff", 16));
    Out.println(text.length("日本語"));
    return 0;
}
```

```text
Ok(255)
3
```

| 書き方 | 意味 |
|---|---|
| `import std.number as number;` | 標準ライブラリの module を別名で使う |
| `import util as u;` | 同じプロジェクトの `util.rw` を別名で使う |
| `import util.{double, total};` | 名前を選んで直接取り込む |
| `import util.{double as dbl};` | 取り込む名前に別名を付ける |

別名を付けない `import util;` では、`util` という名前は使えるようになりません。必ず `as` か `{ }` を使います。

### 公開する名前を決める

ファイルの中の宣言は、既定では他のファイルから見えません。`pub` を付けた宣言だけが公開されます。公開できるのは、`fn`、`struct`、`record`、`enum`、`type`、`trait`、`const` です。公開する関数には、戻り値の型が必要です。

公開した `struct` のフィールドに `private` を付けると、内部の表現を隠したまま、公開した関数を通して扱わせられます。次は、二つのファイルから成る例です。

```
// util.rw
pub struct Account { private total: Int }
pub fn open() -> Account effects {} { return Account(0); }
pub fn deposit(a: &mut Account, n: Int) -> Unit effects {} { a.total += n; }
pub fn balance(a: &Account) -> Int effects {} { return a.total; }
pub const LIMIT: Int = 99;

// main.rw
import util as u;
fn main()->Int {
    let a = u.open();
    u.deposit(&mut a, 5);
    Out.println(u.balance(&a));   // 5
    Out.println(u.LIMIT);         // 99
    // a.total は module の外から読めない
    return 0;
}
```

公開されていない関数を呼ぶと、`unknown name` と報告されます。非公開のフィールドを読むと `field total is private to its module` です。

こうして、「内部の都合を隠して、外には決まった窓口だけを見せる」設計ができます。`Account` の内部表現を変えても、公開した関数が同じ働きをする限り、使う側のコードは変わりません。

### module の中の状態

module の最上位に書けるのは宣言だけです。`const` を除いて、module が **状態を持つことはできません**。状態は、値として作って、引数や戻り値で受け渡します。巻き戻しの対象が、いつも「いま動いている関数の中の値」に限られるのは、このためです。

### プロジェクトにする

ファイルが複数あるプログラムや、権限・依存関係を固定したいプログラムは、ディレクトリに `rewind.toml` を置いてプロジェクトにします。

```toml
language = "4.0.0"
source_root = "src"
entry = "main.rw"
effects = "output,fileRead"
```

| キー | 意味 |
|---|---|
| `language` | 言語バージョン |
| `source_root` | ソースのあるディレクトリ |
| `entry` | 実行を始めるファイル |
| `effects` | プログラムに許可する効果 |
| `[assets]` | 実行に同梱するファイル |
| `[dependencies]` | 依存パッケージ。`rewind update` が `rewind.lock` を書く |

マニフェストが近くにあると、その `language` と `effects` が使われます。単体のファイルを実行するときは、マニフェストも `rewind compile` も要りません。
