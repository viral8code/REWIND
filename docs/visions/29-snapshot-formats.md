# snapshotの保存形式・共有・復元

portable state保存、content-addressed root、既存model formatの構想を、dataの具体的な保存・読取りへ広げます。任意のVM継続やphysical resourceを復活させるformatではありません。

## VFORMAT-01：page単位のsnapshot pack

値、型、shared page、indexを一つのpackへまとめる。全値を巨大なtextへ展開せず、必要な部分だけ扱える保存形式が欲しい。

schema、language、codecのversionを分ける。保存したcodeとdataの互換も明示して確認する。

## VFORMAT-02：共有graphを保つencoding

同じobjectを複数箇所が参照する関係と、循環をformatで表す。単純なtree serializationで共有を失い、memoryが増える問題を避けたい。

復元後もdata identityとimmutable共有を保つ。file内IDとheap addressは別のものとして扱う。

## VFORMAT-03：numeric viewの保存

base配列、shape、stride、offsetを保存して、view同士の共有を再構成する。logicalな列だけを書出す方式と選べると便利。

raw pointerを保存するのではなく、ownedなdata rootからviewを作る。書込み可能性とsnapshotの版も保持したい。

## VFORMAT-04：一部だけ読むtyped reader

巨大なpackから特定record、column、subtreeだけを読む。indexとchecksumを使い、全materializeしないAPIにしたい。

未読範囲の検証状況も示す。読んだ値を独立snapshotとして保持できるかを型と寿命で表す。

## VFORMAT-05：baseを持つdelta pack

共通snapshotに対する変更だけを保存・転送する。base identity、変更順序、適用後identityを持つformatが欲しい。

baseが違う場合は拒否または明示mergeへ進む。任意の最新dataへ黙って差分を適用しない。

## VFORMAT-06：未知fieldを保持するreader

新schemaのfieldを旧codeが理解できなくても、保管・再出力できるdataを持つ。理解した値とopaqueな部分を区別したい。

未知値をtypedな正常値として使わない。容量・深さ上限を保ち、処理していない情報を表示する。

## VFORMAT-07：logical digestとstorage digest

同じlogical dataと、同じbyte表現を別のdigestで識別する。normalization、field順、型versionがどちらへ影響するかを決めたい。

数値の表現やtextの正規化を黙って変えない。比較の目的に合うidentityを利用者が選ぶ。

## VFORMAT-08：schema mappingのformat契約

旧新schemaをつなぐconverterを、入力/出力versionと一緒に保存する。どのfieldが移行・default・削除されたかを結果へ添えたい。

実行するcodeと、単なるmapping dataを区別する。loaderが勝手に任意generatorを起動する方式にはしない。

## VFORMAT-09：公開viewと保護fieldのexport

内部dataから、公開してよいviewだけを出力する。Secretやcredentialを通常serializationへ流さず、利用者がfield単位で契約を作りたい。

暗号化して保存する場合も、keyの寿命・取得方法は別にする。formatがあるだけで全rootを公開可能とは扱わない。

## VFORMAT-10：中断を検出できる保存

chunk、manifest、最終marker等から、保存が完了したかを確認する。途中のpackと利用可能なsnapshotを区別したい。

fileのflush・atomic rename等のplatform契約を使う。複数fileやremote objectを一括atomicに扱えるとは仮定しない。

## VFORMAT-11：storage上のroot管理

保存済みsnapshotのroot、参照pack、保留中の書出しを一覧にする。未参照dataの整理を、利用者が持つ版の削除とは別に扱いたい。

他runtimeが読むdataや共有baseを保持する。参照追跡、lease、削除policyを選び、cleanup候補をpreviewできると便利。

## VFORMAT-12：import後のidentity再構成

dataを新runtimeへ取り込むとき、復元可能なidentityと無効なhandleを区別する。collection/nodeの関係は保ち、元runtimeの権限は引き継がない。

外部resourceは再接続用の非機密設定や説明だけにする。importでSQLや送信を自動再実行しない。
