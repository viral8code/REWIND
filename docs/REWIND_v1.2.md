# REWIND 1.2.0

v1.2はCheckpointの条件分岐内での使いやすさ、数値・文字列の表記、コメント、CLIヘルプを改善します。Linux x86_64とWindows x64の署名付きSDKを配布します。開発branchはcodex/developです。

## Checkpointと条件分岐

外側でcommitしたCheckpointへ、同じ生存中の関数呼び出しのif・while・入れ子ブロックからrevertできます。値・heap・pending I/O・cleanup登録をCheckpoint時点へ戻し、現在の実行位置とブロックの枠を維持します。Checkpoint後のbindingとcleanup登録は破棄し、復元後に新しいbindingやdeferを追加できます。break・continue・returnや通常のブロック終了も維持します。publish済みの出力は破棄・再送しません。

例えば`var num=13; commit start; Out.println("Even"); if num%2==1 { revert start; Out.println("Odd"); } publish;`はOddだけを出力します。num=12ならEvenです。

スコープに実行時の識別情報を持たせ、保存したスコープが現在のスコープ列の先祖であることを検査します。深さが同じでも、既に終了した兄弟ブロックや以前のloop iterationのスコープへrevertすることはできません。異なる関数呼び出し、stackやbranch contextが不適合の場合もInvalidContinuationです。診断にresumeとの違いを説明するhintを追加します。resumeは引き続き保存した制御位置も復元します。スコープの識別情報はtask context・branch・Checkpointと共に保存し、識別番号の発行は巻き戻しません。

## 数値リテラル

10進整数・Floatの数字の間に`_`を使えます。`1_000_000`、`1_2.5_0e+0_1`等です。整数には`0x`/`0X`の16進、`0b`/`0B`の2進、`0o`/`0O`の8進表記を追加します。`0xff`、`0b1010_0101`、`0o755`はそれぞれ255、165、493です。

Intは引き続きsigned 64-bitです。負号を含めた最小値`-0x8000_0000_0000_0000`も扱え、literal patternとrange patternでも同じ表記を使えます。負のrange終端にも対応します。正の0x8000_0000_0000_0000等はIntegerOverflowです。bit patternとして勝手に負数へ変換しません。

`_`は数字同士の間だけに置き、prefix直後・末尾・連続配置は拒否します。基数にない数字や未完のexponentはInvalidNumericLiteralです。元のtoken表記とsource位置を保持します。既存の演算子、整数のゼロ除算・overflow規則を変更しません。

## コメントと文字列

`/* ... */`のblock commentを追加し、入れ子を許可します。既存の`//`は行コメントのままです。comment内の文字列・brace・line comment記号をコードとして扱いません。未完のblock commentは開始位置にUnterminatedCommentを返します。

Stringのescapeに`\0`と`\u{HEX}`を追加します。Unicodeは1〜6桁のhexで有効なscalarを表し、surrogate・0x10ffff超過・空・未閉鎖はInvalidUnicodeEscapeです。`"\u{754c}\u{1f600}"`は`"界😀"`と同じ値です。StringのbyteLen/charLen等の既存単位を変更しません。

formatterはcomment内のquote/braceを無視してindentを計算します。複数行commentの継続行とString内部の空白を保持します。任意のコードを行分割するformatterには変更しません。

## CLIと互換性

`rewind run|compile|check|test|fmt|lsp|replay --help`および`-h`にコマンド別の使用方法を表示します。helpでsourceを実行したりfileを書き換えたりしません。`rewindc --help`はcompileの説明です。`--`以降の`--help`はapplicationへ渡す引数として保持します。

compiler・language・bundled std・lockを1.2.0へ揃えます。1.1以前のlanguage modeの検査規則も維持し、追加の字句表記は旧modeでも使用できます。公開library APIは1.0の34module baselineでbreaking changeがないことを確認します。既存projectはlanguageを1.2.0へ更新し、rewind update/check/testで検証します。artifact・trace・SDKは1.2で再生成してください。compiler内部形式の版跨ぎ互換性は保証しません。

LinuxとWindowsの回帰テスト、標準契約、SDK署名、API、展開後実行、record/replay、source-free実行が通過してから両SDKを公開します。Windowsのstatic CRT・8MiB stackと、各platformの既存予算・配布制限を継承します。process/network、project管理の拡張は今回の対象に含めません。
