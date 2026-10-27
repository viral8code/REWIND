## 第15章 並行処理

### async fn と Task

`async fn` は、呼んでもすぐには実行されません。代わりに、実行の予約である `Task<T>` を返します。`await` で結果を待つと実行され、`spawn` で先に開始させることもできます。`await` は `Result<T, TaskError>` を返すので、タスクの失敗を呼び出し側で受け止められます。

```rewind
async fn compute(n: Int) -> Int effects {} {
    return n * 2;
}

fn main()->Int {
    let direct = compute(4);
    match await direct {
        Ok(v) => { Out.println(v); },
        Err(_) => { Out.println("失敗"); }
    }

    let started = spawn compute(21);
    match await started {
        Ok(v) => { Out.println(v); },
        Err(_) => { Out.println("失敗"); }
    }
    return 0;
}
```

```text
8
42
```

### 決定的な協調スケジューラ

REWIND のタスクは、OS のスレッドで並列に走るのではなく、**協調的なスケジューラ** の上で動きます。`await` などの切り替え点で、順番に実行が移ります。切り替えの順序は決まっているので、**同じ入力なら、実行するたびに同じ結果** になります。タイミングによって結果が変わる、というバグが生まれません。

これは巻き戻しや再生とも相性がよい設計です。タスクの状態も計算状態の一部なので、`revert` で戻せます。

### タスクへ渡せる値

タスクに渡す引数は、`Send` でなければなりません。変更できる値は `move` で渡すか、`freeze` した値を渡します。借用はタスクへ渡せません。

```rewind
async fn total(xs: List<Int>) -> Int effects {} {
    return xs.len();
}

fn main()->Int {
    let xs = [1, 2, 3];
    match await spawn total(move xs) {
        Ok(n) => { Out.println(n); },
        Err(_) => {}
    }
    return 0;
}
```

```text
3
```

### 失敗・取り消し・時間切れ

`TaskError` には三つの場合があります。

- `Failed(診断)`: タスクの中で致命的な失敗が起きた
- `Cancelled`: `cancel()` で取り消された
- `TimedOut`: `timeout(n)` で指定したステップ数を超えた

時間切れは、実時間ではなく、タスクが実行した **論理的なステップ数** で数えます。どんなに遅い環境でも、同じ結果になります。

```rewind
async fn spin() -> Int effects {} {
    while true {}
    return 0;
}

async fn divide(a: Int, b: Int) -> Int effects {} {
    return a / b;
}

fn main()->Int {
    let t = spawn spin();
    t.cancel();
    match await t {
        Err(TaskError::Cancelled) => { Out.println("取り消し"); },
        _ => {}
    }

    match await spin().timeout(20) {
        Err(TaskError::TimedOut) => { Out.println("時間切れ"); },
        _ => {}
    }

    match await spawn divide(1, 0) {
        Err(TaskError::Failed(d)) => { Out.println(d.code); },
        _ => {}
    }
    return 0;
}
```

```text
取り消し
時間切れ
DivisionByZero
```

`t.isDone()` は、待たずに完了しているかを調べます。

### チャネル

`Channel<T>(容量)` は、タスク間で値を受け渡す通路です。容量が 0 なら、送り手と受け手が出会うまで待ち合わせます。`send` と `receive` は `await` で待ち、`close()` で閉じます。閉じて空になると、`receive` は `Err` を返します。

```rewind
async fn producer(c: Channel<Int>) -> Unit effects {tasks} {
    for i in 0..3 {
        match await c.send(i) { Ok(_) => {}, Err(_) => {} }
    }
    c.close();
}

async fn consumer(c: Channel<Int>) -> Int effects {tasks} {
    var total = 0;
    while true {
        match await c.receive() {
            Ok(v) => { total += v; },
            Err(_) => { break; }
        }
    }
    return total;
}

fn main()->Int effects {tasks, output} {
    let c = Channel<Int>(1);
    let p = spawn producer(c);
    let r = spawn consumer(c);
    match await r {
        Ok(v) => { Out.println(v); },
        Err(_) => {}
    }
    return 0;
}
```

```text
3
```

### TaskGroup: 片付け忘れを防ぐ

`using group = TaskGroup();` で作ったグループに、`group.add(タスク)` で加えたタスクは、グループのスコープを出るときに必ず片付けられます。

- 正常にスコープを出るときは、加えたタスクの終了を **待ちます**。
- `Result` を返す関数が `Err` を返して抜けるときは、終わっていないタスクを **取り消します**。

```rewind
async fn work(n: Int) -> Unit effects {output} {
    Out.println("work ${n}");
}

fn run() -> Unit effects {tasks, output} {
    using group = TaskGroup();
    group.add(work(1));
    group.add(work(2));
    Out.println("スコープの終わり");
}

fn main()->Int effects {tasks, output} {
    run();
    Out.println("戻った");
    return 0;
}
```

```text
スコープの終わり
work 1
work 2
戻った
```

`spawn` したタスクを一度も `await` せずに `main` が戻ると、そのタスクは実行されないことがあります。結果を使うタスクは、必ず `await` するか、`TaskGroup` に入れてください。
