# objectの合成・公開面・拡張点

partial、property、trait object等の候補を土台に、型の振舞いをどう組み立てるかを考える。class継承体系の導入を既定事項にせず、用途ごとに構成を比較する。

## VOBJECT-01：要件を宣言するmixin

methodの共通実装と、必要なfield/accessorを一つのfragmentにする。hostが要件を満たす時だけ合成できる。

fragmentが勝手にfieldを追加する方式と、hostの能力だけを使う方式を比較したい。private stateへのアクセス範囲と名前衝突は合成時に確認する。

## VOBJECT-02：明示的なdelegation

wrapperが内部の値へtrait methodを委譲する宣言。全部委譲する、選んだmethodだけ委譲する、一部を上書きする、を一覧で読める。

戻り値のowner、borrow、effectをそのまま検査する。委譲先の変更で公開APIが意図せず増える方式を避けたい。

## VOBJECT-03：閉じた実装集合

sealed traitや許可した実装一覧を用い、公開interfaceの実装集合を制御する。網羅性、最適化、進化の契約へ使う。

利用者が独自実装したいinterfaceは開いたまま残す。型の公開と実装権限の公開を別々に宣言できると便利。

## VOBJECT-04：部分型の合成manifest

partial fragmentの一覧、役割、生成元、hookの提供者をmanifestにする。合成後の一つの型だけでなく、構成の理由を閲覧したい。

file読込み順の代わりに依存graphで合成し、生成fragmentの欠落や同じhookの複数提供を診断する。

## VOBJECT-05：初期化の依存graph

初期化処理が必要なfieldと提供するfieldを宣言する。順序は依存から確認し、未初期化のthisを外へ出さない。

checkpoint外stateやphysical resourceの取得は明示段階へ分ける。初期化失敗時に返すownerとcleanup責任も読める設計を考える。

## VOBJECT-06：public viewを複数持つ型

同じ実装からread-only、editor、administrator等の異なる能力viewを渡す。利用者は必要な操作だけを受け取る。

viewは権限の狭いownerやborrowを持ち、reflectionが隠した能力を勝手に復活させない。別のdata copyを大量に作らず公開面を分けたい。

## VOBJECT-07：module単位のfriend能力

特定のhelperへprivate invariantの更新能力を渡す。file名で特権を決めず、明示した型付きtokenやmodule契約で扱う。

生成mapper等へ最小限の能力を渡し、tokenを持たないextensionからprivate fieldへ触れない構成を考える。

## VOBJECT-08：複数引数によるdispatch

形状同士の衝突、数値型同士の演算等で、両方の型から実装を選ぶ方式を考える。

候補の曖昧さとimportの影響を診断する。closedな集合をcompile時に解決する方式と、openなruntime登録を比較し、偶然の登録順へ依存させない。

## VOBJECT-09：identityと値比較の分離

同じinstance、同じEntity、同じ内容、同じ版を別の比較操作にする。object modelに合わせた既定の比較を宣言したい。

物理addressを永続identityにしない。内容比較の費用と循環graphの扱いもcontractへ持たせる。

## VOBJECT-10：readonly methodの強さ

fieldを書かないmethod、modelを変えないmethod、外部作用もないpure method等を区別したい。

readonlyなreceiverでも外部cacheを書けるか等をeffectとして表す。利用者が名前やgetterの見た目だけで純粋性を推測しなくて済む。

## VOBJECT-11：狭いscopeのextension

blockやmoduleで選択したextension集合だけを有効にする。domainごとの短い表現を、一つの大域的method集合へ押し込まず使いたい。

競合時は修飾して選べるようにする。importを一つ追加しただけで既存の呼出し先が変わる場合、その差を説明する。

## VOBJECT-12：操作を入れ替えるobject試行

validator、formatter、solver strategy等を一時的なviewで差し替え、同じdataに対する結果を比較する。

採用するのはstrategyやmodelの変更であり、旧objectのphysical handleを過去へ戻す操作ではない。diff・Trial・module signatureと組み合わせた体験を考える。
