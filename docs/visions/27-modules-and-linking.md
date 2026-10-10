# module・生成code・linkの表現

moduleを単なるfileの分割だけでなく、型と能力を持つ構成単位として扱う自由構想です。既存project管理を直ちに大型化する計画ではありません。

## VMODULE-01：独立したmodule signature

実装とは別に、型・関数・effect・公開契約を宣言する。callerはsignatureを見て使い、実装のprivateな詳細へ依存しない構成にしたい。

signatureと実装の一致を確認し、docも同じ契約から生成する。名前だけ一致する不完全なadapterを避けたい。

## VMODULE-02：parameter付きmodule

数値型、storage、時計等のmoduleをparameterとして受け取る。Matrix(Element)やRepository(Backend)のように、一群の型と操作をまとめて共通化したい。

普通のfunction genericだけでは繰返す関連型・helperを構成単位にする。parameter moduleのeffectとidentityも保持する。

## VMODULE-03：localなmodule定義

小さな型とhelper群を、利用するscopeの近くにまとめる。専用fileを増やさず、公開しない名前のまとまりを持ちたい。

外の値を捕捉する場合は契約を示す。名前空間だけのmoduleと、runtime stateを持つcomponentを区別する。

## VMODULE-04：versionを指定するimport

異なるlibrary版を明示aliasで使い、移行adapterを同じprogramで比較する。型identityが異なる版を、同じ見た目だからと混用しない。

codeとdata formatの変換を明示する。runtime/native依存が共存できない場合も、compile時に説明できると便利。

## VMODULE-05：契約から解く相互依存

複数moduleが互いの型やsignatureを参照する場合、先に契約を解決する。file読込み順に依存しない型検査を考えたい。

runtime initializerの循環は別に検査する。相互再帰が表現できることと、初期化が終了することを混同しない。

## VMODULE-06：typedなcode quotation

codeを単なるStringではなく、型・effect・source位置を持つsyntax値として組み立てる。埋込みDSLやgeneratorが通常のcheckerへ渡せると嬉しい。

名前captureやprivateアクセスは明示する。生成前後の構造をeditorで調べたい。

## VMODULE-07：生成codeのorigin graph

生成関数から、入力schema、template、手書きhookへ辿る。errorが生成fileの一行だけで終わらず、原因の宣言へ戻れるようにしたい。

複数stageの生成もgraphとして保持する。修正すべき入力と、直接編集してよい出力を区別する。

## VMODULE-08：説明可能なlink plan

どのmodule/impl/native adapterが選ばれたかを表示する。同名の候補、generic特殊化、capabilityの依存をqueryしたい。

単に解決できたという結果ではなく、選択した根拠を示す。artifactとtraceにも必要なidentityを持たせる。

## VMODULE-09：signature付きの動的関数lookup

名前で関数を探す場合も、期待する引数・戻り値・effectを指定する。reflection的な操作からtypedなcallableを得たい。

文字列が一致するだけで型検査を回避しない。関数のvisibility、version、capture、必要能力を確認してから呼ぶ。

## VMODULE-10：capability付きplugin capsule

pluginのcode、signature、許す操作、容量、lifecycleを一つのpackageとして扱う。application全体のresourceを無条件に渡さずに拡張したい。

pluginが要求する能力とcallerが提供する能力を照合する。load/unload時に残るtaskやsnapshotも説明したい。

## VMODULE-11：link互換を検査する差替え

moduleを更新するとき、signatureだけでなくfield layout、effect、関連型等を比較する。変更が安全な部分だけを差し替えたい。

旧版で動くcall frameやcheckpointがあれば、その版を残す。data移行や再接続が必要な場合は別の操作にする。

## VMODULE-12：用途を限定するlanguage profile

埋込み式、rule、template等で使える構文・effectをprofileとして選ぶ。同じcheckerの基盤を使いながら、不要な能力を受け付けないmodeが欲しい。

profileは曖昧な「安全なcode」というラベルではなく、許す操作を列挙する。通常REWINDへ変換する場合も型とsource位置を保つ。
