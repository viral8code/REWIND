# 第25章 自動微分・最適化・モデル保存

## 計算をtapeへ記録する

std.autodiffはtape上へparameterと演算を記録し、lossからgradientを求めます。constantとparameterを区別し、backward後に必要なNodeのgradientを照会します。数学的に微分可能なことと、任意のREWINDコードをそのまま自動微分できることは違います。対応する演算をtape APIで表現します。

同じparameter名を使う場合の契約、shape、broadcast軸のgradient集約を確認します。shapeが合っているだけで、データの行・列の意味まで検査されるわけではありません。

## 最適化の状態

std.optimizeはSGD/Adam等の更新を補助します。Adamの状態にはstepとmomentが含まれます。重みだけを戻してoptimizer状態を戻さないUndoは、同じ学習状態へ戻ったことになりません。必要なら両方をcheckpointで保持します。

std.trainingはsample数で重み付けしたbatch meanやglobal-norm clippingを組み合わせます。大きさが異なるbatchのmeanを単に等重みで平均すると、sample単位のmeanとは違う結果になります。

## 同期と協調

autodiff.backwardは同期です。autodiffAsync.backwardはtapeを所有して協調的に逆伝播します。Nodeを保持し、返却Gradientsから照会できます。準備、走査、重いkernelの途中で制御を渡す契約があります。所有するtapeをmoveするので、同じ可変tapeを別taskから更新する設計ではありません。

forward側にもautodiffForwardAsyncがあります。Asyncという名前だけで全演算が別OS threadへ並列化されるとは考えないでください。取消、部分状態、history memory、native workを含めて設計します。

## モデル保存

std.modelsで名前付き重みをまとめ、encode/decode、save/loadします。model.rwmのようなfileへのsaveは仮想File操作とpublishの境界を持ちます。保存形式の検証を省略せず、読み込んだ重みの名前とshapeを利用側でも確認します。

毎反復のtapeと途中gradientを不要になったら手放します。全epochでcheckpointを保持すれば、正当に全過去状態のメモリが残ります。保持したい最良モデルと、毎回の途中計算を区別して管理すると必要領域を減らせます。

## 小さな学習から始める

最初は既知の小さな線形関係で、lossが下がること、求めた係数が期待に近いこと、保存して再読込みしても同じ重みであることを確認します。次にbatch分割、丸め、clip、optimizer状態の復元を追加します。結果だけでなく、使ったデータと設定も残すと調査しやすくなります。

# 第26章 アルゴリズムの実装と選択

## sortとsearch

std.sort.stableは比較が負/0/正を返す安定merge sortです。入力を借用して新しいListを返します。inPlaceは更新する版です。tryStableはcallbackのErrを返し、入力を保持します。どのAPIでも途中失敗時に入力が完全に戻ると一括保証するものではありません。

~~~rewind sort
import std.sort as sort;
let values=List<Int>();
values.add(8);values.add(2);values.add(5);values.add(2);
let ordered=sort.integers(&values);
assert_eq(ordered.get(0),2);
assert_eq(ordered.get(1),2);
assert_eq(ordered.get(2),5);
assert_eq(ordered.get(3),8);
assert_eq(values.get(0),8);
~~~

比較の整合性が必要です。`a<b`と`b<a`を両方trueにする比較関数では正しい順序が作れません。searchの二分探索を使う場合、同じ比較規則で整列済みであることを事前条件にします。

## 最短経路

graph.createは頂点と辺容量を指定します。addは有向辺を追加します。無向graphなら必要な向きの辺を明示します。

~~~rewind graph
import std.graph as graph;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(v)=>{return move v;},Err(_)=>{panic("graph failed");}}
}
let g=take(graph.create(4,4));
take(graph.add(&mut g,0,1,5));
take(graph.add(&mut g,1,2,2));
take(graph.add(&mut g,0,2,10));
let distance=take(graph.dijkstra(&g,0));
assert_eq(distance.get(0),Some(0));
assert_eq(distance.get(2),Some(7));
assert_eq(distance.get(3),None);
let hops=take(graph.bfs(&g,0));
assert_eq(hops.get(2),1);
assert_eq(hops.get(3),-1);
~~~

Dijkstraは重みを使い、BFSは辺数を使います。ここでは0→2の直接辺があるため、BFSの距離は1です。Dijkstraの未到達はNone、BFSの未到達は−1です。戻り値の単位と表現が違います。

graphは最大65,536頂点/辺、ancestorsは最大4096頂点の根付き木など、関数ごとの上限があります。大きいgraphにはnative IntArrayを使うgraphLargeがあり、頂点/辺は最大1,048,576です。上限内でも予算は別に必要です。

## graphの他の処理

topologicalはcycleをErrにします。bellmanFordは到達可能な負cycleを検査します。componentsは強連結成分の密なidを返します。minimumForestは各有向辺を無向候補として解釈するKruskalです。二重に入れた辺も候補です。ancestors/lcaは根付き木の前提を検査します。

最大流はflow、二部matchingはmatchingです。flowのresidual状態はVMのcheckpoint対象です。最大matchingの結果には最小vertex coverも含まれます。目的が違うため、単純なunion-findで代替できるとは限りません。

## 文字列・区間・幾何

stringSearchにはKMP/Z、trieにはprefix検索、suffixにはsuffix関連処理があります。fenwick、segment、lazySegmentは更新と区間queryの違いに合わせて選びます。geometryでは座標範囲とoverflow、境界点の扱いを確認します。dp、matrix、number、modularも必要なprimitiveを提供します。

教科書上のO(n log n)は主に比較やprimitive操作数です。VM collectionのページ探索・COW費用までゼロになるわけではありません。大きいデータでは入力scanner、native配列、不要なfreezeやcollectの回避も効果があります。

# 第27章 診断・テスト・record / replay

## エラーはまずcodeと位置

診断にはcode、source、line、column、taskId、causes、frames、hintsなどがあります。textでは内側から外側へcallerを表示します。位置は1始まり、columnはUnicode scalarです。LSPへはUTF-16位置へ変換されます。

callerは最大32件で、省略を示します。local値、引数、closure captureをbacktraceへ追加しません。source-free成果物でもsource名と位置は保持します。native内部のすべての関数呼出しまで完全なstackを保証するものではありません。

| code | 最初に確認する点 |
|---|---|
| DivisionByZero | divisor |
| IntegerOverflow | Intの範囲、checked API |
| IndexOutOfBounds | 負index、len以上、空collection |
| UnknownName | scope、綴り、import |
| MissingEffect / EffectMissing | 関数宣言と実行許可 |
| InvalidContinuation | checkpointと関数・scopeの寿命 |
| ExecutionBudgetExceeded / NativeWorkBudgetExceeded | 停止条件、入力量、費用 |
| TaskDeadlock | waitGraph、通知・close・joinの順 |
| ExternalStateConflict | Host fileが途中で変更されたか |
| PublishPartiallyApplied | phaseとapplied、実際の外部状態 |

JSON診断はstderrへ出し、applicationのstdoutと分けます。

~~~sh
rewind run main.rw --diagnostic-format json
~~~

診断JSONにはexit_status、message、diagnostic、publish_failure、retryable、retry_hintなどがあります。過去のrecordでは追加fieldが省略される場合があります。codeを見ずにmessageの一部だけで分類しないようにします。

## 子taskの失敗

std.taskError.describeは元の診断とtyped budget、wait graphを所有するFailure envelopeへ変換します。診断を持つFailedは元codeを保持し、取消、timeout、native work超過を区別します。

asStdはcodeと失敗診断の行をStdErrorへ投影します。全文と原因treeを保存する投影ではありません。詳細を必要とするなら先にdescribeしてください。具体的なbudget失敗をすべて「NumericTask」として処理する旧前提は使わないでください。

## 小さなテスト

assertとassert_eqでpureな関数を確認します。正常な入力だけでなく、空、最小値、最大値、境界の一つ外、負数、Unicode、失敗時に入力が保持されるかを考えます。可変処理では「戻り値が正しい」と「入力を壊していない」を分けて検査します。

projectならrewind testを使えます。recordは入力・scheduler選択・観測・失敗位置などを再現する調査手段です。unit testをすべてrecordへ置き換える必要はありません。

~~~sh
rewind run main.rw --record trace.json
rewind run main.rw --record trace.json --record-mode compact
rewind replay trace.json --root .
~~~

既定debug記録はstep移動に対応し、compactは詳細step履歴を保持せず命令数・順序digest・観測・最終状態を照合します。長い計算でdebug indexの上限に当たる場合、compactの保証を理解して選びます。

## replayは外部処理の再実行ではない

通信とDBの記録済み観測を使い、送信やDB書込みを再実行しません。fixture GUIもOS画面を開きません。secret入力は再注入が必要な場合があります。実サービスとの接続が不要な再現は有用ですが、実サービスの現在の状態を検査する試験とは区別します。

# 第28章 速度・メモリ・実行予算

## 二種類の仕事量

VM命令のstepsと、native処理のworkは別です。既定はそれぞれ1,000,000、task-stepsは100,000です。実行量に合わせて明示します。

~~~sh
rewind run main.rw --steps 10000000 --native-work 100000000 --task-steps 1000000
rewind run main.rw --history-memory 128MiB --history-storage 512MiB --spill-threshold 32MiB
~~~

historyの値はbytes、KiB、MiB、GiBを指定できます。spill thresholdの0は強制spill用です。source内のruntime設定もありますが、処理済みの累積予算をrevertや設定変更で回復できません。

予算はlogical work/allocationの契約で、物理CPU時間やRSSを厳密に測る値ではありません。Linuxではcompiler/runtimeへ既定2GiBのaddress-space上限がありmemory-mibで指定できます。WindowsではそのOS上限を提供せず、明示memory-mibはunsupportedです。両OSでVMのlogical予算は使えます。

## 履歴とGC

checkpointが保持するメモリは生きたデータです。GCは、利用者が保存した過去状態を勝手に捨ててメモリを減らしません。不要なcheckpointをdropし、現在の変数やTaskにも不要な値を残さないようにします。

List/Map/heapは永続構造で、共有部分はruntime内で重複計上を抑え、変更部分と独立した所有元は計上します。GCは参照を含まない共有部分を省略し、同じ共有nodeを一回の探索で重複走査しません。checkpointのheapはそれぞれ保持されます。

Bytesはshared native bufferの実際のcapacityと管理情報をruntimeごとに計上します。sliceが小さくても大きいbufferの参照を残す場合があります。弱参照索引はbufferを保持せず、GC・profile・上限判定で解放済み項目を整理します。外部workerがまだ保持するbufferも生きています。

## 何から最適化するか

まず入力量とアルゴリズム、次に中間collectionの数、snapshotと履歴、native変換境界、最後に小さな式を調べます。何回もvaluesFloatへ変換するよりnative配列のまま計算する、全行のCSVを保持せず集約する、不要なcheckpointを落とす、といった変更は効果が分かりやすくなります。

速さのためにすべてをexternal liveへ出すと、再現性の境界が変わります。性能変更も効果・所有権・失敗契約を守ってください。測定時は同じ入力・予算・記録modeで比較します。

## compilerにも上限がある

sourceは1MiB/module、module数は256などの上限があります。generic展開、type展開、dependency解決、effect推論にも有限予算が適用されます。allocatorやOS強制終了をResultへ必ず変換できる保証はありません。大量sourceを一つのmoduleへ詰めれば問題が消えるとも限りません。

# 第29章 ツールと日々の開発

## よく使うコマンド

| 目的 | コマンド |
|---|---|
| 版・総合help | rewind --version / rewind --help |
| source実行 | rewind run main.rw |
| compile | rewind compile main.rw / rewindc main.rw |
| artifact実行 | rewind run main.rwc / rewind main.rwc |
| project検査 | rewind check --root DIR |
| project試験 | rewind test --root DIR |
| project成果物 | rewind build --root DIR --output app.rwc |
| 整形 | rewind fmt main.rw |
| 整形検査 | rewind fmt main.rw --check |
| module API生成 | rewind doc FILE --root DIR --output FILE |
| エディタ接続 | rewind lsp --root DIR |
| 記録再生 | rewind replay TRACE --root DIR |

formatterは既存改行を保ちつつindentを整えます。任意の一行コードをすべて文単位に分割するformatterではありません。docには対象sourceを指定します。全subcommandで`--help`が同じように受理されると仮定せず、総合helpと主要commandのhelpを利用します。

## LSP

stdio LSP clientをエディタへ設定します。診断、補完、hover、定義・参照、rename、signature help、quick fix、整形などを提供します。エディタ側でstdio serverを起動できる設定が必要です。runtime診断と編集中の診断は、利用できる情報とタイミングが違います。

rename後にCLIでcheckし、公開APIへ影響した場合は利用側も検査します。quick fixは提案であり、アプリケーションの目的まで保証しません。

## 配布前の確認

source-freeで実行するなら、ソースがなくても動くか、必要なeffectを許可しているか、application引数を受け取れるか、診断位置が残るかを確認します。記録も実行するならexact compilerを揃えます。

GUI fixtureとnative画面、DB memoryと実file、local HTTPとTLSなど、異なる層の試験を混同しないでください。公開したartifactを更新した場合、元sourceとcompiler版、API契約の対応を残すと問い合わせに答えやすくなります。

# 第30章 実習：小さなアプリケーションへ進む

## 実習の順序

最初は計算とassertだけで作り、次に入力、エラー表示、保存、GUIを加えます。一度に通信とDBとGUIを導入するより、失敗の原因を絞れます。以下の完全な実習コードは、それぞれ独立して試せます。

カウンターGUIでは、AddとUndoを繰り返し、戻したmodelをpublishします。SQLiteではparameter付きINSERTとcursorを使います。HTTPではURLを引数に取り、二重のResultを検査します。線形方程式ではnative配列を作り、結果とcheckpointによる復元を確認します。

## 例を実用化するとき

例中のtakeは失敗するとpanicします。実用アプリではErrを分類して表示し、必要なcleanupやrollbackを行う関数へ変えてください。HTTPの引数がない場合、利用説明を出します。DBのCREATE TABLEはfile DBで再実行すると既存tableに対して失敗するため、migrationや初期化規則を別に設けます。

GUIのUndoはfileやDBに保存済みの結果を自動で戻しません。保存後に編集内容を戻した場合、表示、現在model、保存済み状態の違いを利用者に伝える必要があります。保存をUndoしたいなら、DBの新しいtransaction等で補償する処理を明示します。

数値処理では、例の小さい行列から、大きいshape、悪条件、非finite、work不足へ試験を増やします。エラーを隠して0や空配列に変えると、呼出し元が誤った結果を正常と扱う可能性があります。

{{worked-examples}}

## 次に読む場所

目的のmoduleをAPI編で検索し、signatureの引数名と型、所有権、effectsを確認してください。失敗・費用の契約がある場合はそれを読み、source実装で境界の動作を確かめます。本書の文法編とAPI編を行き来して、自分の入力・容量・外部作用の条件に合わせて組み立ててください。
