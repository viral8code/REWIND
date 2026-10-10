# 第5章 関数とプログラムの入口

## 関数を定義する

関数には名前、引数、戻り値の型、効果を宣言します。`effects {}`は外部操作をしない契約です。returnで結果を返します。通常の関数呼出しにはawaitを使いません。

~~~rewind factorial
fn factorial(n:Int)->Int effects {} {
    if n<=1 {return 1;}
    return n*factorial(n-1);
}
assert_eq(factorial(5),120);
Out.println(factorial(5));
publish;
~~~

再帰では同じ関数を呼びます。停止条件が重要です。この例は5!の計算を確認するためのもので、任意の巨大なnに対応する階乗ではありません。負のnを拒否する検査もしていません。実用関数では定義域、overflow、予算を考え、必要ならResultやBigIntを使います。

引数が少ないからといって効果が少ないとは限りません。出力する関数は`effects {output}`を宣言します。効果のない関数から、出力関数を呼び出すことも効果検査の対象です。

## main関数

短いプログラムではトップレベルに文を並べられます。アプリケーションの入口を明示する場合は`fn main()->Int`を使います。

~~~rewind main-function
fn main()->Int effects {output} {
    Out.println("started");
    publish;
    return 0;
}
~~~

0は正常終了です。非0は終了値として扱われます。mainを使っても終了時に出力を自動確定しません。publishを明示してください。外部へ見える結果を最後にまとめて確定する関数なのか、途中でも確定する関数なのかをAPIの設計時に決めると、呼び出し元が扱いやすくなります。

## 補助関数の責任

文字列を整数に変換する関数は、成功したらInt、失敗したらエラーを返すと自然です。常にpanicする補助関数はテストや短い例には便利ですが、ユーザーの入力を読むアプリケーションでは、呼出し元が再入力やエラー表示を選べるResultの方が適しています。

小さく分ける目安は「この関数が成功したとき何が保証されるか」を一文で説明できることです。計算と入力、モデル更新とGUI表示、SQL構築とDB実行を分ければ、計算部分を外部アクセスなしにテストできます。

# 第6章 List・Map・Iterator

## Listの基本

Listは型付きの可変列です。indexは0始まりです。`get`の範囲外は診断となり、自動でNoneになりません。入力から得たindexでは、先に範囲を確認します。

~~~rewind list
let values=List<Int>();
values.add(3);
values.add(5);
values.add(7);
assert_eq(values.len(),3);
values.set(1,9);
var total=0;
for i in 0..values.len() {total+=values.get(i);}
Out.println(total);
publish;
~~~

出力は19です。空のListの最後の要素を読むために`len()-1`を使う場合、先に空かどうかを確認してください。要素型がIntであることと、indexが正当であることは別の問題です。

Listは永続ページを使うVMのデータ構造です。checkpointで保持する過去の版と共有でき、更新された部分は分離されます。普通のメモリ配列の全操作を厳密にO(1)と見積もるのではなく、ページ木の探索、COW、要素の共有・コピー、保持する履歴の費用も含めて考えます。

## Mapの基本

Mapのgetは値の有無をOptionで返します。存在しないキーと、値として0が入っていることを区別できます。

~~~rewind map
import std.map as map;
let counts=Map<String,Int>();
counts.set("apple",3);
assert_eq(counts.get("apple"),Some(3));
assert_eq(counts.get("pear"),None);
assert_eq(map.getOr(&counts,"pear",0),0);
map.update(&mut counts,"apple",|old:Option<Int>|->Option<Int>{
    match old {
        Some(n)=>{return Some(n+1);},
        None=>{return Some(1);}
    }
});
assert_eq(counts.get("apple"),Some(4));
~~~

`std.map.update`のcallbackは、Someを返すと設定し、Noneを返すと削除します。キーはStringです。std.mapのすべての関数が任意のKを受け取るわけではありません。`entries`もStringキー用です。API編の型を確認してください。

キーにはOrdの契約があります。組込みのキーとuser定義Ordキーでは格納・探索費用が同一とは限りません。文字列をキーにするとき、正規化や大文字小文字の同一視は自動ではありません。

## Iteratorはsnapshotを読む

Iteratorを使うと、列挙、変換、絞込み、集約をつなげて表現できます。`collect`は結果をListに集めます。

~~~rewind iterator
let values=List<Int>();
values.add(1);values.add(2);values.add(3);
let doubled=values.iter()
    .map(|n:Int|->Int{return n*2;})
    .filter(|n:Int|->Bool{return n>2;})
    .collect();
assert_eq(doubled.len(),2);
assert_eq(doubled.get(0),4);
assert_eq(doubled.get(1),6);
~~~

Iteratorにはnext、fold、take、enumerate、zipもあります。元のListを更新しても、既に作ったIteratorのsnapshotに追従して変更が混ざることはありません。cursorはcheckpointの対象です。取り出し位置を戻せば同じsnapshotを再度列挙できます。

`iter()`とfreezeはnative work・history memoryを消費します。深さ64、循環、関数や資源の捕捉にも制限があります。大きいcollectionだから直ちに1万要素で拒否されるという規則ではなく、Mapキーの検査制限とは区別します。列挙してできるListまで含めて必要メモリを見積もってください。

## データ構造の選択

| 必要な操作 | 候補 |
|---|---|
| 順に保持・添字で読む | List |
| キーで対応を引く | Map、std.map |
| 両端を追加・削除 | std.deque |
| 最小値を反復して取り出す | std.heap |
| 同じ集合かを判定 | std.disjointSet、std.rollbackSet |
| 区間集約・更新 | std.fenwick、std.segment、std.lazySegment |
| bit集合 | std.bitset |
| staticな区間最小値 | std.sparse |

heapの比較関数は全操作で一貫させます。disjointSetとrollbackSetは用途が異なり、VMのrevertとは別にデータ構造自身の履歴を操作する場合はrollbackSetを選びます。容量指定やResultの失敗処理を省略しないでください。

# 第7章 Option・Result・match

## 値がないことを表現する

`Option<T>`は`Some(value)`か`None`です。例えば「検索が見つからない」を、例外ではなく通常の結果として表現できます。

~~~rewind option
let value:Option<Int>=Some(7);
match value {
    Some(n)=>{assert_eq(n,7);},
    None=>{panic("値がありません");}
}
~~~

Noneを0と同じにするかどうかはアプリケーションの判断です。金額が0であることと金額未入力を区別したい場合、早い段階でNoneを0へ変換すると情報を失います。

## 成功と失敗を分ける

`Result<T,E>`は`Ok(value)`か`Err(error)`です。EはStringでも、recordでも、enumでも構いません。標準ライブラリのStdErrorはcodeとoffsetを持ちますが、すべてのエラーがStdErrorではありません。

~~~rewind result
import std.number as number;
import std.integer as integer;
fn increment(input:String)->Result<Int,String> effects {} {
    match number.decimal(input) {
        Ok(value)=>{return integer.add(value,1);},
        Err(_)=>{return Err("InvalidNumber");}
    }
}
fn twice(input:Int)->Result<Int,String> effects {} {
    let value=integer.add(input,input)?;
    return Ok(value);
}
assert_eq(increment("41"),Ok(42));
assert_eq(increment("x"),Err("InvalidNumber"));
assert_eq(twice(21),Ok(42));
~~~

`?`はResultを返す関数の中で使い、Errならその失敗を早期returnします。呼出し先と返却先の失敗型を合わせます。任意の異なるエラー型を自動変換する規則ではありません。

通常算術のoverflow、範囲外get、panic、実行予算超過は、勝手にその関数のResultへ変換されません。上の`integer.add`は回復可能なchecked加算を選んでいる点が大切です。失敗を受け取って続行したい場所では、検査するかfallible APIを使います。

## matchの網羅性

enumをmatchする場合、可能なvariantを扱います。入れ子のOption/Result、tuple、generic enum、recordも検査対象です。`_`は値を捨てるpatternであり、「分岐を省略しても何か返す」という命令ではありません。

guardは追加条件です。guardが付いた分岐だけでは、そのvariant全体を網羅したことになりません。guardの中ではcheckpoint操作やpublishを行えません。前のarmに完全に含まれるarmや不正なInt区間も拒否されます。

所有する接続などをResultから取り出す場合は`match move result`が重要です。普通のmatchで共有借用した値に、後から`&mut`を付ければよいとは限りません。所有権の章とDB・通信の例を参照してください。

## エラーの型を設計する

入力ミス、存在しないデータ、外部通信失敗を同じ文字列へ潰すと、呼出し元が判断できません。enumのvariantやcodeを使って分類し、詳細メッセージは別に持つと扱いやすくなります。特に外部書込みでは「未送信」と「送信後の結果不明」を区別しないと、retryで重複する可能性があります。

# 第8章 自分の型・generic・trait

## structとrecord

structは可変データ、recordはShareを満たす不変データの組合せに使います。recordのfieldにはShareな型が必要です。名前付きのデータは、tupleより意味を表しやすくなります。

~~~rewind data-types
struct Counter {value:Int}
record Point {x:Int,y:Int}
enum Status {Ready,Failed(String)}
type Score=Int;
let counter=Counter(1);
counter.value=2;
let point=Point(3,4);
assert_eq(point.x+point.y,7);
let status=Status::Failed("missing");
match status {
    Status::Ready=>{},
    Status::Failed(message)=>{assert_eq(message,"missing");}
}
~~~

struct/record/enumには位置引数のconstructorがあります。named constructorとfield patternも利用できます。private fieldを持つ型は、module外から直接構築・読み書き・分解できません。公開factoryとaccessorを使います。

外部接続やnative配列のような組込み型は、API文書にfieldが見えていても、任意の値をconstructorで作れば正当な接続になるものではありません。opaque値の生成は対応する標準関数を使ってください。

## generic

同じ操作を異なる型へ適用するためにtype parameterを使います。`T:Ord+Share`は、そのTに順序と共有の契約を要求します。genericは「型を検査せず何でも入れる」という仕組みではありません。

~~~rewind traits
trait Read {fn read(self:&Self)->Int effects {};}
struct Box {value:Int}
impl Read for Box {
    fn read(self:&Box)->Int effects {} {return self.value;}
}
fn read<T:Read>(value:&T)->Int effects {} {return value.read();}
fn compare<T>(a:T,b:T)->Int effects {} where T:Ord+Share {
    return a.cmp(b);
}
let box=Box(7);
assert_eq(read(&box),7);
assert_eq(compare(1,2),-1);
~~~

traitは型が提供する操作の契約、implはその実装です。関連型とdefault methodも利用できます。whereを使うと、複数の制約をsignatureから整理して書けます。

generic factoryの型は、戻り値の期待型から全parameterを確定できる場合に推論できます。分からない場合は`List<Int>()`のように明示します。深い型の展開と特殊化は有限のcompiler予算に従うため、無制限の型レベル計算を前提にしないでください。

## クロージャー

`|n:Int|->Int{return n*2;}`は引数を取る関数値です。引数がなければ`||->Int{...}`です。関数型にもeffectsがあり、必要に応じて`captures {Send,Share}`の契約が付きます。

捕捉方法はvalue、move、borrowを区別します。借用captureは借用元より長生きしたりtaskへ逃げたりできません。callback引数に関数を渡す場合、「何を計算するか」だけでなく「何を捕捉し、どんな効果を持つか」もsignatureの一部です。API編ではcompilerの関数型表記`!{...}~{...}`も載せています。`!`の後は効果、`~`の後はcapture契約です。

## 捕捉を指定する例

value captureは作成時の値を保持します。あとで元bindingを変更しても捕捉値は変わりません。

~~~rewind capture-value
var n=7;
let read=capture value ||->Int{return n;};
n=9;
assert_eq(read(),7);
assert_eq(n,9);
~~~

move captureはownerをクロージャーへ移します。元の名前から再利用するのではなく、クロージャーが所有する値として使います。

~~~rewind capture-move
let values=List<Int>();values.add(7);
let read=capture move ||->Int{return values.len();};
assert_eq(read(),1);
~~~

borrow captureは借用の有効なscopeへ限定します。scopeを出た後なら元ownerを更新できます。

~~~rewind capture-borrow
let values=List<Int>();values.add(7);
{
    let read=capture borrow ||->Int{return values.len();};
    assert_eq(read(),1);
}
values.add(8);
assert_eq(values.len(),2);
~~~

## 名前付きvariantとmatch式

field名で構築するenumは、引数の順番だけに頼らず値の意味を示せます。matchを式として使う場合、各armの返す値の型を合わせます。

~~~rewind named-enum
enum Command {Write {path:String,count:Int},Quit}
let command=Command::Write {count:2,path:"a"};
let text=match command {
    Command::Write {path:value,count:_}=>value,
    Command::Quit=>"quit"
};
assert_eq(text,"a");
let size=match 4 {
    0..3=>"small",
    n if n>3=>"large",
    _=>"middle"
};
assert_eq(size,"large");
~~~

最後のmatchにはguardで覆えない場合のarmもあります。区間の終端とguardの網羅性を混同しないでください。

## 練習

座標を持つPointと、二つのPointの距離の二乗を返す関数を作ってください。Intで計算する場合はoverflowの可能性を考えます。成功・失敗をResultへ分ける版と、Floatで計算する版では契約がどう変わるでしょうか。
