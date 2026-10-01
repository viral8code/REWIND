# REWIND v0.9.3 Releaseの導入

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


旧版はmanifestが必要です。元のSDK同梱GETTING_STARTED.mdを参照してください。現行開発版の使い方は[入門](getting-started.md)を参照してください。
