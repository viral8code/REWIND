# データ処理・検索・知識の構造

table/schemaの候補より先の使い方を考える自由構想です。既存JSON/CSV/DB/Unicode等がある部分は、統合体験や追加operationの案として扱います。

## VDATA-01：query planを見ながら変換する

queryがどのfilter、join、scanへ展開されるかを表示する。件数見積り、実測、temporary memoryを見ながら、遅い変換をsourceと対応させて調べたい。

## VDATA-02：遅延・部分実行のdataset

巨大な入力でも必要な列・行だけを評価する。previewで先頭だけを見た後、同じ変換定義から全体を処理し、再利用する中間値を明示してcacheしたい。

## VDATA-03：差分に反応する集計view

データが1行変わったとき、全体を再集計せず影響部分を更新する。VM内datasetのrevertと、外部DBの変更通知を区別しながら、集計の出自を追いたい。

## VDATA-04：Arrow/Parquet等との列共有

他のデータ処理系が作った列指向形式を、その構造を保って読める。NULL、dictionary、timezone、Decimalの意味を落とさず、できる区間はnative配列と共有したい。

## VDATA-05：data lineageの自動表示

集計値やchartの点から、元file・row・query・変換処理へ辿る。結果の正しさを説明するとき、数字を作った経路を後から手作業で再構成しなくて済むと嬉しい。

## VDATA-06：二つの時間を持つrecord

「その値が有効だった期間」と「systemが知った時刻」を別に持つ。後日訂正されたデータでも、当時の判断に使った情報と現在の正しい情報をqueryで分けたい。

## VDATA-07：eventからstateを組み立てる

保存したevent列から現在stateや過去のviewを作れる。event formatの移行、重複、欠落、projection versionを扱い、VMのcheckpointと永続eventの役割を並べて説明したい。

## VDATA-08：全文検索のlibrary

日本語を含むtextから検索indexを作り、token、位置、score、highlightを扱う。normalizationや形態分割の方針を明示し、文書更新からindex変更までを追いたい。

## VDATA-09：vector検索とmetadata filter

embedding等のvectorから近い項目を探し、属性条件を同時に指定する。exact検索とapproximate検索を選び、再現性、recall、index容量を結果と一緒に見たい。

## VDATA-10：graphと知識query

node/edgeに型付き属性を持たせ、経路だけでなく関係のpatternをqueryする。由来や根拠を持つedgeを扱い、推論結果と元の事実を区別して表示したい。

## VDATA-11：地理・空間データ

座標系、地図projection、geometry、空間indexを扱う。距離計算や範囲検索で単位を混同せず、数値libraryと地図表示をつなぎたい。

## VDATA-12：schema evolutionの対話的移行

旧recordを新recordへ移す変換を定義し、実データのsampleでpreviewする。欠けたfield、変更された単位、失敗するrowを先に調べ、移行結果の説明を残したい。

## VDATA-13：datasetのdiffとpatch

行の追加・削除・field変更をstable keyで比較し、差分として保存する。dataset間のbranchやmergeに使い、単なるfile全体の差分より意味が分かる表示にしたい。

## VDATA-14：型付きspreadsheet

cell式、依存関係、単位、table参照を扱える。小さな計算からREWIND関数へ育て、cellごとの履歴や再計算理由を見られると便利。

## VDATA-15：offline編集とdata同期

localの編集stateとremoteの確定stateを分け、競合を利用者が解決できる。revertでremote更新を取消したふりをせず、差分・同期・再取得を別の操作として扱いたい。

## VDATA-16：用途に応じた比較と照合

文字のcollation、大小文字、accent、natural sort、localeを明示して比較する。表示順、検索、keyの同一性を用途ごとに選び、言語全体のString比較を勝手に変えない方式にしたい。
