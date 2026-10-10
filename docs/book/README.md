# REWIND 2.0.0 入門とリファレンス

読む場合は **[HTML](index.html)** または **[PDF](REWIND-2.0.0-book.pdf)** を開いてください。
HTMLは単独ファイルで、ネット接続・サーバー・外部JavaScript・CDNを必要としません。
全文検索、目次、実装の折り畳み、実行例のコピーと.rw保存、印刷に対応します。
PDFにはクリックできる目次と見出しのしおりがあります。

対象は公開済みcompiler/language 2.0.0です。32章、60実行例、全86公開標準モジュール・625関数を収録します。
本文の仕様は実装と照合し、APIは公開SDKから抽出しています。対象実装commitとSDKのSHA-256は本の「出典と検証」に記載しています。

## 編集

- `chapters/*.md`: 入門・仕様・実用分野の原稿。編集用のMarkdownで、読者向け成果物はHTML/PDFです。
- `worked-examples.json` と `examples/*.rw`: 完全な実習例と解説。
- `examples.json`: 標準出力、必要な効果、引数、fixture、予算、保存fileの期待値。
- `api-snapshot.json`: 公開SDK由来のAPI、引数名付き宣言、source hash。
- `modules.json`: 各標準モジュールの日本語解説。
- `style.css` / `reader.js`: HTML内へ埋め込む表示・検索・印刷処理。
- `validation.json` / `browser-validation.json`: ローカル検証結果。

## 再生成と検証

REWIND本体は2.0.0のバイナリを使います。単一source検証にmanifestなしの`--root`を付けません。
検証scriptは一時directory内で実行し、File操作やHTTP fixtureをrepositoryから隔離します。

~~~sh
python -m pip install -r docs/book/requirements.txt
python scripts/check-book-examples.py --rewind /path/to/rewind
python scripts/build-book.py
~~~

全60例のsource実行とsourceを削除したartifact実行を比較します。
巻き戻し、begin、GUI、HTTP、SQLite、数値返却Listの8例ではartifactのcompact recordをreplayします。
GUI試験はfixtureを使います。今回の例の実行確認はLinux x86_64であり、Windowsで全60例を再実行したという記録ではありません。

ブラウザー検証とPDF生成にはPythonのPlaywright、Chromium、日本語fontを用意します。Playwrightのバージョンは`browser-validation.json`に記録します。

~~~sh
python scripts/check-book-browser.py --chromium /path/to/chromium --pdf
~~~

環境のブラウザーポリシーがfile URLを拒否するため、検証ではローカルHTMLをブラウザーへ直接読み込ませます。
内容は同じ生成HTMLです。ネットワーク要求が発生しないことも検査します。
ローカル検証scriptはGitHub ActionsやReleaseを起動しません。

## SDKからAPIを更新する

署名・checksumを検証済みの2.0.0 SDK archiveを指定します。

~~~sh
python scripts/import-book-api.py /path/to/rewind-2.0.0-linux-x86_64.tar.gz
~~~

compiler/languageが2.0.0であること、公開宣言とAPI関数の一致、全標準sourceとrepositoryの一致を検査します。
HTML生成時にもsource hashを再確認します。
次版向けへ更新する場合、単にversionを置き換えるのではなく、本文・API・期待値・検証の対象を一緒に見直してください。
