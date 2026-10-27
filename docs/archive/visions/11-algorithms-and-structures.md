# 高度なalgorithmとデータ構造

既存graph/flow/tree/文字列等のlibraryを不足と断定する一覧ではありません。より高度な操作や統合的な使い方を考える自由構想です。

## VALGO-01：動的graphの連結性

edgeの追加・削除が続くgraphで、連結成分や到達性を調べる。全graphを作り直さず更新でき、変更履歴とquery結果を比較したい。

## VALGO-02：動的な木のpath query

link-cut tree等で、木の接続変更とpath集約を扱う。clusterの構造と操作の費用を可視化し、複雑な実装の途中stateを調べたい。

## VALGO-03：succinctな列とrank/select

bitvector、wavelet系の構造で、範囲内の順位・頻度・値検索を行う。圧縮率とquery費用を見ながら、大きなimmutable列を小さく保持したい。

## VALGO-04：圧縮text index

FM-index等で大きなtextを検索し、元位置へ戻す。index構築とqueryを分け、byte/scalar/tokenのどの単位を検索しているか明示したい。

## VALGO-05：多項式と高速畳込み

係数型、modulus、長さ、丸め条件を指定して演算する。FFT/NTTやexact arithmeticの選択を確認し、計算結果の意味を型で保ちたい。

## VALGO-06：最適化したDPの道具

Li Chao tree、convex hull trick、divide-and-conquer最適化等を、適用条件と一緒に使う。条件が成立しない入力を見つけ、素朴な参照解との比較を行いたい。

## VALGO-07：exact coverと組合せ探索

候補、制約、分岐、剪定をmodelとして扱う。探索木を記録から調べ、heuristic変更でどのbranchが減ったかを見たい。

## VALGO-08：robustな幾何predicate

近接点、orientation、intersection等を、浮動小数点誤差に強いpredicateで判定する。近似計算とexact fallbackのどちらが使われたかを調べたい。

## VALGO-09：位相的なデータ解析

simplicial complex、homology、persistent featureを扱う。scaleを変えたとき残る構造を可視化し、数値datasetから幾何・位相の分析へ進みたい。

## VALGO-10：decision diagramと論理回路

BDD等で条件の組合せを圧縮して扱う。変数順序、共有node、simplificationを調べ、複雑な判定条件をqueryや検証へ接続したい。

## VALGO-11：monoid/semiringを使うalgorithm

最短path、数え上げ、matrix積、区間集約を共通の代数的interfaceで表す。利用者が演算と単位元を定義し、同じalgorithmを別の値へ適用したい。

## VALGO-12：external-memory algorithm

sort、merge、index構築を、RAMより大きい入力へ使う。I/O量、temporary file、再開位置をplanとして表示し、stream処理と接続したい。

## VALGO-13：meld可能な永続構造

priority queue、tree等を複数のrootで保持し、split/merge/meldする。VMのcheckpointだけでなく、algorithm自身が複数の版を操作する用途に使いたい。

## VALGO-14：再現可能なrandomized algorithm

sampling、hash、sketch、近似計算を、seedとerror条件を持つ結果として扱う。失敗確率と再現用情報を残し、結果が変わった理由を調べたい。

## VALGO-15：matroid等の組合せ最適化

独立性、交換、制約をinterfaceとして扱い、選択問題を解く。単なるsolver呼出しではなく、成立する前提と反例を利用者が確認できるようにしたい。

## VALGO-16：入力特性によるalgorithm選択

size、密度、順序、必要精度から候補を比較する。選択理由と参照解を表示し、利用者が固定algorithmへ切り替えられる説明可能なhelperにしたい。
