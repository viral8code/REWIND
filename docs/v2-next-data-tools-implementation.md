# 履歴・変更操作libraryの実装記録

更新日：2026-10-12。状態：**source実装済み・実行検証省略**。ユーザーの方針により、環境起動失敗を待たずに実装を進める。compile、test、性能、API生成を確認済みとは扱わない。

既存[std.snapshot / std.history](v2-next-snapshot-implementation.md)から、[永続collection構想](visions/19-persistent-collections.md)と[履歴graph構想](visions/48-history-graphs.md)のdata操作へ進める。全VM checkpointや外部操作を扱うものではない。

## std.timeline：bounded Undo/Redo

初期値とcapacityを受け、現在位置とsnapshotの列を保持する。capacityは現在版を含めて1〜65,536。current、get、seek、undo、redo、length、position、canUndo、canRedo、push、pushSnapshot、clearを提供する。

undo/redo/seekは選択位置を動かしてsnapshotを返す。呼出側modelの更新はsnapshot.fork等で明示する。実行位置、外部ledger、global変数を自動的に戻さない。

pushはcapture後にRedo側を破棄して新しい版を末尾へ置く。fullなら最古版を一つ外す。取得済みsnapshotには影響しない。pushSnapshotは既存rootを保持してcaptureの再実行を避ける。clearは現在版を一件だけ残す。

保存領域はMapのring indexを使う。前方の全要素を移し替えずにevictする。Map操作の費用は既存実装に従う。Redoをk件捨てるpushはk件の登録解除、captureは既存freezeの費用を持つ。fatal budget failureのatomic recoveryは追加しない。

indexは最古の保持版を0とした位置で、evict後は変わる。永続identityや他timelineへのhandleとして使わない。Timeline自身は通常modelで、commit/revertの対象になる。

### 確認用source

libraries/std/tests.rwにring周回、容量1、undo/redo境界、Redo破棄、保持済みroot、clear、不正capacity/indexを用意した。実行は省略。

## std.changes：型付きMap差分とmerge

VPERSIST-08、VHISTORY-04/06のうち、String keyのMapに対象を限定した実装。Change<T:Share>はkey、変更前Option<T>、変更後Option<T>を持つ。ChangeSet<T>はprivateなFrozen<List<Change<T>>>を保持する。

fromChangesは重複keyと65,536件超を拒否し、指定した等値関数でno-opを除く。diffは二つのMapのkey順を二方向に走査して追加・削除・更新を作る。editsとlengthで内容を読める。inverseはbefore/afterを交換する。

applyは全変更のbeforeを照合してから新しいMapを返す。元Mapを変更せず、patchが触らないkeyを保持する。別branchへの適用もこの前提検査を通す。対象Map全体のidentity一致やdomain不変条件の検証を自動的に代行しない。

compose(first,second)は同じkeyのfirst.afterとsecond.beforeを検査し、一つの差分へまとめる。相殺した変更は除く。merge(base,left,right)は三方向のMap比較で、独立したkey変更と、同じ最終値への変更を統合する。削除と更新等、異なる最終値はConflictとして返す。入力は変更しない。

比較関数はpureなSend+Share callbackで、呼出側が等価関係を保証する。浮動小数点の近似一致等、推移律を満たさない規則を無条件に安全なmerge規則として扱わない。errorはcodeとkeyを持ち、旧新の値を含めない。

diffはkey列を走査し、Map lookupの費用を持つ。applyは新しいMapを構築するため全入力keyをcopyする。root共有だけでO(1)適用できる機能として宣伝しない。65,536件の上限は差分件数で、値のbyte量や入力Map全体のmemory上限ではない。freeze、callback、既存VM予算の制約は残る。

library test sourceに、逆差分、未変更keyの保持、衝突時の入力不変、合成の相殺、重複key、独立変更のmerge、同一変更と衝突を追加した。実行は省略。

### 変更列のrebase preview（2026-10-12）

VHISTORY-05のString Map限定部分としてpreviewRebase(base,onto,patches,same)を追加した。最大1,024 step、累積65,536 edit。純粋な値の操作だけを移し、外部operationやreceipt、VM checkpointへ作用しない。

先に全patchが元のbaseから順につながることを検査する。最初に前提が違うstepをRebaseError("OriginalConflict",key,step)で返す。変更済みkeyだけをoverlayに持ち、検証のたびに全Mapを再構築しない。新しい土台で先に衝突する場合でも、元の列の不整合を隠さない。

新しい土台では、値がbeforeと等しければ変更を採用する。既にafterと等しければ適用済みとして、そのkeyを再適用patchから除く。それ以外は衝突とし、当該stepのどの変更も反映しない。成功prefixのsteps、最後のvalue、最初のconflict（keyと0始まりstep）をRebasePreviewで返す。後続stepは未評価。conflictがNoneなら全stepが完了している。

各RebaseStepはindex、再適用用のpatch、直後のFrozen Mapを保持する。全変更が適用済みのstepも空patchと結果を一件返す。元patch、元base、ontoは変更しない。全step後に採用するか、保存するか、domain不変条件を再検査するかは呼出側が明示する。汎用的なdomain merge lawを導いたことにはしない。

新しい土台のMapは最初に一回copyし、以降は同じ作業Mapへ変更を加えて各結果をfreezeする。毎step全keyをlibraryのloopでcopyする方式を避ける。freeze・COW・Map・payloadの費用とVM budgetは別に残る。結果は途中の全snapshotを保持するため、step上限はmemory量の上限ではない。必要な値だけを取り出しpreviewのownerを解放すれば、未保持の途中rootは通常の回収対象になる。

library test sourceを3件追加した。適用済みkey、段階ごとの値保持、未変更key、途中衝突のstep atomicity、元列の不整合、削除→再追加、空列を扱う。実行は省略。

## std.historyGraph：親関係・branch HEAD・annotation

VHISTORY-01/02/03、08/10の一部を実装する。HistoryGraph<T:Send>はprivateなversion registry、branch HEAD、annotationを持つ。createでversion件数とbranch件数の各上限を1〜65,536に設定する。

add/addSnapshotは1〜128 Unicode scalarの一意な名前で版を追加する。親は既存の異なる版を最大16件、指定順で保持する。既存版の親を書き換える入口がないので追加でcycleを作れない。名前はそのgraph内だけのkeyで、VM全体のlabelや実行checkpoint IDではない。

Version<T>はprivateなsnapshotを自身で保持するimmutable record。get/headで取得した後にHEADを動かしたりbranch名を削除・再利用しても、取得済みVersionが別dataへ変わらない。name、parents、snapshotOf、read、forkで読む。別graphの同じStringを、同一版の証明として扱わない。

setHeadは版の作成と独立。存在しない版を指すHEADを拒否する。dropHeadは名前だけを解除し、data版を削除しない。versions/branchesは独立したkey列を返す。

annotateは説明4,096 scalarと著者128 scalar以内のmetadataをdataと別に置く。annotation取得後のrecordも独立値。完全なannotation変更streamや著者認証は今回の範囲に含めない。

isAncestorは自分自身をancestorに含め、queueとvisited mapで探索する。各到達版は一度だけenqueueし、深い履歴を再帰関数で辿らない。matchingはpure predicateで各snapshotを調べ、名前順のVersion列を返す。

Graphは通常のVM model。revertでregistry、HEAD、annotationも戻る。全rootを利用者の保持として所有し、勝手に古い版を削除しない。容量に達したらtyped errorで止まる。2026-10-12に、明示的なpruneとprotected rootを追加した。squash、pack、跨runtime identityは含まない。

library test sourceに、分岐と複数親、ancestor、HEAD移動、独立Version、annotation、predicate query、欠落親・重複親・重複版・欠落HEADの拒否を用意した。実行は省略。

### 保護rootと明示的な整理（2026-10-12）

VHISTORY-09と11の整理の一部としてpin、unpin、pins、ancestors、previewPrune、pruneを追加した。pinは既存版だけを受け、同じ版への重複pinは一件として扱う。pinsは名前順の独立List。unpinはHEADを変更しない。

pruneは現在の全HEAD、pin、明示したkeep rootの和から親を辿り、到達しない版とそのannotationを取り除く。親を残すため、保持した版のedgeが欠落しない。HEADもpinもkeepも空なら全版を整理する。自動GCや容量不足を契機に実行するものではない。

previewPruneは削除予定の名前順Listだけを返す。pruneはそのListを承認tokenとして受け取らず、呼出し時点のrootで再計算する。preview後に追加したHEADやpinを無視して削除しない。keepは重複を許すが入力は65,536件まで。存在しないrootはUnknownVersionで、変更前に拒否する。

ancestorsは指定root自身を含む祖先の和を名前順に返す。重複rootや複数親があっても一度だけ探索する。文字列Listはsnapshotを所有しない。

pruneは保持用の新しいregistryとannotation mapを組み立ててから置換し、通常のtyped errorでは元graphを変更しない。探索はversion/edgeを一度ずつ訪ね、Map lookupの費用を伴う。整理中は新旧のMapを一時保持する。fatal予算失敗までのatomic recoveryは保証しない。

外で取得済みのVersion/Snapshotはrootを保持し続ける。pruneした名前は再利用できるが、取得済みVersionが新しい値へ変わることはない。name/parentsはgraph内のlookup文字列であり、削除・再利用後に古いVersion.parentsの文字列を現在graphへ解決して同じ版だと判断しない。graph内の祖先情報まで保持したい場合はその版をpinする。

library test sourceに、HEADとpinからの祖先保持、annotation除去、preview後のHEAD追加、欠落root時の入力不変、明示keep、名前再利用と取得済みrootの独立性を2件追加した。実行は省略。

### 共通祖先と経路（2026-10-12）

VHISTORY-07/10のgraph queryとしてmergeBasesとpathを追加した。mergeBases(left,right)は共通祖先のうち、別の共通祖先の祖先ではない版をすべて名前順に返す。criss-crossの履歴では複数件になり得る。無関係な履歴は空List。片方がもう片方の祖先ならその版自身、同じ版同士なら一件になる。単一候補を恣意的に選んでdata mergeを実行しない。

両側の祖先集合の共通部分は親をすべて含むため、その共通部分の各版の直接の親を除くことで最も近い候補を求める。候補ごとに全祖先を再探索する方式を避ける。到達version/edgeに比例した走査とMap操作を行う。

path(ancestor,descendant)は最短の親edge経路をOk(Some(List<String>))で返す。順序はancestor→descendant、両端を含む。到達できない場合はOk(None)、存在しない端点はUnknownVersion。後ろからBFSし、同じ長さの経路は保存された親の順序で選ぶ。再帰せず、各到達版を一度だけqueueへ置く。

返す名前列はdata rootを保持しない。取得後にpruneや名前の再利用があった場合、現在のgraphへ無条件に解決して同じ版と判断しない。必要な版はpinまたはVersion/Snapshotの取得で保持する。field単位の差分queryや任意の全経路列挙は含まない。

library test sourceを2件追加した。複数merge base、祖先自身・同じ版・独立履歴、最短経路、親順によるtie、欠落端点、到達不能を扱う。実行は省略。

## std.orderedMap：挿入順とlookupを一つのownerで保持

VPERSIST-04のString key / Share valueの基本部分を実装する。OrderedMap<T:Share>のprivateなMapに値と前後のkeyを置く。createの容量は1〜65,536。put、get、contains、remove、keys、values、entries、length、clearを提供する。

既存keyのputは値を更新して位置を保持する。削除後の再挿入は末尾へ追加する。容量上限でも既存keyの更新は成功する。欠落keyの削除はNoneで順序を変えない。列挙結果は独立したList。

隣接関係は文字列keyで表し、mutableな相互参照や別ownerの順序Listを利用者へ渡さない。通常Mapのkey順とは区別する。put/removeは少数のnative Map lookup/updateで前後を繋ぎ替え、後続要素を全シフトしない。列挙はn件のMap lookupを伴うため、native linked-listのO(n)走査と同じ費用を約束しない。

通常modelとしてfreeze/thaw、snapshot、commit/revertへ従う。高度なpersistent treeのsplit/concatや専用native iteratorは未実装。privateなlinksの更新中にfatal budget failureが起きた場合までtransactional recoveryを追加するものではない。

library test sourceに、非key順の挿入、full時の更新と新規拒否、head/middle/tailの削除、再挿入、clear、snapshot後の順序・値の独立性を用意した。実行は省略。

## 保存・検証方針

codex/developだけへ小さくcommit/pushする。commitには[skip ci]を付ける。公開CI/CD、main統合、tag、Release、版更新を行わない。

将来の実行確認はcargo run -- test --root libraries/stdと、追加example、API snapshot生成から始める。実行を省略した事実を、検証済みや全構想完了へ読み替えない。
