## 第24章 外の世界とつなぐ: データベース・通信・GUI

この章では、プログラムの外にあるデータベース、ネットワーク、画面とやりとりする方法を、一つずつ見ていきます。どれも、「巻き戻せない作用」を含みます。第12章の「戻せるものと戻せないものの境目」を、もう一度思い出しながら読んでください。

### 共通の約束

通信やデータベースの操作は、REWIND の外の相手とやりとりします。そのため、次の三つの約束に従います。

1. **開始は `external { }` の中、結果の待機は外**。領域の中では、操作を「依頼する」だけで、`await` はできません。依頼すると `Task` が返るので、領域を出てから `await` します。
2. **結果は二重の `Result`**。`Task<Result<T,E>>` を `await` すると、`Result<Result<T,E>,TaskError>` になります。外側は「タスクとして待てたか」、内側は「その API としての成否」です。
3. **外へ及んだ作用は、`revert` で消えない**。送信した通信も、データベースに書き込んだ行も、巻き戻せません。

どの操作にも、対応する効果（`network`、`db`、`gui`）と、`external`、`tasks` の許可が必要です。許可するときは、実行時に `--allow-effects external,db,tasks` のように指定します。

### データベース

`std.db` は、SQLite と PostgreSQL を扱います。SQLite の `":memory:"` は、そのプログラムの中だけにある、独立したデータベースを作ります。ファイル名を指定すると、プログラムが終わった後もデータが残ります。

次の例は、表を作り、値を挿入し、問い合わせで読み出します。

```rewind allow=external,db,tasks
import std.db as db;

fn take<T,E>(value: Result<T,E>) -> T effects {} {
    match move value {
        Ok(item) => { return move item; },
        Err(_) => { panic("データベース操作に失敗しました"); }
    }
}

fn main()->Int effects {external, db, tasks, output} {
    // 1. 接続する。依頼は external の中、待機は外
    var opening: Option<Task<Result<DbConnection, DbError>>> = None;
    external { opening = Some(db.sqlite(":memory:", false, 5000)); }
    match opening {
        None => { panic("接続の Task がない"); },
        Some(task) => {
            let connection = take(take(await task));

            // 2. 表を作る
            var writing: Option<Task<Result<Int, DbError>>> = None;
            external { writing = Some(db.execute(&mut connection, "CREATE TABLE scores (name TEXT, score INTEGER)", freeze(List<DbValue>()), 5000)); }
            match writing { Some(t) => { take(take(await t)); }, None => { panic("なし"); } }

            // 3. 値をパラメータとして渡して挿入する
            let values = List<DbValue>();
            values.push(DbValue::Text("Alice"));
            values.push(DbValue::Int(42));
            external { writing = Some(db.execute(&mut connection, "INSERT INTO scores VALUES (?1, ?2)", freeze(move values), 5000)); }
            match writing { Some(t) => { Out.println(take(take(await t))); }, None => { panic("なし"); } }

            // 4. 問い合わせて、結果を読む
            var querying: Option<Task<Result<DbCursor, DbError>>> = None;
            external { querying = Some(db.query(&mut connection, "SELECT name, score FROM scores", freeze(List<DbValue>()), 5000)); }
            match querying {
                None => { panic("なし"); },
                Some(t) => {
                    let cursor = take(take(await t));
                    var reading: Option<Task<Result<DbBatch, DbError>>> = None;
                    external { reading = Some(db.next(&mut cursor, 64, 1048576, 5000)); }
                    match reading {
                        None => { panic("なし"); },
                        Some(rt) => {
                            let batch = take(take(await rt));
                            Out.println(take(db.text(batch.rows.get(0), 0)));
                            Out.println(take(db.integer(batch.rows.get(0), 1)));
                        }
                    }
                }
            }
        }
    }
    // 5. 後始末の完了を待つ
    var cleanup: Option<Task<Result<Unit, DbError>>> = None;
    external { cleanup = Some(db.waitForCleanup(5000)); }
    match cleanup { Some(t) => { take(take(await t)); }, None => { panic("なし"); } }
    return 0;
}
```

```text
1
Alice
42
```

長く見えますが、手順は 5 つだけです。接続、表の作成、挿入、問い合わせ、後始末で、どの手順も「`external` の中で依頼して、外で `await` する」形をしています。`take(take(await t))` が二重になっているのは、先ほどの二重の `Result` のためです。

`DbConnection` や `DbCursor` は、持ち主が一人の資源です。スコープを出るときに解放が始まりますが、`waitForCleanup` で完了を待つと、後始末の失敗も確実に受け取れます。

#### 値は、パラメータで渡す

SQL に値を埋め込むとき、文字列を連結してはいけません。上の例の `?1`、`?2` のように、プレースホルダを使い、`DbValue` の列を別に渡します。文字列連結は、SQL インジェクションの原因になります。値で置き換えられない部分（表名や列名の動的な指定など）は、別に検査してください。

`NULL` は `DbValue::Null` で、整数の `0` や空文字列とは別物です。`db.integer` は整数のセル、`db.text` は文字列のセルを読みます。型が違うと、暗黙には変換されず、`Err` になります。

#### VM のコミットとデータベースのコミットは別

| 操作 | 対象 |
|---|---|
| `commit 名前` | REWIND の計算状態の保存 |
| `revert 名前` | 保存した計算状態への復元 |
| `publish` | 保留中のファイルや出力の確定 |
| データベースの `begin` / `commit` / `rollback` | データベース側のトランザクション |

`revert` は、データベースの `rollback` ではありません。データベースへ保存された行を、REWIND の `revert` で消すことはできません。複数の SQL をまとめて、成功か失敗のどちらかにしたいときは、データベースのトランザクションを使います（`std.dbTransaction`）。

PostgreSQL では、パスワードを含む接続文字列を `Secret` として渡します。TLS の証明書とホスト名は検証されます。自動の再接続や、書き込みの再試行は行われません。送信した後で結果が分からなくなったときは、データベースの実際の状態を確かめてから判断してください。

### HTTP の通信

`std.http` の `get` は、`Task<Result<HttpResponse,HttpError>>` を返します。次の例は、URL を引数で受け取り、応答の状態コードと本文を表示します。

```rewind http=1 allow=args,external,network,tasks
import std.http as http;
import std.bytes as bytes;

fn main()->Int effects {args, external, network, tasks, output} {
    let arguments = Args.all();
    if arguments.len() != 1 {
        Err.println("使い方: rewind run main.rw --allow-effects external,network,tasks -- URL");
        return 1;
    }
    var pending: Option<Task<Result<HttpResponse, HttpError>>> = None;
    external { pending = Some(http.get(arguments.get(0), 10000, 65536)); }
    match pending {
        None => { panic("Task がない"); },
        Some(task) => {
            match await task {
                Err(_) => { Err.println("取り消されました"); },
                Ok(response) => {
                    match response {
                        Err(e) => { Err.println(e.code + " (" + e.phase + ")"); },
                        Ok(r) => {
                            Out.println(r.status);
                            match bytes.decode(r.body) {
                                Ok(text) => { Out.println(text); },
                                Err(e) => { Err.println(e.code); }
                            }
                        }
                    }
                }
            }
        }
    }
    return 0;
}
```

```text
200
hello from server
```

この例は、手元で簡単なサーバーを立てて実行して確かめました。実際に使うときは、`rewind run main.rw --allow-effects args,external,network,tasks -- https://example.com/` のように、URL を渡します。

通信を扱うときの注意を挙げます。

- `get` の引数は、URL、期限（ミリ秒）、応答本文の最大バイト数です。期限は実時間で、第15章のタスクの `timeout`（論理的なステップ数）とは別の単位です。
- リダイレクトや再試行は、自動では行われません。
- 状態コードが `404` のような応答と、接続や TLS の失敗は、別々に扱います。`200` が返ったことは、本文が期待した形式であることを意味しません。
- HTTPS では、証明書の連鎖とホスト名が検証されます。診断を避けるために、検証を無効にする設計にはしないでください。
- 送信した後に時間切れになったときは、相手が処理したかどうかは分かりません。送信は `revert` で取り消せないので、再試行するときは、相手側の重複防止（冪等性のキーなど）も考えてください。
- 記録付きの実行では、通信の結果が記録され、再生では再送されません。再生に成功したことは、実際のサーバーに再び送れたという意味ではありません。

大きな本文を少しずつ受け取る `download`、`read` や、生の TCP を扱う `std.tcp`、HTTP サーバーを作る `std.httpServer`、`std.httpRouter` もあります。TCP の `read` は、メッセージの境界を保証しません。プログラムの側で、長さや区切りを決めて処理してください。

### ネイティブ GUI

`std.gui` は、ネイティブのウィンドウを作ります。GUI の状態を、三つの層に分けて考えると整理できます。

| 層 | 内容 |
|---|---|
| モデル | 数値や入力内容など、アプリケーションのデータ |
| View | 部品とその配置 |
| OS の画面 | `present` して `publish` した画面 |

モデルや View を変えても、`present` して `publish` するまで、画面は変わりません。

次の例は、ラベルとボタンを表示し、閉じる操作を待ちます。実行には `gui` の許可と、画面を表示できる環境が必要です。

```rewind gui=gui-minimal.events.json allow=gui
import std.gui as gui;

fn checked(value: Result<Unit, gui.GuiError>) -> Unit effects {} {
    match value {
        Ok(_) => {},
        Err(e) => { panic(e.code); }
    }
}

fn main()->Int effects {gui, output} {
    match gui.window("Hello GUI", 320, 180) {
        Err(e) => { panic(e.code); },
        Ok(view) => {
            checked(gui.label(&mut view, "message", "Hello, REWIND", 20, 20, 280, 40));
            checked(gui.button(&mut view, "close", "Close", 20, 90, 120, 40));
            checked(gui.present(&view));
            publish;
            var running = true;
            while running {
                match gui.nextEvent() {
                    Err(e) => { panic(e.code); },
                    Ok(event) => {
                        match gui.dispatch(&mut view, event) {
                            Some(gui.Action::Close) => { running = false; },
                            Some(gui.Action::Activate(id)) => { if id == "close" { running = false; } },
                            _ => {}
                        }
                    }
                }
            }
            checked(gui.close());
        }
    }
    return 0;
}
```

`gui.window` が View を作り、`gui.present` と `publish` で画面に表示します。`gui.nextEvent` は入力を待ち、`gui.dispatch` は座標やキーを、部品に対応する動作（`Action`）に変換します。

#### Undo を作る

GUI は、巻き戻しの力がよく見える場面です。次の例は、`Add` で数を増やし、`Undo all` で最初の状態に戻します。

```rewind gui=gui-counter.events.json allow=gui
import std.gui as gui;

fn checked(value: Result<Unit, gui.GuiError>) -> Unit effects {} {
    match value {
        Ok(_) => {},
        Err(e) => { panic(e.code); }
    }
}

fn main()->Int effects {gui, output} {
    match gui.window("カウンター", 360, 180) {
        Err(e) => { panic(e.code); },
        Ok(view) => {
            var count = 0;
            checked(gui.label(&mut view, "count", "0", 20, 20, 300, 40));
            checked(gui.button(&mut view, "add", "Add", 20, 80, 140, 40));
            checked(gui.button(&mut view, "undo", "Undo all", 180, 80, 140, 40));
            var running = true;
            commit ready;                       // 最初の状態を保存する
            checked(gui.present(&view));
            publish;
            while running {
                match gui.nextEvent() {
                    Err(e) => { panic(e.code); },
                    Ok(event) => {
                        match gui.dispatch(&mut view, event) {
                            Some(gui.Action::Activate(id)) => {
                                if id == "add" {
                                    count += 1;
                                } else if id == "undo" {
                                    revert ready;             // 状態を戻す
                                    gui.continueInput();      // すでに届いた入力は読み飛ばす
                                }
                                checked(gui.setText(&mut view, "count", count.format()));
                            },
                            Some(gui.Action::Close) => { running = false; },
                            _ => {}
                        }
                    }
                }
                if running {
                    checked(gui.present(&view));
                    publish;
                }
            }
            checked(gui.close());
            Out.println(count);
        }
    }
    return 0;
}
```

```text
1
```

この例の入力は、「Add を 2 回、Undo、Add、閉じる」です。`Undo all` で `count` が 0 に戻り、その後の `Add` で 1 になるので、最後に `1` が表示されます。

ポイントは二つです。

- `revert ready;` が戻すのは、`count` などの計算状態です。**画面は戻りません**。`present` と `publish` で、戻った状態を画面へ反映します。
- `gui.continueInput()` は、すでに届けた入力を読み飛ばします。これが無いと、巻き戻した入力の位置から、古いクリックをもう一度読んでしまい、Undo の直後に古い `Add` が効いてしまいます。

#### GUI の制限と環境

- 一画面の幅と高さは、それぞれ 64〜4096 ピクセルです。部品は最大 2048 個で、id は 128 バイト以内の空でない文字列です。
- 複数のウィンドウ（`std.guiWindows`）は、最大 16 枚です。
- Windows では Win32/GDI、Linux では X11 を使います。Linux では、`DISPLAY` と、X11 のライブラリ、フォントが必要です。
- 日本語入力には、OS 側の IME が必要です。変換中の未確定文字列は、OS 側の状態で、確定した文字だけがプログラムの入力になります。

入力は記録できます。`--gui-events EVENTS.json` を付けると、実際のウィンドウを開かず、ファイルのイベント列を与えて実行できます。この章の GUI の例も、この方法で確かめています。モデルの更新や終了の回帰テストに便利ですが、ネイティブの IME や描画の確認にはなりません。

フォーム（`std.guiForm`）、表（`std.guiTable`）、メニュー（`std.guiMenu`）、ファイル選択ダイアログ（`std.guiDialog`）といった部品も用意されています。通信やデータベースの処理を、GUI のイベントループと組み合わせるときは、チャネル（第15章）で結果を通知する構成にすると、画面と外部作用の境界を保てます。
