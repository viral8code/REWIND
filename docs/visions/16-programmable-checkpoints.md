# プログラムから扱うcheckpointと複数版の値

ユーザー提案の動的commit名を出発点にした自由構想です。詳しい区分と擬似コードは[動的checkpoint・snapshot案](../v2-next-checkpoint-api.md)にあります。現行のlabel構文、List/Mapの永続storageを土台にする発展案です。

## VCHECK-01：文字列で履歴を作る・選ぶ

入力やcounterからcheckpoint名を組み立て、libraryで履歴を管理する。名前が存在するかを通常の結果として受け取り、固定labelをsourceへ列挙せずに済むと嬉しい。

## VCHECK-02：名前から安定したhandleを得る

名前lookupからopaqueなhandleを取得する。同じ名前を作り直しても古いhandleは別の版を指さず、無効になった理由を確認したい。

## VCHECK-03：現在を戻さずに過去の値を読む

二つのcheckpointをread-only viewとして開く。現在の作業変数や履歴Listを保ったまま比較できれば、過去を読むためのrevert/restore往復が不要になる。

## VCHECK-04：型付きの履歴key

変数名の文字列だけでなく、型と定義を持つkeyで保存値を選ぶ。shadowingやprivateを守り、取得する値の型をcompile時に分かるようにしたい。

## VCHECK-05：特定dataだけのsnapshot

Mapやtree等のrootだけをSnapshot<T>にする。全VM stateを戻さず、複数版を同時に保持・queryできる永続データ構造のlibraryへつなげたい。

## VCHECK-06：過去の版から編集する

immutableな過去のsnapshotを起点に、別のmutable版を作る。共有できるpageを保持し、元版と他branchを変えずに更新したい。

## VCHECK-07：版のdiff・merge

parentとstable keyを使い、値の意味に沿った差分を作る。利用者のmerge規則を渡し、Map・tree・文書等の版を選択して組み合わせたい。

## VCHECK-08：履歴のregistryと寿命

名前の登録、復元能力、snapshotへの参照を分ける。labelをdropしても独立snapshotを保持する場合のmemoryを見られ、意図した寿命で履歴を解放したい。

## VCHECK-09：checkpoint外の変数領域

巻き戻されない変数を宣言し、履歴catalog、試行回数、探索統計等を保持する。通常modelをrevertしても管理用stateは残し、可変値の所有領域・GC・beginとの関係は明示したい。詳しくは[checkpoint外state案](../v2-next-retained-state.md)を参照。
