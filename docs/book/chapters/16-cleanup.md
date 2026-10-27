## 第16章 後始末: defer と using

### defer

`defer` は、「現在のスコープを出るときにやること」を予約します。`return` で出るときも、失敗で止まるときも実行されます。複数予約したときは、**予約したのと逆の順** に実行されます。

```rewind
fn work() -> Unit effects {output} {
    defer || -> Unit { Out.println("後始末 1"); };
    defer || -> Unit { Out.println("後始末 2"); };
    Out.println("作業");
}

fn main()->Int {
    work();
    return 0;
}
```

```text
作業
後始末 2
後始末 1
```

逆順になるのは、資源を確保した順の逆に解放するのが安全だからです。後から確保したものが、先に確保したものに依存していることがよくあります。

ループの本体で `defer` を使うと、反復のたびに実行されます。`defer 式;` の形で、式をそのまま予約することもできます。後始末が失敗したときは、元の失敗を主な診断として残し、後始末の失敗は原因（`causes`）に追加されます。元の失敗が隠されることはありません。

### using

`using 名前 = 式;` は、資源（ファイルハンドル、データベース接続、`TaskGroup` など）を、スコープを出るときに確実に閉じる宣言です。`defer` で閉じる処理を書くより、資源を作る行と閉じる動作が一か所にまとまります。

次は、ファイルを開いて途中から読む例です。関数を出るとき、`f` は自動で閉じられます。

```rewind
fn readTail() -> String effects {fileRead} {
    using f = File.openSnapshot("a.txt");
    f.seek(6);
    return f.read(5);
}

fn main()->Int effects {fileRead, fileWrite, output} {
    File.writeText("a.txt", "hello world");
    publish;
    Out.println(readTail());
    return 0;
}
```

```text
world
```

`File.openSnapshot` は、開いた時点のファイルの内容を読み出すハンドルを作ります。`seek` で位置を決め、`read(n)` で `n` 文字分を読みます。

### 巻き戻しと後始末

`using` や `defer` による後始末は、「スコープを抜けた」ことを意味するだけです。外部へ書いた内容が確定したことまでは意味しません。ファイルへ書き込んだ内容が外に出るのは、`publish` のときです（第12章）。
