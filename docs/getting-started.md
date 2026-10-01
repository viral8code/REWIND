# REWIND 0.9.3を試す

Linux x86_64向けの開発版SDKです。Ubuntu 22.04以降などglibc 2.35以降の環境を対象とします。WindowsはWSL2のUbuntuから試せます。macOS・Windows native・ARM・Alpine向けバイナリは今回の配布には含みません。RustやJVMの導入は不要です。

## ダウンロードと展開

[GitHub Release v0.9.3](https://github.com/viral8code/REWIND/releases/tag/v0.9.3)から次を同じdirectoryへダウンロードします。

- `rewind-0.9.3-linux-x86_64.tar.gz`：実行ファイル、25moduleの標準ライブラリ、API文書、実行例、ライセンス
- `rewind-0.9.3-sdk.pub`：このリリースの署名検証用公開鍵
- `SHA256SUMS`：配布ファイルのSHA-256
- `GETTING_STARTED.md`、`BUILD_INFO.json`：この説明とビルド情報

Source codeを読みたい場合は`rewind-0.9.3-source.tar.gz`もダウンロードします。GitHubが自動生成するSource codeのzip/tar.gzには実行バイナリは含まれません。

```sh
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf rewind-0.9.3-linux-x86_64.tar.gz
SDK="$(pwd)/rewind-0.9.3-linux-x86_64"
PUBLIC_KEY="$(cat rewind-0.9.3-sdk.pub)"
"$SDK/bin/rewind" sdk-verify --sdk "$SDK" --public-key "$PUBLIC_KEY"
export PATH="$SDK/bin:$PATH"
```

`SHA256SUMS`の確認でSDK archiveと公開鍵の両方が`OK`となることを確認してください。署名鍵はリリースごとに生成し、秘密鍵は配布しません。公開鍵・checksumの信頼元はこのリポジトリのReleaseページです。SDKの場所を変えた場合はPATHも更新します。

## 最初のプログラム

```sh
mkdir hello-rewind
cat > hello-rewind/rewind.toml <<'TOML'
language = "0.9.3"
source_root = "."
entry = "main.rw"
effects = "output"
TOML
cat > hello-rewind/main.rw <<'RW'
fn main()->Int effects {output} {
    Out.println("Hello, REWIND!");
    publish;
    return 0;
}
RW
rewind update --root hello-rewind
rewind check --root hello-rewind
rewind run --root hello-rewind
```

`Hello, REWIND!`が表示されます。出力は`publish`時に確定します。標準ライブラリを使うprojectへは次で導入します。

```sh
rewind sdk-install --root hello-rewind --sdk "$SDK" --public-key "$PUBLIC_KEY"
```

## 標準ライブラリと入力を試す

同梱の最短距離CLIを、編集できるdirectoryへコピーします。

```sh
cp -R "$SDK/share/rewind/examples/shortest" ./rewind-shortest
rewind sdk-install --root ./rewind-shortest --sdk "$SDK" --public-key "$PUBLIC_KEY"
printf '4 3\n0 1 4\n0 2 1\n2 1 1\n' | rewind run --root ./rewind-shortest --task-steps 2000000
```

出力は4行です。

```text
0
2
1
unreachable
```

SDKの`share/rewind/doc/sdk-guide.md`とmoduleごとのAPI文書を参照してください。`share/rewind/examples/sum`も入力処理の小さな例です。

## 現在の制約

language 0.9.3のprojectで試してください。まだ開発版で、旧版のlock/artifact/replayは更新・再ビルド・再記録が必要です。入力やcollectionは容量・実行step・履歴memoryの上限があります。一般のstreaming I/O、高度なgraph/string/数値処理の一部は今後追加します。実装範囲はSDKの`share/rewind/doc/v0.9.3-status.md`に記載しています。
