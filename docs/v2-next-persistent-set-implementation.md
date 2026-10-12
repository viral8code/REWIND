# 複数版を同時に保持するunion-find

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[VPERSIST-06](visions/19-persistent-collections.md)をstd.persistentSetへ実装した。既存std.disjointSetの経路圧縮付きowner、std.rollbackSetのmarkまで戻すownerは維持する。新しいAPIは任意の旧版から新しい版を作り、複数の結果を同時にqueryするimmutableなデータ構造。

## 公開値と操作

PersistentSetはprivateなparents/sizesのIntArrayと、要素数・成分数を持つrecord。createは0〜1,048,576要素を受ける。要素IDは0始まりで、作成後に要素の追加・削除は行わない。

length、components、root、same、sizeは版を変更しない。rootはunion-by-sizeの親を辿り、経路圧縮をしない。代表要素の番号はunionで変わる場合があるため、成分の永続identityとして扱わない。

unite(set,a,b)はUnion{state,merged,root}を返す。同じ成分ならmerged=falseで既存stateを返す。異なる成分なら小さい成分を大きい方へ接続し、サイズが同じ場合は小さい代表番号を親にする。引数順を反転しただけでtie結果を変えない。

uniteManyは最大65,536個のTuple<Int,Int>を順に統合し、最後の版を返す。途中で失敗しても元のsetは変更しない。中間版の全履歴を自動保存しない。途中版が必要な場合はuniteの結果を利用者が保持する。

## 共有pageと費用

初期化は既存numericRange.integersで二つのnative配列を作り、要素ごとのVM List値を構築しない。更新はnumericIndex.withIntを使い、親の一要素と代表のsize一要素だけを新しい配列版へ書く。

既存native配列のCOWにより、各書込みは256要素のpageとそのtree pathをcopyする。配列全体をthawして全要素を再構築しない。旧版と新しい版は未変更pageを共有する。単独の同一成分unionはpage更新をしない。

union-by-sizeにより木の高さはO(log n)。rootは各段階でnative配列のlookup費用を伴い、uniteは二回のroot探索と二つのpage更新を行う。初期化はO(n)。rank=1の配列を使い、queryごとにcoordinate Listを作らない。ここでの計算量は既存storage契約からの記述で、実測の速度保証ではない。

利用者が保存した版は共有pageをrootとして保持する。不要な版のownerを手放すと、その版だけが保持していた領域は既存の回収対象になる。GC・共有pageの計測は今回実施していない。件数上限はmemory予算の代わりではなく、作成時・更新時に既存のnative/VM予算も適用される。

## 失敗と巻き戻し

SetErrorはcodeとindexを持つ。Capacityは要素数、Rangeは要素ID、BatchLimitは指定したpair件数をindexに入れる。native数値配列の失敗は元のcode/offsetを引き継ぐ。fatal予算失敗は通常のResultへ変換しない。

純粋なVM dataであり、Snapshotや通常のcommit/revertに従う。履歴registryやhyper領域へ自動登録するものではない。snapshotを作ることなくPersistentSet値をそのまま保持しても、その版のqueryは独立して行える。

## 確認source

libraries/std/tests.rwへ3件追加した。旧版からの別branch、後続union、サイズと成分数、重複union、tie、読取りの不変、snapshot、空集合・不正要素・件数上限、batch途中失敗を扱う。実行は省略。

利用例は[persistent-union-find.rw](../examples/persistent-union-find.rw)。SDKのmodule registryへ追加したが、SDK/API生成・公開は行っていない。
