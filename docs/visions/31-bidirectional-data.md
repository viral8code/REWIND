# 深い値の操作と双方向の変換

lensの構想を、永続data、型付きUI、migrationへ広げる。深い構造を更新する時に、copy範囲・欠損・履歴・逆変換の条件が読める体験を考える。

## VOPTIC-01：型付きのfield path

FieldPath<Root, Value>を値として組み立て、getter、validator、form、query等へ渡したい。文字列のfield名より、renameに追従する経路を使う。

reflectionから得た動的経路は、RootとValueの一致を検証してから同じ操作へ使う。型付きpathが任意のprivate memberを開く能力にはしない。

## VOPTIC-02：欠損を持つ経路

Option field、Map key、variantを通る経路に、欠損し得る性質を持たせる。取得結果も更新結果も、存在しなかった場所を返せる。

値がなければ何も変更しない操作と、欠損部分を生成する操作を分ける。中間objectの自動生成を利用者が選びたい。

## VOPTIC-03：複数箇所へ届くtraversal

条件に合う全要素を一つのtyped traversalで更新したい。要素ごとに返るvalidation errorには、元の経路を残す。

変換対象がaliasで同じ値を指していた場合の更新回数と順序を定義する。結果が列挙順の偶然に依存するなら表示する。

## VOPTIC-04：経路を使うChangeSet

field pathへ旧値・新値・前提版を付けた変更を作る。適用前に、対象の型と前提が一致するか確認できる。

List indexの移動に弱い変更と、Entity IDを基準に追従する変更を分ける。経路が型付きでも、別版で同じ意味の対象とは限らない。

## VOPTIC-05：構造共有を保つ深い更新

大きいtreeの一部を書き換える時、path上のnodeだけを作り直したい。利用者は複雑なcopyコードを書かず、共有された部分も費用として確認できる。

複数経路の更新をまとめ、共通prefixのcopyを一度にするbuilderも考える。生きたcheckpointのrootは変更しない。

## VOPTIC-06：法則を持つlens

get後に同じ値をsetする、set後にgetする、setを繰り返す、といった法則をlensの定義へ添えたい。

property testを導出できる形にする。丸め・正規化を伴うlensは厳密な法則と異なるため、別のcontractとして宣言する。

## VOPTIC-07：正規化を返す編集変換

入力文字列から値を作り、整形済み文字列と入力位置の対応も返す。利用者は入力中の状態と、確定したmodelを分けて保てる。

保存用の正規形だけへ置き換えて、入力途中の小数点やIME compositionを失わない変換を考える。

## VOPTIC-08：失われる情報を持つcomplement

双方向変換でviewに出ない情報をcomplementとして保持し、viewからの変更を元のmodelへ戻す。

そのcomplementがどのmodel版に対応するかも持たせる。古いviewからの更新で、別の変更を黙って上書きしない。

## VOPTIC-09：mergeの対象を小さく切る

model全体の衝突ではなく、同じtyped pathへ異なる変更が入った場合だけ、解決候補を返したい。

domain上連動するfieldは一つの更新unitとして扱えるようにする。fieldごとのmergeが型を保っていても、不変条件を満たすかは再検証する。

## VOPTIC-10：不変条件を守るfocus

amountとcurrency、開始と終了時刻等、組で扱う必要がある値にfocusを定義する。更新用callbackは整合した組を返す。

単独fieldへのsetを禁止するprivateなmodelにも、安全な編集能力だけを公開できると便利。

## VOPTIC-11：schema間の双方向adapter

異なるschemaの対応をpathと変換で記述し、読込み・書戻し・未対応fieldを一覧にしたい。

可逆な変換、損失のある変換、片方向だけの変換を区別する。migrationの逆方向が未定義なのにUndo可能と表示しない。

## VOPTIC-12：pathから導く観測と説明

同じtyped pathから、watch、UI binding、diff表示、validation messageの位置を得る。どのfieldが変化したかを一貫した識別子で扱う。

値を取得するたびにgetterが外部作用を起こす設計にはしない。pathの観測は純粋なmodelへのアクセスを基本にする。
