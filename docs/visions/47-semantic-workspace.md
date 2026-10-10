# 意味を持つsource workspace

semantic code databaseやrefactoringの構想を、編集途中のsource、生成元、実行中の版へつなぐ。現行editor支援を置き換える宣言ではなく、追加体験を考える。

## VWORK-01：編集途中の型付きhole

未完成の式をHoleとして保持し、周囲の型とeffectの期待を表示する。project全体を完成させる前でも局所的に調べたい。

holeを含むartifactの実行可否は別profileで管理する。未実装の部分を適当な既定値で実行する仕様にはしない。

## VWORK-02：symbolの安定したidentity

renameやfile移動に追従するsymbol IDを持つ。参照、annotation、過去runの位置をtext offsetだけで結び付けない。

別code版への対応が曖昧なら候補として返す。偶然同じ名前のsymbolへ旧traceを対応付けない。

## VWORK-03：source変更のsemantic diff

API、型、effect、visibility、constant、control flow等の差を分類する。単なるformat変更と振舞いを変える変更を見分けたい。

分析できない部分は不明として表示する。diffが小さいことを意味の保存の証明とはしない。

## VWORK-04：推論結果の説明query

この型、borrow、method候補、effectがどこから来たかをqueryする。利用者はsolverの全logを追わず、必要な根拠へ辿れる。

巨大な型展開は要求した部分だけ表示する。annotationを一つ変えた時の推論差も並べたい。

## VWORK-05：例から探すAPI

入力型、欲しい結果型、許すeffectを指定し、候補APIや短いcompositionを探す。

候補にはcopy、allocation、失敗条件等を添える。名前が似ているだけの検索と、型で確認したcompositionを区別する。

## VWORK-06：localな評価sandbox

選択した純粋式を、明示的な値やdata snapshotへ適用する。debug watchから任意の外部作用を実行せず、短い実験をしたい。

使用したcode版、入力版、budgetをrunへ残す。現在stackのownerを消費する操作は別の能力を必要とする。

## VWORK-07：一括refactorのtransaction

複数fileの変更案をworkspace snapshot上で検査し、成功した差分だけ採用する。部分的なrename失敗を残さない。

利用者が並行編集したfileには前提hashを照合する。外部toolや生成処理を起動する場合は別段階にする。

## VWORK-08：生成sourceの二方向案内

generated methodからschemaやmacroへ、schemaのfieldから生成先へ移動する。複数段階の生成にもorigin graphを使う。

手書きで編集できる領域と、再生成で置き換わる領域を明示する。生成fileの変更が原稿へ自動逆変換できるとはしない。

## VWORK-09：公開契約の影響graph

型やeffectを変えた時、影響する公開API、実装、fixture、保存schemaを表示する。

graphは選択したprofileと依存版に基づく。まだ取得していない外部利用者まで完全に解析したと表示しない。

## VWORK-10：code版付きのinline実行結果

sourceのそばに出す値やplotへ、実行したcode版を付ける。編集後の古い結果はstaleとして見える。

同じ変数名が残っただけで最新結果に見せない。過去版の結果をpinして比較する操作も考える。

## VWORK-11：tool extensionの狭い能力

diagnostic、formatter、query等のextensionへ、必要なsource viewと予算だけを渡す。

sourceを書き換える、外部へ接続する、実行する等は別能力にする。extensionの結果も通常の型・位置・変更前提を検査する。

## VWORK-12：分析stateの版付きcache

parse、型検査、symbol graphを変更依存に応じて再利用する。workspace全体を毎回解析しない体験を目指す。

cache rootと利用者が保持するsource snapshotを別会計にする。旧分析の再利用条件と、無効になった理由を確認できると便利。
