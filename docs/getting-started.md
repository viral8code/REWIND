# REWIND 1.7.0を試す

1.7.0はLinux x86_64とWindows x64用SDKを提供します。簡易CLI、Checkpoint/publishと呼び出しスタック付き診断を利用できます。旧Releaseは[0.9.3用の説明](getting-started-v0.9.3.md)を参照してください。

## Linux SDKをダウンロードする

[Release v1.7.0](https://github.com/viral8code/REWIND/releases/tag/v1.7.0)から次を同じdirectoryへ保存します。

- `rewind-1.7.0-linux-x86_64.tar.gz`：rewind/rewindc、38 module、文書、例
- `rewind-1.7.0-sdk.pub`：このreleaseの公開鍵
- `SHA256SUMS`、`BUILD_INFO.json`：checksumと最低glibc等のbuild情報

```sh
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf rewind-1.7.0-linux-x86_64.tar.gz
SDK="$(pwd)/rewind-1.7.0-linux-x86_64"
PUBLIC_KEY="$(cat rewind-1.7.0-sdk.pub)"
"$SDK/bin/rewind" sdk-verify --sdk "$SDK" --public-key "$PUBLIC_KEY"
export PATH="$SDK/bin:$PATH"
rewind --version
```

SDK archiveと公開鍵の両方がchecksum検証でOKになることを確認します。公開鍵の信頼元はこのrepositoryのReleaseページです。実行する利用者にRustやJVMは不要です。Windows版は次の節を参照してください。macOS/ARM/Alpine用バイナリは含みません。最低glibcはBUILD_INFO.jsonを確認してください。

## Windows SDKをダウンロードする

[Release v1.7.0](https://github.com/viral8code/REWIND/releases/tag/v1.7.0)から次を同じfolderへ保存します。Windows x64（MSVC、Windows 10以降）向けで、CIはWindows Server 2022上で実行しています。

- `rewind-1.7.0-windows-x86_64.zip`
- `rewind-1.7.0-windows-x86_64-sdk.pub`
- `SHA256SUMS.windows`、`BUILD_INFO.windows.json`

PowerShellでchecksumを確認し、展開してPATHへ追加します。

```powershell
$expected = @{}
Get-Content .\SHA256SUMS.windows | ForEach-Object {
    $hash, $name = $_ -split '  ', 2
    $expected[$name] = $hash
}
foreach ($name in @('rewind-1.7.0-windows-x86_64.zip', 'rewind-1.7.0-windows-x86_64-sdk.pub')) {
    if ((Get-FileHash $name -Algorithm SHA256).Hash.ToLower() -ne $expected[$name]) {
        throw "Checksum mismatch: $name"
    }
}
Expand-Archive .\rewind-1.7.0-windows-x86_64.zip -DestinationPath .
$sdk = Join-Path (Get-Location) 'rewind-1.7.0-windows-x86_64'
$publicKey = (Get-Content .\rewind-1.7.0-windows-x86_64-sdk.pub -Raw).Trim()
& "$sdk\bin\rewind.exe" sdk-verify --sdk $sdk --public-key $publicKey
$env:Path = "$sdk\bin;$env:Path"
rewind --version
```

RustやJVMのインストールは不要です。このPATH変更は現在のPowerShellに適用されます。継続して使う場合はWindowsの環境変数設定へSDKのbinを追加します。公開鍵はplatformごとに異なるため、Windows ZIPにはWindowsの公開鍵を使います。

## sourceからビルドする

`rewind-1.7.0-source.tar.gz`、またはrepositoryの`codex/develop`を使います。Rust 1.98.1と依存lockを固定しています。

```sh
cargo build --release --locked
export PATH="$(pwd)/target/release:$PATH"
rewind --version
rewind --help
```

GitHubが自動生成するSource codeのzip/tar.gzには実行バイナリは含まれません。SDKの場所を変えた場合はPATHを更新します。

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

成果物へ権限を記録しても、実行側の許可は必要です。manifestが近傍にある場合は、そのlanguage/effects/依存/lockを優先します。既存projectを新仕様に移す場合はlanguageを1.7.0へ変更して`rewind update --root DIR`を実行し、artifact/replayを作り直します。署名付き外部dependencyの導入は既存のsdk-install/updateを使います。

```sh
rewind run main.rw --record trace.json
rewind replay trace.json --root .
```

実行・履歴・stream/collectionには上限があります。`--steps N` / `--native-work N`で累積実行予算を指定します。Linuxのプロセスaddress-spaceは既定2,048 MiBで、`--memory-mib N`で指定できます。上限超過によるOS/allocatorの終了は言語のResultとして回復できません。保証範囲と未提供機能は[1.0仕様](REWIND_v1.0.md)を参照してください。

## エラーとエディタ

v1.2はエラーコード、発生位置、callerと対処ヒントを表示します。`--diagnostic-format json`はstderrへJSONを出します。整形は`rewind fmt main.rw`、stdio LSPの起動は`rewind lsp --root DIR`です。manifestなしでもstd importとincremental診断・整形を利用できます。

構文と動作は[言語リファレンス](language-reference.md)、変更点とWindowsの制限は[1.5仕様](REWIND_v1.5.md)を参照してください。WindowsではVM予算は有効ですがLinuxの`--memory-mib`によるOS上限は提供しません。

## ネイティブ GUI を試す

SDK の `share/rewind/examples/gui/main.rw` をコピーし、`rewind run main.rw --allow-effects gui` を実行します。Linux は X11 の表示環境と core fonts が必要です。Windows は OS 標準の Win32 を利用します。[GUI guide](gui.md) に操作・Undo・入力テストの手順があります。

1.3 では初期状態の予約 Checkpoint `begin` が自動作成されます。`revert begin;` で未公開処理と作業データをすべて破棄できます。確定済み出力は保持します。
