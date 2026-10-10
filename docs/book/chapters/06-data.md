# 第21章 SQLite・PostgreSQL・transaction

## 接続と所有権

std.dbはSQLiteとPostgreSQLを提供します。db、external、tasksを許可し、external内で接続Taskを作り、外でawaitします。DbConnection、DbStatement、DbCursorはscope所有のaffine resourceです。

SQLiteのパスはroot相対です。`:memory:`は独立したmemory DBを作ります。終了すればその内容は保持されません。file名を指定したDBは、REWINDを終了しても保存した内容が残ります。SQLiteへ書くことはFile.writeTextのpending操作ではありません。

実習のSQLite例ではtable作成、parameter付きINSERT、cursorでSELECT、cleanupの待機までを一つのプログラムにまとめています。二重のResult、所有権を取り出すtake、externalの外でのawaitを順に追ってください。

## parameter binding

SQLを文字列連結して値を埋め込むのではなく、DbValueの列をparameterとして渡します。SQLiteとPostgreSQLではplaceholderの書き方などをbackendに合わせます。型付きbindingでも、任意のSQL文が業務上安全になるわけではありません。table名や列名の動的指定など、値bindingで代替できない場所は別途検査します。

NULLはDbValue::Nullです。整数0や空Stringと同じではありません。db.integerはInt cell、db.floatingはFloat cellを要求し、暗黙にStringをparseしません。型が違えばDbTypeになります。

列名で読む場合、存在しない列や重複した名前を勝手に選びません。cellByNameとcolumnの失敗を処理します。大量の行ではcursorとbounded batchを使い、全件をListへ集める必要があるか考えます。

## VMとDBのcommitは別

VMのcommitはcheckpoint保存です。DB transactionのcommitはDBへの確定です。revertはDB transactionのrollbackではありません。DBで保存済みの行をVMのrevertで削除しようとしてはいけません。

| 操作 | 対象 |
|---|---|
| commit label | VMの作業状態 |
| revert label | VMの保存状態への復元 |
| publish | pending仮想I/O |
| DB begin / commit / rollback API | DBのtransaction |
| cursor / connection close | physical資源の終了 |

DB transactionを明示して複数SQLをまとめます。executeManyは暗黙のtransactionを作らないため、途中失敗前の行が適用済みの場合があります。atomicにしたい処理はtransactionへ入れ、成功時にcommit、失敗時にrollbackしてください。

既に閉じた接続はrevertで復活しません。保存していたtokenのVM情報が戻ることと、physical接続が生きていることを混同しないでください。

## PostgreSQLとcredential

postgres接続では、Secretの接続文字列をcredentialsでopaque aliasへ登録し、aliasで開きます。TLSのchainとhostnameを検証します。certificate指定の形式はDB APIに従い、HTTPのCA指定と同じ形式だと仮定しないでください。

DbErrorはcode、phase、sqlCode、sqlStateを持ちます。SQLSTATEを残せば、制約違反と接続失敗を区別できます。自動reconnect、failover、書込みretryを勝手に行うAPIではありません。送信後の結果不明では外部状態の確認が必要です。

## cleanupを確認する

scope終了で資源の解放を開始できます。waitForCleanupでnative cleanup完了とrollback/closeの失敗を待ちます。cursorを明示closeし、完了を確認してからconnectionを再利用する設計も重要です。cancelしたからDBで一切処理されていないとは判断しません。

# 第22章 JSON・CSV・設定データ

## JSONの型

JsonはNull、Bool、Int、Float、Text、Array、Objectのvariantを持ちます。parseはResultを返します。JSONとして正しいことと、アプリケーションが期待するschemaであることを分けて検査します。

~~~rewind json
import std.json as json;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(v)=>{return move v;},Err(_)=>{panic("json failed");}}
}
let value=take(json.parse("{\"name\":\"Alice\",\"score\":42}"));
match json.get(value,"score") {
    Some(item)=>{assert_eq(json.integer(item),Some(42));},
    None=>{panic("score is required");}
}
assert_eq(json.get(value,"absent"),None);
~~~

json.integerはInt variantを返し、数値に見えるStringを自動parseしません。json.floatingもFloat variantを区別します。型が違う場合をNoneとして扱う便利さと、詳細なschemaエラーを返したい要求を区別してください。

## 大きなJSONとCSV

std.jsonStreamはchunkを供給してeventを読みます。構造開始・終了、key、text、numberなどのeventを扱います。number tokenを必要なexact型へ変換する段階は別に設計します。

std.csvはboundedなデータの変換、std.csvStreamは増分処理に使えます。CSVの引用符や改行を考えず単純にcommaでsplitする方法は、一般的なCSV parserではありません。chunkの境目に引用符やUTF-8文字が来ても状態を保持するparserを選びます。

増分処理にしても、最後に全行をListへ保持すれば全体の保存メモリが必要です。一行ずつ検査・集約・書込みできる設計では、その分を抑えられます。途中の結果をpublishするなら、後続の失敗で取り消せる範囲を明示します。

## exactな値を保存する

BigIntやDecimalをFloatへ変換してからJSON numberへ保存すると、精度を失う場合があります。対応するtoJson/fromJsonの契約に従い、正確なtext表現で往復します。日時もtimezoneと精度を明示して保存してください。

config、path、argsは設定や名前の処理を補助します。pathを組み立てたことが、File APIのroot外アクセス許可を与えるわけではありません。検査と権限は別です。

# 第23章 BigInt・Decimal・日時

## 大きな整数

BigIntはFloatを介さず正確な整数を扱います。parse、format、算術、bit操作などはstd.bigintを使います。native値は不変で、Map keyやcheckpointにも対応します。容量、work、memory予算は適用されます。

~~~rewind bigint
import std.bigint as big;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(v)=>{return move v;},Err(_)=>{panic("bigint failed");}}
}
let two=take(big.fromInt(2));
let value=take(big.pow(&two,100));
assert_eq(take(big.format(&value,10)),"1267650600228229401496703205376");
Out.println(take(big.format(&value,10)));
publish;
~~~

BigIntは無限メモリを提供する型ではありません。非常に大きいべき乗の結果を求めるより、剰余だけが必要ならmodular演算を選ぶ方が適切な場合があります。

## 十進の丸め

Decimalは十進数としてprecisionとscaleを管理し、丸め方を明示します。Floatの`0.1+0.2`が必ず十進の0.3と同じ表現になるわけではないため、金額などの用途では丸め仕様を先に決めます。

~~~rewind decimal
import std.decimal as decimal;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(v)=>{return move v;},Err(_)=>{panic("decimal failed");}}
}
let a=take(decimal.parse("0.1"));
let b=take(decimal.parse("0.2"));
let sum=take(decimal.add(&a,&b,28,DecimalRounding::Exact));
assert_eq(take(decimal.format(&sum)),"0.3");
let tie=take(decimal.parse("2.5"));
let rounded=take(decimal.quantize(&tie,0,28,DecimalRounding::HalfEven));
assert_eq(take(decimal.format(&rounded)),"2");
~~~

HalfEvenはtieを偶数側へ丸めます。Exactは必要な丸めを黙って行うmodeではありません。Decimalの数値比較とscaleを含む保存表現は別なので、`1.0`と`1.00`の表示桁を保存したい用途では契約を確認します。dbDecimalはDBとの精度境界を扱います。

## 時刻と暦

Instantは時点、Durationは時間差です。datetimeの変換・算術はpureなResult APIです。現在時刻を観測するclock.nowはexternal、clockを要求します。「日時をformatする」と「現在時刻を読む」は効果が違います。

timezoneを明示します。DSTの戻りで同じ現地時刻が二度現れる場合はEarlier/Later/Rejectを選び、存在しない現地時刻のgapはエラーです。Calendarへ年・月・日を入れればあらゆるzoneで一意にInstantへ変換できるわけではありません。

SDK同梱IANAデータ版はdatetime.databaseVersionで確認できます。DBに保存する場合、相手のtimestamp精度で丸められるか、timezone情報をどこへ保持するかを決めます。dbDatetimeの変換契約も参照してください。

# 第24章 数値配列・線形代数・統計

## Listとnative配列

List<Float>は汎用collectionです。FloatArray/IntArrayはshape、rank、strideを持つtyped native storageです。大量の数値を処理するにはnumericの配列APIを使います。

~~~rewind numeric-owner
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(v)=>{return move v;},Err(_)=>{panic("numeric failed");}}
}
let shape=List<Int>();shape.add(3);
let zero=take(numeric.zerosFloat(&shape));
let data=take(numeric.affine(&zero,1.0,0.25));
let values=take(numeric.valuesFloat(&data));
values.set(0,1.0);
assert_eq(values.get(0),1.0);
let index=List<Int>();index.add(0);
assert_eq(take(numeric.getFloat(&data,&index)),0.25);
~~~

v2.0.0で返却Listは更新できます。元配列は0.25のままです。valuesFloatは明示的にListを作る境界であり、native配列のまま計算を続けられるなら不要な変換を避けます。

## shapeとview

reshape、transpose、slice、broadcastはshapeとstrideの契約を持ちます。viewは生成時の版を保持します。broadcastはreadonlyです。withFloat/withIntは更新された新しい値を返すので、利用したい変数へ代入します。元変数を変更する副作用関数だと思って結果を捨てないでください。

numericIndexはlogicalな行優先の一つのindexで要素へアクセスします。転置や逆順sliceでもlogical順で読むため、physical storageの並びをそのままindexと考えないようにします。

## 線形代数

matmul、LU solve、列pivot QR、最小二乗、対称固有値分解などを提供します。戻るshape、rank、収束、許容誤差を調べて使います。単にResultがOkだから、入力行列が望んだconditionを持つと保証されるわけではありません。

Floatの加算順序は結果の末尾のbitへ影響します。実数の数学的等式とFloatの完全一致を混同しないでください。誤差判定はabsoluteとrelativeを用途に応じて選び、NaNやInfを含めて入力を検査します。

## 統計と分布

sum、mean、variance、共分散、相関、分位点、histogramがあります。varianceのddofや分位点の規則はAPI契約へ合わせます。空配列、定数列、非finite値の扱いを確認してください。

OnlineMomentsは全サンプルを保存せずに集約します。random効果のdistributionsはVMのcheckpointed generatorから分布を生成します。再現可能な乱数状態と外部から観測したデータを混同しないでください。

## sparse・FFT・協調処理

sparseは疎行列、fftは変換の補助です。計算量だけでなくデータの表現変換と一時領域も費用に含めます。大きい計算をGUIと同時に行うなら、対応するAsync moduleを選び、TaskErrorも処理します。同期版がすべて廃止されたわけではありません。
