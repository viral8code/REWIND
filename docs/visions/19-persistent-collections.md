# 永続collectionをlibraryとして組み立てる

現行の共有page、COW、freezeと、動的snapshotの構想を土台にします。保存済み版と、現在編集中のownerを操作として区別する発展案です。

## VPERSIST-01：transient builder

永続collectionを大量に更新するときだけ、独占ownerのbuilderへ変換する。完成後はimmutableなrootを返し、編集途中の参照を外へ漏らさずに済ませたい。

各操作で過去版を残す必要がない区間の費用を減らす。過去のrootが残る場合は共有している部分を変更しない。

## VPERSIST-02：構造を保つsplitとconcat

大きな列を切り分け、部分を組み替えても、未変更の内部nodeを共有する。文書やtimelineを編集するとき、すべての要素をcopyしないoperationが欲しい。

arrayとしてのindexと、treeとしての部分共有を両立する。sliceした版を持ったままconcatした結果も読めるようにしたい。

## VPERSIST-03：ordered mapの範囲view

keyの範囲を指定し、元Mapを全列挙せずにsnapshot viewを得る。境界の含め方とcompare契約を持ち、別版の同じ範囲を比べたい。

viewが保持するrootと、実際に必要なpageをprofileで見たい。範囲viewから派生した更新も明示できると便利。

## VPERSIST-04：挿入順とlookupを共有するMap

key lookupと挿入順の列挙を、別のversion管理を利用者へ押し付けずに扱う。削除と再挿入で順序がどう変わるかも契約へ含める。

orderedなkey比較と、挿入順の意味は分ける。表示・serializationで意図した順序を保ちたい。

## VPERSIST-05：版付きgraphのnode更新

node identityを保ちながら、edgeや属性を更新した新しいrootを作る。循環graphも扱い、未変更部分を共有したい。

同じnodeの旧新属性をqueryでき、削除済みnodeは不在として返す。過去版のnodeへ現在のmutable参照を混ぜない。

## VPERSIST-06：永続union-find

集合の統合結果を複数版で保持し、別branchから追加統合する。単にundoするrollback構造と、任意の版を同時にqueryする構造を使い分けたい。

path compression等の最適化がどの版へ作用するかを明示する。探索だけで旧版の意味が変わらない方式が欲しい。

## VPERSIST-07：複数indexを持つcollection

一つのrecord集合に、ID、名前、日時等のindexを持たせる。更新で一部indexだけが古くならず、同じversionのrootとして公開したい。

indexの追加・削除と、recordの変更を一つの編集単位にする。構築費用とquery費用を利用者が選べるようにしたい。

## VPERSIST-08：構造を知るChangeSet<T>

追加、削除、field更新を型付きの変更列として扱う。適用先version、逆差分、合成を持ち、Undoや同期へ同じdataを渡したい。

差分を表示するだけでなく、利用者のcodeから操作する。任意Tで正しいmergeができると仮定せず、対応型ごとの契約を使う。

## VPERSIST-09：同じsubtreeの共有cache

immutable subtreeのidentityやcontentから、同じpure queryの結果を再利用する。branchが違っても共通部分の計算を繰り返さない。

code versionとquery条件もkeyへ含める。保存値を保持するuser rootと、消えてよいcacheを区別したい。

## VPERSIST-10：圧縮辞書とtableの共有版

同じtextやcategoryをdictionaryへ集め、各版が共有する。少数の更新で巨大な列全体の辞書を作り直さずに済ませたい。

辞書の入替えとcategory IDの対応を保持する。内部IDが同じでも意味が違う版を混同しないようにしたい。

## VPERSIST-11：content-addressedなdata root

immutable dataをhashで識別し、共通部分を保存・転送する。checkpoint名ではなくdata identityとして、異なるruntime間で照合したい。

hashはアクセス権や公開許可にはしない。opaque resourceを対象から分け、data formatとschemaのversionも識別する。

## VPERSIST-12：履歴を要約して残す

古い版の全値は不要でも、変更理由やaggregateだけ残したい場合を扱う。利用者が選んだ履歴をsummaryへ置き換えるoperationが欲しい。

現在保持中のsnapshotを勝手に削除せず、何がqueryできなくなるかを示す。full版・差分・summaryを用途に応じて使い分けたい。
