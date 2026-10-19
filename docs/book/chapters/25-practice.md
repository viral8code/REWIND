## 第25章 実習: 在庫を「全部反映するか、一つも反映しないか」で更新する

最後に、これまでの章の内容を一つのプログラムにまとめます。題材は、在庫の更新です。標準入力から、1 行に 1 つずつ命令を読みます。

```
add apple 5
remove apple 3
```

命令を一つずつ反映していき、**途中で一つでも失敗したら、それまでの反映もすべて取り消す**、という動作にします。データが半端に更新された状態を、残さないためです。

### 設計を考える

最初に、部品を分けて考えましょう。

| 部品 | 役割 | 使う機能 |
|---|---|---|
| `Command` | 命令を表す型 | `enum`（第7章） |
| `parse` | 1 行を `Command` に変換する | `Result` と `?`（第10章） |
| `apply` | 在庫に命令を反映する | `&mut` の借用（第9章） |
| `have` | 在庫の数を調べる | `Map.get` と `Option`（第6章、第10章） |
| `main` | 読み込み、取り消し、報告 | `commit` と `revert`（第11章）、`publish`（第12章） |

計算の部分（`parse`、`apply`、`have`）は、外部とやりとりしないので、`effects {}` と書けます。入出力は `main` と `report` だけに集めます。こうして分けておけば、計算の部分を、入出力なしでテストできます（第22章）。

### プログラム

```rewind-exit 1 stdin="add apple 5\nremove apple 9\nadd pear 2\n"
enum Command {
    Add(String, Int),
    Remove(String, Int)
}

fn parse(line: String) -> Result<Command, String> effects {} {
    let parts = line.split(" ");
    if parts.len() != 3 {
        return Err("形式が違います: " + line);
    }
    let amount = parts.get(2).parseInt()?;
    let item = parts.get(1);
    if parts.get(0) == "add" { return Ok(Command::Add(item, amount)); }
    if parts.get(0) == "remove" { return Ok(Command::Remove(item, amount)); }
    return Err("未知の命令です: " + parts.get(0));
}

fn have(stock: &Map<String, Int>, item: String) -> Int effects {} {
    match stock.get(item) {
        Some(n) => { return n; },
        None => { return 0; }
    }
}

fn apply(stock: &mut Map<String, Int>, command: Command) -> Result<Unit, String> effects {} {
    match command {
        Command::Add(item, n) => {
            stock.set(item, have(stock, item) + n);
            return Ok(());
        },
        Command::Remove(item, n) => {
            let current = have(stock, item);
            if current < n {
                return Err(item + " の在庫が足りません");
            }
            stock.set(item, current - n);
            return Ok(());
        }
    }
}

fn report(stock: &Map<String, Int>) -> Unit effects {output} {
    for item in stock.keys() {
        Out.println("${item}: ${have(stock, item)}");
    }
}

fn main()->Int effects {input, output} {
    let stock = Map<String, Int>();
    stock.set("apple", 1);
    var count = 0;
    commit start;                                 // 反映前の状態を保存する
    while true {
        match In.readLine() {
            None => { break; },
            Some(line) => {
                let result = match parse(line) {
                    Ok(command) => { apply(&mut stock, command) },
                    Err(message) => { Err(message) }
                };
                match result {
                    Ok(_) => { count += 1; },
                    Err(message) => {
                        Out.println("失敗: " + message);
                        publish;                      // 理由は取り消されないよう、先に確定する
                        revert start;                 // 在庫への変更をすべてなかったことにする
                        Out.println("1 件も反映せず、在庫は元のままです");
                        report(&stock);
                        return 1;
                    }
                }
            }
        }
    }
    Out.println("${count} 件を反映しました");
    report(&stock);
    return 0;
}
```

```text
失敗: apple の在庫が足りません
1 件も反映せず、在庫は元のままです
apple: 1
```

標準入力に、`add apple 5`、`remove apple 9`、`add pear 2` の 3 行を渡した結果です。2 行目で在庫が足りなくなり、1 行目の `add apple 5` も取り消され、`apple` は最初の 1 のままです。`return 1` で終了したので、終了コードは 1 です。

すべての命令が成功すると、次のようになります。

```rewind stdin="add apple 5\nadd pear 2\nremove apple 3\n"
enum Command {
    Add(String, Int),
    Remove(String, Int)
}

fn parse(line: String) -> Result<Command, String> effects {} {
    let parts = line.split(" ");
    if parts.len() != 3 {
        return Err("形式が違います: " + line);
    }
    let amount = parts.get(2).parseInt()?;
    let item = parts.get(1);
    if parts.get(0) == "add" { return Ok(Command::Add(item, amount)); }
    if parts.get(0) == "remove" { return Ok(Command::Remove(item, amount)); }
    return Err("未知の命令です: " + parts.get(0));
}

fn have(stock: &Map<String, Int>, item: String) -> Int effects {} {
    match stock.get(item) {
        Some(n) => { return n; },
        None => { return 0; }
    }
}

fn apply(stock: &mut Map<String, Int>, command: Command) -> Result<Unit, String> effects {} {
    match command {
        Command::Add(item, n) => {
            stock.set(item, have(stock, item) + n);
            return Ok(());
        },
        Command::Remove(item, n) => {
            let current = have(stock, item);
            if current < n {
                return Err(item + " の在庫が足りません");
            }
            stock.set(item, current - n);
            return Ok(());
        }
    }
}

fn report(stock: &Map<String, Int>) -> Unit effects {output} {
    for item in stock.keys() {
        Out.println("${item}: ${have(stock, item)}");
    }
}

fn main()->Int effects {input, output} {
    let stock = Map<String, Int>();
    stock.set("apple", 1);
    var count = 0;
    commit start;
    while true {
        match In.readLine() {
            None => { break; },
            Some(line) => {
                let result = match parse(line) {
                    Ok(command) => { apply(&mut stock, command) },
                    Err(message) => { Err(message) }
                };
                match result {
                    Ok(_) => { count += 1; },
                    Err(message) => {
                        Out.println("失敗: " + message);
                        publish;
                        revert start;
                        Out.println("1 件も反映せず、在庫は元のままです");
                        report(&stock);
                        return 1;
                    }
                }
            }
        }
    }
    Out.println("${count} 件を反映しました");
    report(&stock);
    return 0;
}
```

```text
3 件を反映しました
apple: 3
pear: 2
```

### 読み解きのポイント

**`revert` の前に `publish` している理由。** `revert start;` は、`start` より後に作られた変数を消します。`message` もその一つで、`revert` の後では使えません。そこで、失敗の理由を、`revert` の前に出力して `publish` で確定しています。確定済みの出力は、`revert` で取り消されません。この「戻る前に、残したい情報を外へ出す」という順序は、チェックポイントを使うときの基本です。

**`count` を `commit` の前に宣言している理由。** `commit` の後で作った変数は、`revert` で消えます。ループの中で `revert` するので、ループの外にある変数は、`commit` より前に作っておく必要があります。

**`return 1` でも出力は公開される。** `main` が戻れば、保留中の出力は、戻り値にかかわらず公開されます。「反映せず、在庫は元のまま」の出力と、`report` の出力は、この自動公開で外に出ます。

**借用した `Map` を `for` で回す。** `report` は `&Map` を受け取りますが、`for item in stock.keys()` と、そのままキーを回せます。`keys()` が返すのはキーの列を進める反復子で、`Map` そのものは変更しません。回している間に `stock.set(...)` のような変更を書くと、借用中の変更として拒否されます。

### 発展: 取り組んでみよう

1. `remove` で在庫が 0 になった品目を、`Map` から消すように変えてみましょう（`stock.remove(item)`）。
2. `parse` と `apply` に対する `test fn` を書き、`rewind test` で確かめましょう。入出力を持たない関数なので、標準入力を用意する必要はありません。
3. 実行を `--record` で記録して、`rewind replay` で再生してみましょう。標準入力を渡さなくても、同じ結果が再現されるはずです。
4. 命令が千件を超えると、既定の実行ステップ（第23章）を使い切るかもしれません。`rewind profile` で、どこに仕事が集中しているか調べてみましょう。
