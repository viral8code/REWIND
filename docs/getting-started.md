# REWIND 0.9.4を試す

v0.9.4は`codex/develop`の開発版です。公開済み[Release v0.9.3](https://github.com/viral8code/REWIND/releases/tag/v0.9.3)には以下の簡易CLIと新しいpublish規則は含まれません。[旧Releaseの導入](getting-started-v0.9.3.md)と区別してください。

## バイナリを用意する

リポジトリの`codex/develop`でRustのbuild環境から次を実行します。

```sh
cargo build --release --locked
export PATH="$(pwd)/target/release:$PATH"
rewind --version
rewind --help
```

ビルド済みSDKを受け取った場合は、その`bin`をPATHへ追加します。実行する利用者にRustやJVMは不要です。SDKの署名・checksumは配布元の公開鍵で検証してください。現在の配布targetはLinux x86_64で、macOS/Windows native/ARM向けの動作保証はまだありません。

## 最初のプログラム

新しいdirectoryで`main.rw`を作ります。

```rewind
Out.println("Hello, REWIND!");
publish;
```

```sh
rewind run main.rw
rewind compile main.rw
rewind run main.rwc
```

compileは`main.rwc`を生成し、runはその成果物をsourceなしでも実行できます。`rewindc main.rw`と`rewind main.rwc`も同じ操作です。run main.rwに事前compileは不要です。manifest/lock/update/--rootも不要で、内部cacheを`.rewind`へ作成します。relative importとFile pathの基準はsource directoryです。

## Checkpointとpublish

```rewind
var score = 10;
commit test1;
score = 99;
Out.println(score);
commit test2;
publish;
revert test1;
Out.println(score);
publish;
```

出力は次の通りです。

```text
99
10
```

commitはCheckpointの保存です。revertは計算状態と未公開I/Oを戻します。publishはpending I/Oを外部へ確定します。確定済みの99は消えず、再送もしません。最初のpublishを省くと99の未公開出力は捨てられ、10だけ表示されます。戻ってからの処理もpublishで確定するまで外部には出ません。

## stdと入力

単一ファイルでもstdを直接importできます。

```rewind
import std.number as number;
fn main()->Int effects {input,output} {
    match In.readLine() {
        None=>{return 0;},
        Some(text)=>{match number.decimal(text) {
            Ok(n)=>{Out.println(n);publish;return 0;},
            Err(_)=>{return 2;}
        }}
    }
}
```

```sh
printf '42\n' | rewind run main.rw
```

Intはchecked signed64、Floatはbinary64、StringはUTF-8、Bytesはbyte列です。List/Map/Option/Result、record/enum、generic/trait、closure、if/while/for、再帰、matchがあります。letはbindingの再代入を禁止し、varは許可します。mutable ownerはmove/borrow/freezeの規則に従います。byte chunkとpureなparser/bufferは`In.readChunk`、`Out.writeBytes`、`std.stream`を使います。[一覧と容量・費用](../libraries/README.md)を参照してください。

## 権限とproject

簡易モードの既定effectはinput/output/args/locale/random/tasksです。fileRead/fileWrite/env/clockは明示します。

```sh
rewind run main.rw --allow-effects fileRead,fileWrite
rewind compile main.rw --allow-effects fileRead,fileWrite
rewind run main.rwc --allow-effects fileRead,fileWrite
```

成果物へ権限を記録しても、実行側の許可は必要です。manifestが近傍にある場合は、そのlanguage/effects/依存/lockを優先します。既存projectを新仕様に移す場合はlanguageを0.9.4へ変更して`rewind update --root DIR`を実行し、artifact/replayを作り直します。署名付き外部dependencyの導入は既存のsdk-install/updateを使います。

```sh
rewind run main.rw --record trace.json
rewind replay trace.json --root .
```

実行step、履歴memory、stream/collectionには上限があります。heap回収とnative workの完全な予算は未実装です。[実装範囲](v0.9.4-status.md)、[次の計画](REWIND_v0.9.5.md)を参照してください。
