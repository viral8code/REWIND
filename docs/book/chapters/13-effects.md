## 第13章 効果と許可

### 効果とは何か

**効果**（effect）は、関数が外の世界に対して行う操作の種類です。画面への出力、ファイルの読み書き、現在時刻の取得、通信などがあたります。REWIND では、関数が行う効果を `effects { ... }` で宣言します。宣言に無い効果を使うプログラムは、実行する前に拒否されます。

```rewind-error MissingEffect
fn greet() -> Unit effects {} {
    Out.println("hi");        // output を宣言していない
}

fn main()->Int {
    greet();
    return 0;
}
```

何もしない関数は `effects {}` と書きます。これは、「この関数は外部とやりとりしない。同じ引数なら同じ結果になる」という約束です。この約束があるおかげで、巻き戻しや再生をしても安全だと言えます。

### 効果の一覧

| 効果 | できること |
|---|---|
| `output` | `Out`、`Err` への出力 |
| `input` | `In` からの入力 |
| `args` | `Args` による起動引数の読み取り |
| `env` | `Env` による環境変数の読み取り |
| `locale` | `Locale` による地域設定の読み取り |
| `clock` | `Time` による時刻の観測 |
| `random` | `Random` による乱数 |
| `tasks` | タスクとチャネルの作成・待機 |
| `fileRead` | `File` の読み取り、`Directory` の一覧 |
| `fileWrite` | `File`、`Directory` の書き込み・作成・削除 |
| `external` | `external { }` での外部操作 |
| `live` | `external live { }` での、再利用しない物理操作 |
| `network` | HTTP、TCP などの通信 |
| `db` | SQLite、PostgreSQL |
| `gui` | ネイティブ画面 |

この表にない名前を宣言すると `unknown effect` になります。

### 効果は呼び出し元へ伝わる

関数が呼ぶ関数の効果は、呼ぶ側も宣言しなければなりません。

```rewind
fn greet() -> Unit effects {output} {
    Out.println("hi");
}

fn twice() -> Unit effects {output} {
    greet();
    greet();
}

fn main()->Int effects {output} {
    twice();
    return 0;
}
```

```text
hi
hi
```

どこかで宣言が足りないと、不足している箇所と、その原因となった呼び出しの経路が、`caused by` として表示されます。どこで宣言を足せばよいかが一目で分かります。

`main` だけは `effects` を省略でき、そのときは本体が必要とする効果が自動で求められます。ここまでの例で `main` に `effects` を書かなかったのは、そのためです。

### 宣言と許可は別

効果の宣言は、「この関数は何をするか」の表明です。それを実際に **許す** かどうかは、プログラムを **実行する人** が決めます。宣言だけでは何も許可されません。

| 実行のしかた | 許可される効果 |
|---|---|
| 単体のファイルの `rewind run` | 既定で `input`、`output`、`args`、`locale`、`random`、`tasks` |
| さらに許可する | `--allow-effects fileRead,fileWrite` のように、カンマ区切りで指定する |
| プロジェクト | `rewind.toml` の `effects` |

たとえばファイルを書くプログラムは、`fileWrite` を宣言したうえで、次のように実行します。

```sh
rewind run main.rw --allow-effects fileRead,fileWrite
```

許可が無いと、`effect gui is not allowed` のように拒否されます。他人が書いたプログラムを実行するときも、何を許すかを自分で決められるので、安心して試せます。

環境変数は、効果 `env` に加えて **変数名ごとの許可** が必要です。`--allow-env 名前` で許可します。値を診断や記録に残したくないときは `--secret-env 名前` を使います。

### ファイルのパス

`File` と `Directory` のパスは、実行の **root** からの相対パスです。root は、単体のファイルではそのファイルのあるディレクトリで、プロジェクトでは `--root` で指定した場所です。区切りには、OS に関係なく `/` を使います。絶対パス、`..`、root の外を指すシンボリックリンクは、`InvalidPath` で拒否されます。

```rewind
fn main()->Int effects {fileRead, fileWrite, output} {
    File.writeText("state.txt", "saved");
    Out.println(File.readText("state.txt"));    // 保留中の書き込みも読める
    Out.println(File.readText("missing.txt"));
    Out.println(File.readText("../escape.txt"));
    return 0;
}
```

```text
Ok(saved)
Err(NotFound: missing.txt (file not found: missing.txt))
Err(InvalidPath: ../escape.txt (invalid path: ../escape.txt))
```

読み取りの失敗は `Result` で返ります。書き込みは保留中になり、`publish` か `main` の終了で、実際のファイルに反映されます。

### Secret: 値を伏せる

`secret(値)` は、値を `Secret<T>` で包みます。`Secret` を出力すると `<redacted: Secret>` と表示され、診断や記録にも値が残りません。取り出すときは `reveal` を使います。パスワードやトークンを扱うときの、うっかり漏らさないための仕組みです。暗号化や、外部への送信の防止までは保証しません。
