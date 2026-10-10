# 構想の具体化：動的checkpoint・handle・型付きsnapshot

状態：採用判断前。未実装のAPI案です。ユーザー提案の「commit名を文字列でreflection的に指定する」から、libraryが複数版を扱える方向へ掘り下げます。版番号は未割当です。

## 現行仕様と差分

現行のcommit/revert/resume/dropは構文上のlabelを受け取ります。`src/v2.rs`のStmtKindは名前を保持し、parserはlabelを解析します。runtime内に名前で保存先を探す処理があることと、REWINDプログラムからString式で呼べることは別です。

現行List/Map等は共有pageとCOWを使うため、永続データ構造の基盤はすでにあります。追加したいのは「過去の版を選ぶ」「同時に読む」「新しい版を派生させる」を利用者のコードから表現する能力です。

関連する既存候補は[STATE-01](v2-next-language.md)、[MEM-03](v2-next-memory.md)、[EXEC-08](v2-next-execution.md)。この文書はそれらと接続する具体案であり、現在のreferenceへ掲載する仕様ではありません。

## CP-01：Stringでcheckpointを作る・探す

候補はcreate(name)、lookup(name)、list、describeです。文字列で名前を組み立てて、多数の履歴をloopやlibraryから管理します。

名前にはnamespace、長さ、重複時の扱いを定めます。beginを作成・上書き・削除する入口にはしません。同じ名前を再利用したとき、過去に取得したhandleが新しいcheckpointへ勝手に結び付かないようにします。

名前で見つからない場合はtyped failure/Optionとして扱います。外部から受け取った文字列だけで、他moduleのprivateな保存値や別taskのstateへアクセスできるglobal reflectionにはしません。

## CP-02：CheckpointIdとcapability

lookupの結果はopaqueなCheckpointId等のhandleにする案です。runtime identity、checkpoint identity、世代を持ち、利用者がInt/Stringをconstructorへ渡して偽造することはできません。

同じlabelをdropして作り直す、beginへ戻す、branchが終了する等の境界では、古いhandleを明示的に無効と判定します。identityを再利用して別checkpointを指すことを避けます。

名前は発見用、handleは対象の同一性を保つために使います。read、restore、resume、releaseの権限も一つにまとめず、必要な操作だけを渡せるcapabilityを検討します。

## CP-03：handleによるrestore/resume

Stringを受ける便利入口はlookupとhandle操作の組合せとして扱います。動的に選べても、現行revert/resumeのcontinuation、scope、task、branch、external等の制約は維持します。

通常のlibrary関数内でcheckpointを作ると、そのcalleeの継続を保存してしまう可能性があります。呼出側の状態を保存するintrinsic、専用式、data snapshot APIのどれが適切かを区別します。「関数に包めば同じ」と仮定しません。

restoreすると、checkpoint後に作ったhandle変数や履歴List自体が消える場合があります。[checkpoint外の変数領域](v2-next-retained-state.md)があれば管理用stateを残す候補になりますが、全VMのrestoreを版付きcollectionのgetとして使う設計にはしません。budget、観測、公開済み作用は従来どおり戻りません。

## CP-04：現在を変えないCheckpointView

過去のVM stateをread-onlyに読む入口です。restoreせずに二つのcheckpointを比較し、現在の作業変数やtaskを保持します。

読みたい対象は明示的なtyped key/captureで登録する案を優先します。keyは単なる変数名ではなく、型と定義identityを持ちます。変数のshadowing、型変更、欠落を区別し、private境界を越えません。

戻り値は独立したimmutable版やFrozen値として扱い、過去heapへのmutable borrowを返しません。native connection、閉じたarena、実行継続を生きたresourceとして復元しません。reflectionで任意getter/callbackや外部作用を実行する入口にもなりません。

## CP-05：データだけのSnapshot<T>

永続データ構造をlibraryから作る用途では、全VM checkpointより、指定値だけのSnapshot<T>が扱いやすい案です。capture、read、fork/thaw、releaseを検討します。

共有できる部分は既存storage版を保持し、更新する部分はCOW等で分離します。深いgraph、循環、owner移動、Secret、native resourceの制約を明示します。Tの条件は既存Share/freeze契約を確認し、必要なら専用のsnapshot可能性を設計します。

以下は**将来APIを説明する擬似コード**です。snapshot/capture/read/forkは現在使えるsignatureではありません。

~~~text
let data = Map<String, Int>();
data.set("score", 10);
let v1 = snapshot.capture(&data);

data.set("score", 99);
let v2 = snapshot.capture(&data);

let old = snapshot.read(&v1);
let latest = snapshot.read(&v2);
// oldのscoreは10、latestのscoreは99。
// 現在のdata、現在の実行位置、履歴の一覧は戻らない。

let fork = snapshot.fork(&v1);
fork.set("bonus", 5);
// v1とv2を変更せず、v1から新しい版を作る。
~~~

captureが常にO(1)とは約束しません。pageの共有、freeze検査、graph走査、materialize等を区別し、実際の入力型ごとの費用を示します。

## CP-06：Versioned<T>と永続collection

Snapshot<T>を明示的にList/Mapへ保持し、版のparent、説明、利用者のversion IDを付けるlibrary案です。get(version)、fork(version)、diff(a,b)、merge(base,a,b)を用途ごとに検討します。

tree、graph、queue等の内部状態を保存し、各版のqueryを同時に行えます。commit labelを動的に増やす方式だけに依存せず、data rootだけを保持する方式と比較します。

mergeは値に対する明示的な規則を使います。外部DBや送信済みmessageをmergeしてなかったことにしません。任意Tに正しいmergeを自動生成できるとは仮定しません。

## CP-07：dropとview寿命の分離

名前の登録解除、restore可能なcheckpointの削除、data snapshotの参照解放を別の操作として整理します。

checkpointの登録をdropしても、既に取得した独立data snapshotがpageを保持する設計なら、その保持量は正当なuser rootとして会計します。古いCheckpointIdのrestoreは拒否しつつ、独立Snapshot<T>は自身の寿命で読めるという区分です。

registryやviewが互いに強参照し続けるcycleを作らないようにします。begin後にVM rootから消えたviewを、名前索引が意図せず保持し続けません。自動削除は利用者が所有を委ねた履歴の範囲だけにします。

## CP-08：reflectionの範囲

名前、作成位置、parent、公開状態、型、保持理由等のmetadataを見る案です。値へのアクセスはtyped key/capabilityとredactionを通します。

checkpointをserializeする場合は、opaque handle、call stack、physical resourceを一般dataと混同しません。data snapshotのexport/importと、VM continuationの保存は別の設計です。

## 利用シナリオ

- 任意件数のUndo/Redoを持つeditor。
- 複数版を同時にqueryするtree/graph/Map。
- simulationの途中から作る仮説branch。
- 設定やmodelの版比較と、採用前のpure検証。
- checkpoint名をschemaや入力eventへ対応させる履歴library。

## 調査するなら

最初はCP-05/06を現行freeze/共有storageでどこまで書けるか確認します。その後、CP-01/02の動的label管理、CP-04のtyped key、CP-03のcontinuation扱いを分けて設計します。

文字列lookupだけの小さな入口と、同時に読めるsnapshotは別の能力です。前者は履歴を選びやすくし、後者は版付きデータ構造を通常のlibraryとして組み立てやすくします。今は構想の保存だけで、compiler/runtimeは変更しません。
