# 第9章 所有権・借用・snapshot

## 何を渡すのかを明示する

可変ownerは自由に複製できません。関数へ値を渡すとき、所有権を移すなら`move`、読むだけなら`&`、変更するなら`&mut`を使います。Int、String、適切なrecordなどのShare値は共有できます。

~~~rewind ownership
fn total(values:&List<Int>)->Int effects {} {
    var result=0;
    for i in 0..values.len() {result+=values.get(i);}
    return result;
}
fn append(values:&mut List<Int>,n:Int)->Unit effects {} {
    values.add(n);
}
let values=List<Int>();
append(&mut values,7);
assert_eq(total(&values),7);
let snapshot=freeze(values);
let copy=thaw(snapshot);
copy.add(8);
assert_eq(copy.len(),2);
assert_eq(snapshot.len(),1);
~~~

totalはListを読み、appendは変更します。snapshotはその時点の値を保持します。thawで得た可変値の変更はsnapshotへ混ざりません。bindingがletかvarかだけでは、関数に渡せるかどうかは決まりません。

借用が生きている間、元ownerを競合する形で変更・moveできません。借用引数を戻り値や長生きするtaskへ逃がすことにも制限があります。検査に合わせるために無差別にclone相当の処理を追加するのではなく、関数が本当に読むだけか、所有する必要があるかを先に考えます。

## ShareとSend

Shareは不変として共有できる契約、Sendはtaskへ渡せる契約です。同じではありません。通信接続はSendでもShareではなく、freezeでsnapshotへ入れられるものではありません。

外部リソースを含むTaskの完了値は一度だけ取り出せます。スコープ所有の接続を別の場所へ移す場合、所有権を明示します。終了済みのphysical接続をcheckpointから復活させることはできません。

## freezeの費用

freezeは、対象グラフの検証と必要なコピーを行います。不変ページの共有はできますが、操作全体が無条件に無料になるわけではありません。循環や深さ64を超える構造、関数・資源の捕捉は制限されます。検証のnative workと一時領域も予算対象です。

v2.0.0のnative factoryが返すList/MapとOption/Result内のそれらは、通常の可変所有値として扱えます。例えばnumeric.valuesFloatで得たListをsetしても元配列は変わりません。これは外部接続を共有可能にする変更ではありません。

# 第10章 commit・revert・publishを理解する

## 三つの操作を分ける

`commit name;`はVMの作業状態をcheckpointとして保持します。`revert name;`は状態を戻し、現在の実行位置から続けます。`publish;`はpending I/Oを外部へ確定します。この三つは代用できません。

~~~rewind publish-revert
var score=10;
commit test1;
score=99;
Out.println(score);
commit test2;
publish;
revert test1;
Out.println(score);
publish;
drop test2;
drop test1;
~~~

出力は次の通りです。

~~~text
99
10
~~~

99のprintlnは最初のpublishで外部へ出ています。revertはscoreを10へ戻しますが、既に出た99を消しません。最後のpublishは新しい10の出力だけを確定します。古い99を再送しません。

## publish前なら出力を取り消せる

~~~rewind pending-revert
var score=10;
commit test1;
score=99;
Out.println(score);
commit test2;
revert test1;
Out.println(score);
publish;
drop test2;
drop test1;
~~~

出力は10だけです。99のprintlnはpendingであり、test1に戻すことで破棄されています。test2をcommitしていてもpublishにはなりません。

| 時点 | score | pending出力 | 外部に確定した出力 |
|---|---|---|---|
| test1保存直後 | 10 | 空 | 空 |
| 99をprintlnした直後 | 99 | 99 | 空 |
| publishした場合 | 99 | 確定済み扱い | 99 |
| test1へrevert | 10 | test1の状態 | 既に確定した99は残る |
| 10をprintln・publish | 10 | 新しい出力を確定 | 99、10 |

この表のpublishを省けば、確定した99は存在しません。Gitの履歴を思い浮かべることは役立ちますが、Gitの全コマンドと一対一に対応する仕様ではありません。特にcheckpointは外部ファイルへのcommitではありません。

## 状態には何が含まれるか

変数、heap、List/Map、仮想file、Iterator cursor、task状態などが対象です。乱数のVM状態も戻せます。一方、消費した実行予算、記録済みのHost観測、確定済みの外部作用は戻りません。

checkpoint後に宣言した変数はrevertで失われる場合があります。戻す前のローカル名を使い続ける設計にしないでください。GUIでもViewとmodelは戻せますが、OSへ確定した画面を変更するには戻したsceneを再度present・publishする必要があります。

## dropを忘れたら

publishはcheckpointを自動で捨てません。dropを書かなければ、checkpointが過去の状態を保持し続けます。保持する履歴が予算を圧迫する場合があります。終了時にruntimeが解放されることと、長時間実行中に不要な履歴を解放することは別です。

Undoのために保持したいcheckpointは正当な使用メモリです。不要になったcheckpointはdropしてください。過去のsnapshotを保持しながら変更すると、そのsnapshotが参照する旧ページは回収できません。最後の参照がなくなると回収可能になります。

# 第11章 begin・resume・branch

## 自動のbegin

ユーザーコードを実行する前に、`commit begin;`に相当する初期checkpointが自動保存されます。beginは予約名であり、自分でcommit/dropしたり、変数・関数・branchの名前にしたりできません。

~~~rewind begin-reset
var score=99;
commit test;
Out.println(score);publish;
Out.println("discarded");
revert begin;
var score=10;
commit test;
Out.println(score);publish;
drop test;
~~~

出力は99、10です。pendingのdiscardedは消えます。ユーザーcheckpointもリセットされるため、testを新しく作れます。revert begin後にscoreを宣言し直している点にも注目してください。

revert beginはmain taskで、active branchの外から使います。変数・heap・未公開I/O・task状態を初期化しますが、実行中の関数やblockの継続枠は保持します。後続のreturnやblock終了を可能にするためです。公開済みの外部結果、観測記録、消費済み予算を消す「OS全体の初期化」ではありません。

## revertとresume

revertは状態、resumeは状態と制御位置を復元します。resume beginならユーザーコード先頭へ戻ります。停止条件を外側に持たずにresumeを繰り返すと、予算を消費するループになります。

同じ関数呼出しの中なら、入れ子のifから外側のcheckpointへrevertできます。既に終了した内側スコープや別の関数呼出しのcheckpointへrevertすると、InvalidContinuationになります。checkpointのデータが存在するだけで、任意の継続位置が有効になるわけではありません。

## branchの隔離

branchは試行用の状態を作り、終了時に親状態へ戻します。

~~~rewind branch
var score=10;
branch trial {
    score=99;
    Out.println("試行の出力");
}
assert_eq(score,10);
Out.println(score);
publish;
~~~

出力は10です。branch内の変更とpending出力は親へ持ち帰りません。branch内でpublishして試行を外部へ確定する使い方には制限があります。採用する結果を返す一般的な関数と、隔離試行のbranchを使い分けてください。

## 設計の目安

「失敗したら戻す」だけならResultと新しい値の構築で済むこともあります。多数の関連状態をまとめて保持したいならcheckpointが役立ちます。ユーザー操作のUndoでは、どの操作単位にcheckpointを置くか、いくつ保持するか、外部保存の後もUndoを許すかを明確にします。

# 第12章 module・公開範囲・プロジェクト

## 標準moduleのimport

~~~rewind import
import std.number as number;
assert_eq(number.decimal("42"),Ok(42));
~~~

`std.number`はmodule、`number`はこのsourceで使うaliasです。aliasはmoduleごとの名前です。選択importとaliasも利用できます。単一ファイルで使うstdはcompilerに内蔵されており、最初の一つのプログラムにsdk-installは不要です。

## 自分のmodule

宣言は原則としてmodule内に属し、外部へ見せるものにpubを付けます。private fieldは型の不変条件を守るために使います。利用者が内部fieldへ直接アクセスする必要があるなら、必要な操作をpublic関数として定義できないかを先に検討します。

相対importはsource directoryを基準に解決します。projectではsource_root、import設定、lock、署名が解決の基準になります。module名の変更は利用側のimportにも影響します。

## projectを使う場合

単一ファイルから先へ進み、複数moduleと固定した依存を管理する場合はrewind.tomlを使います。次は基本形です。

~~~toml
language = "2.0.0"
source_root = "."
entry = "main.rw"
effects = "input,output,args,locale,random,tasks"
~~~

~~~sh
rewind check --root .
rewind test --root .
rewind run --root .
rewind build --root . --output app.rwc
~~~

checkは型と効果を調べ、testはprojectのテストを実行します。SDKをprojectへ固定するには、署名を検証してsdk-installします。標準moduleをprojectのvendorへ固定し、lockを更新するため、あとでSDKの場所を動かしても解決できます。

~~~sh
rewind sdk-verify --sdk SDK_DIR --public-key PUBLIC_KEY_HEX
rewind sdk-install --root PROJECT_DIR --sdk SDK_DIR --public-key PUBLIC_KEY_HEX
~~~

PUBLIC_KEY_HEXはplaceholderです。検証済みの鍵へ置き換えます。鍵、SDK、projectが異なるときの失敗を無視して先へ進めないでください。

## APIを壊さず変更する

public functionの引数型、戻り型、効果、generic bound、所有権契約は利用者に影響します。単に関数名を残しても、Shareを追加要求したり新しい効果を付けたりすれば呼出し元がコンパイルできなくなる場合があります。本書API編の宣言は、それらを含む契約として読んでください。
