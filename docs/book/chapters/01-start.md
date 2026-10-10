# 第1章 この本の読み方

REWINDは、計算途中の状態を保存し、戻し、まだ確定していない出力を取り消せるプログラミング言語です。この本はREWIND **2.0.0**の入門とリファレンスを兼ねています。将来の草案ではなく、公開済み処理系と同じ標準ライブラリの実装を対象にしています。

最初の数章は、ファイルを一つ作って実行するところから始めます。変数、条件分岐、関数、コレクションを覚えたら、巻き戻し、所有権、エラー処理に進んでください。その後は必要に応じて、文字列、ファイル、非同期処理、GUI、通信、DB、数値計算の章を選べます。最後のAPI編は標準ライブラリの全86公開モジュール・625関数を検索するためのものです。

## 実行例と説明用断片

「REWIND・実行例」と表示されるコードは、それぞれ独立したプログラムです。必要なimportや補助関数も含めています。コピーして`main.rw`に保存できます。「構文」や「断片」は、文法や一部分を示すもので、そのまま実行する前提ではありません。CLIの例では`rewind`がPATHに入っているものとします。

`assert_eq(a,b)`は等しいことを確認する命令です。失敗すると実行時診断になります。出力がない例も、assertが最後まで通れば成功しています。`take`という補助関数は例に定義した関数であり、予約語や組込み関数ではありません。

## HTMLの使い方

目次から章へ移動できます。検索欄で日本語、関数名、型名、エラーコードを探せます。このHTMLには本文、スタイル、検索処理をすべて含めているため、ダウンロード後はネット接続なしで読めます。ブラウザーの文字サイズ変更や検索も利用できます。

「印刷 / PDF保存」からブラウザーの印刷画面を開けます。保存先にPDFを選べば、全章を印刷向けのレイアウトで保存できます。画面での読書には、折り畳めるAPI実装と検索を使えるHTMLがおすすめです。紙面のページ番号に依存せず、章と節の名前で参照します。

## 本書が説明する保証

「巻き戻せる」とは、VMが管理する状態を巻き戻せるという意味です。すでに表示した文字、送信済みのHTTP要求、DBに確定した書込み、OSのIME変換途中の状態まで消せるという意味ではありません。本書では、その境界を具体例で繰り返し説明します。

API編のsignature、効果、generic boundはv2.0.0 SDKの機械生成文書から抽出しています。費用・失敗契約がソースに明記されている場合は原文も載せています。記述がない関数の計算量を、名前だけから推測して保証することはしません。必要な場合は、その場で開ける実装を確認してください。

# 第2章 インストールと最初の実行

## SDKを入れる

[GitHubのv2.0.0 Release](https://github.com/viral8code/REWIND/releases/tag/v2.0.0)からOSに合ったSDKを選びます。Linux x86_64とWindows x64の配布があります。ソースアーカイブにはソースが入っており、実行バイナリを試す場合はSDKを選んでください。

展開したSDKには`bin/rewind`、`bin/rewindc`があります。Windowsでは`rewind.exe`、`rewindc.exe`です。`bin`をPATHへ追加するか、実行ファイルのフルパスを指定します。

~~~sh
rewind --version
rewind --help
~~~

版が2.0.0であることを確認してください。チェックサムはダウンロードしたファイルの破損確認に使います。署名検証では、配布者の公開鍵を信頼できる経路で確認することが別途必要です。同じ配布ページに鍵があるだけでは、配布元を独立に確認したことにはなりません。

## Hello, REWIND

作業用フォルダーを作り、次をUTF-8で`main.rw`に保存します。

~~~rewind hello
Out.println("Hello, REWIND!");
publish;
~~~

~~~sh
rewind run main.rw
~~~

出力は`Hello, REWIND!`です。`Out.println`は改行を付けた出力をVM内で準備します。`publish`で外部へ確定します。プログラム終了を暗黙のpublishとして扱わないことが最初の大切な規則です。

~~~rewind unpublished
Out.println("まだ公開していません");
~~~

この例の標準出力は空です。処理を実行していないわけではなく、pending出力を確定していないためです。何も表示されないなら、publishの位置を確認してください。

## 単一ファイルのrootを決める

この本の単一ファイル例では、`--root`を付けずに実行します。CLIはsourceの親からrewind.tomlを探し、なければsourceの親をrootとして現行languageのstandalone modeを選びます。明示した`--root`にmanifestがない場合は旧来のproject読込み経路になり、現行構文やstdが使えないことがあります。rootを明示するprojectではlanguage="2.0.0"のmanifestを用意してください。

## コンパイルして渡す

~~~sh
rewind compile main.rw
rewind run main.rwc
~~~

標準では同じ場所に`main.rwc`を作ります。`rewindc main.rw`もコンパイル用の入口です。`rewind main.rwc`も実行方法です。成果物はソースなしでも実行できますが、runtimeは必要です。OSのネイティブ実行ファイルへ変換するコマンドではありません。

成果物、record、lockは正確なcompiler版と関係します。更新後は再コンパイル・再記録・必要なlock更新を行います。同じREWINDという名前だけで、全版の成果物を相互に実行できる保証はありません。

## Windowsで試す

PowerShellで直接指定する場合の形です。パスは実際の展開場所に置き換えます。

~~~powershell
& 'C:\Tools\rewind\bin\rewind.exe' --version
& 'C:\Tools\rewind\bin\rewind.exe' run .\main.rw
~~~

CLIへ渡すパスはOSの形式です。一方、File APIに渡すパスはroot相対の`/`区切りです。この二つを混同しないでください。コンソールプログラムだけならGUI用ディスプレイ環境は不要です。

## 練習

Helloの文字列を自分の名前に変えて実行してください。次にprintlnを二つ書き、最後に一回publishしてください。最後にpublishを削除し、標準出力が空になることを確認します。「出力を準備する」と「確定する」が別の処理であることが分かります。

# 第3章 変数・値・基本型

## letとvar

`let`はbindingの再代入を禁止し、`var`は許可します。bindingは名前と値の結び付きです。型が推論できる場合、型注釈は省略できます。

~~~rewind variables
let initial:Int = 10;
var score = initial;
score += 5;
Out.println(score);
publish;
~~~

出力は15です。`initial=20;`はletへの再代入なので拒否されます。`score`に後でStringを入れることもできません。varは型が何にでも変わる変数という意味ではありません。

Listなどはletで保持していても内容を更新できます。`let values=List<Int>(); values.add(1);`は正当です。bindingの再代入と、所有するデータへの変更は別の規則です。変更できないsnapshotにはFrozenを使います。

## 数値の型

| 型 | 表現 | 注意点 |
|---|---|---|
| Int | signed 64-bit | 範囲外の通常算術は診断になる |
| Float | IEEE 754 binary64 | 小数の丸め、NaN、無限大がある |
| BigInt | std.bigintの任意精度整数 | 予算と容量の範囲で正確に計算する |
| Decimal | std.decimalの十進数 | precision、scale、丸め方を選ぶ |

Intは−9,223,372,036,854,775,808から9,223,372,036,854,775,807までです。大きい数を扱うからといってFloatへ変えると、整数の下位桁が失われる場合があります。正確な整数にはBigInt、十進の丸めを指定する計算にはDecimalを検討してください。

~~~rewind literals
let thousand = 1_000;
assert_eq(thousand,1000);
assert_eq(0xff,255);
assert_eq(0b1010,10);
assert_eq(0o755,493);
let fraction:Float=1.25;
assert_eq(fraction,1.25);
assert_eq("\u{754c}","界");
~~~

underscoreは数字の間に置きます。型名と識別子は大文字・小文字を区別します。IntとFloatの間に暗黙変換を仮定しないでください。対象APIのchecked変換を使用します。

## String、Bytes、Unit

StringはUTF-8文字列、Bytesはbyte列です。`Bytes("text")`はUTF-8表現を作ります。一文字を必ず一byteと考えてはいけません。Unicodeの章でbyte・scalar・graphemeの違いを扱います。

Unitは値`()`を持つ型です。値を返す必要のない関数の戻り値に使います。Boolは`true`と`false`です。`if 1`のように整数を暗黙に真偽値として扱いません。

tupleには異なる型をまとめられます。`(Int,String)`の値は`(3,"three")`です。要素は`._0`、`._1`で読みます。Listは同じ要素型を持つ列、Mapはキーと値の型を指定する対応表です。

## コメントと文字列escape

行コメントは`//`から行末までです。`/* ... */`は入れ子にできます。Stringは二重引用符で囲み、改行`\n`、復帰`\r`、tab`\t`、引用符`\"`、backslash`\\`、NUL`\0`、Unicode scalar`\u{HEX}`を使えます。Unicode escapeは有効なscalarを1〜6桁のhexで指定します。surrogateは有効なscalarではありません。

# 第4章 演算と条件分岐

## 演算子の優先順位

弱い順は`||`、`&&`、`== !=`、`< <= > >=`、`+ -`、`* / %`です。括弧で意図を明示できます。`&&`と`||`は短絡評価し、必要がない右辺を評価しません。

~~~rewind arithmetic
assert_eq(2+3*4,14);
assert_eq((2+3)*4,20);
assert_eq(-7/3,-2);
assert_eq(-7%3,-1);
let divisor=0;
assert_eq(divisor!=0 && 10/divisor>1,false);
~~~

整数除算はゼロ方向へ切り捨てます。余りは被除数の符号に従います。負の剰余を0からm−1に揃えたい場合、通常の`%`の結果をそのまま使わず、std.modularの契約を確認します。bit操作はstd.bitsにあります。

## ifの書き方

条件の括弧は省略できます。ブロックの波括弧は必要です。

~~~rewind even-odd
var num = 13;
commit start;
Out.println("Even");
if num%2 == 1 {
    revert start;
    Out.println("Odd");
}
publish;
drop start;
~~~

出力はOddです。Evenはpublish前にrevertで取り消されています。ifの中から外側のcheckpointへ戻るこの形はv2.0.0で利用できます。revert後もifの実行位置は進み、次のprintlnを実行します。条件を最初から評価する動作ではありません。

通常の分岐なら次の方が簡単です。

~~~rewind plain-if
let num=13;
if num%2 == 0 {
    Out.println("Even");
} else {
    Out.println("Odd");
}
publish;
~~~

巻き戻しは、分岐をすべて置き換えるためのものではありません。多数の変更を試してからまとめて戻したい場合に便利です。二つの表示を選ぶだけならif/elseで十分です。

## 反復

`for i in 0..4`は0、1、2、3を順に使います。終端は含みません。whileは条件がtrueの間繰り返します。breakはループを終了し、continueは次の反復へ進みます。

~~~rewind loops
var sum=0;
for i in 0..4 {
    if i==2 {continue;}
    sum+=i;
}
while sum<10 {sum+=1;}
assert_eq(sum,10);
Out.println(sum);
publish;
~~~

無限ループでも実行予算は消費します。revertによって予算が回復することはありません。長い反復は停止条件と入力サイズを考えて設計します。

## 練習

1から100までの和を求めるには`1..101`を使います。3の倍数だけ加算する条件を加えてください。さらに、負の数の商と余りについて、上のassertの除数を変えて確かめてください。
