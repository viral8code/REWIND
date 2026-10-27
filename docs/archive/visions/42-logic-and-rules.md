# 論理query・rule・制約のlibrary

logic programming、knowledge query、ruleの構想を、型付きdataと版へ結び付ける。宣言的な関係を、通常のREWIND関数と一緒に扱いたい。

## VLOGIC-01：型付きの論理変数

LogicVar<T>とunificationをlibraryの値にし、違う型の値を統合しない。未束縛、束縛済み、解の中だけで有効な変数を区別する。

探索中のbindingは候補stateに属する。解として返す時は普通の値へmaterializeし、内部のtrailへの参照を逃がさない。

## VLOGIC-02：occurs checkのcontract

tree termの循環を禁止するunificationと、明示的なcyclic termを許す方式を選ぶ。

循環を許したtermの比較、表示、serialization、GCを同じcontractで扱う。再帰したからというだけで無限のpretty printへ入らない。

## VLOGIC-03：関係を返す関数

predicateがboolだけでなく、bindingを持つsolution streamを返す。通常の関数から問い合わせ、patternで結果を使いたい。

解の順序、重複、有限性の保証をpredicateの能力として示す。無限列挙ではfairなstrategyを選べるようにする。

## VLOGIC-04：negationの意味を選ぶ

存在しないと確認できた否定、まだ証明できていない状態、矛盾を別に扱う。finiteな閉じたdatasetと、開いた知識sourceで同じ否定を使わない。

stratified rule等、意味が明確な部分集合を選べるprofileを考える。再帰と否定の循環は説明付きで診断する。

## VLOGIC-05：ruleの依存graph

predicate間の正・負の依存をgraphにし、評価段階とfixed pointの範囲を表示する。

aggregateや外部関数の能力もedgeへ持たせる。rule追加で評価の性質が変わる場合、その影響を差分として読みたい。

## VLOGIC-06：data変更から差分推論

factの追加・削除に合わせ、関連する導出だけを更新する。現在版と旧版のderived relationを構造共有で持ちたい。

削除の扱いには、同じ結論を支える別の根拠も含める。factを一つ消しただけで、他の証明を持つ結論まで消さない。

## VLOGIC-07：解の根拠を返す

solutionに、使ったfact、rule、条件のproof graphを付ける。「成立する理由」を利用者が確認できる。

すべてのproofを無制限に保持せず、代表的な根拠、圧縮graph、要求時の再構築を選ぶ。説明が省略されている範囲も示す。

## VLOGIC-08：不成立の説明候補

必要なfact、失敗した条件、未探索のbranchを区別して返す。成立しなかった理由と、調べ切れていない理由を混ぜない。

最小の修正案をsolverへ依頼する場合、その案の前提と探索範囲を残す。外部dataが不足した場合の推測は別variantにする。

## VLOGIC-09：純粋な外部predicate

REWIND関数をpredicateへ接続し、決定性、有限性、memoization可能性を宣言する。

network lookup等は観測dataを先に取得する別段階へ置く。探索のbacktrackで物理呼出しを何度も繰り返す構成を既定にしない。

## VLOGIC-10：tableと制約を組み合わせる

関係queryで候補を絞り、数値制約solverへ渡し、結果を関係として戻す。database joinとsolverの境界を計算planにする。

値の型、単位、精度、解の完全性を境界で維持する。近似solverの候補を、exactな論理証明と同じ扱いにしない。

## VLOGIC-11：解と最適化目的

scoreやcostを持つ解を列挙し、best-first、branch-and-bound等のstrategyで調べる。

最良値の更新はcheckpoint外catalogへ残せる。最適性を証明できた場合と、budget内で最良の解を別結果にする。

## VLOGIC-12：rule版の比較

同じfact snapshotへ異なるrule版を適用し、新たに成立したもの、消えたもの、根拠が変わったものを並べる。

rule変更による結論と、入力変更による結論を区別する。readonlyの解比較から、実行するoperation planを別に作りたい。
