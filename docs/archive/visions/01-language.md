# 言語表現とメタプログラミング

現行仕様とは分けた自由構想です。既存のgeneric・trait・pattern等を土台に、「どう書けたら嬉しいか」を考えます。

## VLANG-01：式として組み立てるデータ変換

filter、group、join、sortを、一つの読みやすいquery式として書ける。変換の各段階に名前を付け、途中の型・件数・費用をエディタで確認できると嬉しい。

## VLANG-02：埋込みDSLの型検査

SQL、regex、template、shader、単位付き数式などを、単なるStringではなく専用literalとして書ける。型付きparameterと補完を使い、埋込み言語のエラーも元sourceの位置へ戻したい。

## VLANG-03：型付き文字列補間

数値の桁、Decimalの丸め、日時zone、escapingを補間の仕様として明示できる。SQLやHTMLでは、文字列連結ではなくparameter/encodingを保った専用補間になると便利。

## VLANG-04：次元解析を伴う単位計算

長さ、時間、角度、電圧、通貨等を区別し、掛算や割算から結果の次元を推論する。金額の換算率と物理定数を同じ種類の暗黙変換にせず、式から単位の根拠を辿りたい。

## VLANG-05：範囲・性質を持つ型

正数、非空列、sorted列、正規化済み文字列などの性質を型へ付けられる。検証済み値を渡すと内部の同じ検査を繰り返さずに済み、外部入力には検証の入口を残せると嬉しい。

## VLANG-06：design by contract

関数の事前条件・事後条件・不変条件を、実行可能な説明として宣言できる。実行時検査、静的証明、test生成に同じ条件を使い、失敗時に「どの条件が壊れたか」を示したい。

## VLANG-07：hygienic macroと展開閲覧

小さな繰返し構文をmacroにでき、名前の衝突や予期しないcaptureを避けられる。エディタで展開前・展開後・型検査結果を並べ、生成された効果まで確認できると安心して使える。

## VLANG-08：型付きcompile-time計算

小さなlookup table、schema、状態遷移表をcompile時に作れる。計算が失敗した位置を元の宣言へ戻し、結果を通常の値や型として使いたい。

## VLANG-09：利用者が定義するeffect

DBやoutput等の既存effectに加え、application固有の「承認」「audit」「model更新」等を宣言できる。呼出しgraphから誰がどの操作を要求するかを追い、test用handlerへ明示的に差し替えたい。

## VLANG-10：algebraic effectとhandler

入力取得、選択、失敗の扱いをhandlerへ切り出し、同じ計算をCLI・GUI・testで使える。continuationとREWINDのresumeがどう違うかを表現でき、外部作用まで自動的に取り消す説明にはしない。

## VLANG-11：型付きholeからコードを育てる

未完成の式をholeとして置き、期待型・利用可能な変数・必要effectを表示できる。全fileが完成する前に周囲を調べたり、小さい例だけを実行できる開発体験にしたい。

## VLANG-12：patternから得る型の絞込み

分岐後に「このvariant」「この範囲」「このfieldが存在する」という情報を型検査へ引き継ぐ。繰り返すcastやNULL検査を減らし、条件を変えたときには依存する処理を指摘してほしい。

## VLANG-13：型を失わないvariadicとparameter pack

tupleや複数引数をまとめて扱うhelperを書ける。format、zip、callbackの組合せでも各要素型を保持し、可変引数をすべて汎用objectへ落とす必要を減らしたい。

## VLANG-14：open recursionとstrategy差替え

処理の一部だけを差し替えて再利用できる。class継承に限らず、traitや明示strategyの合成で、serializer・solver・validationの小さな変更を表現したい。

## VLANG-15：APIの意図を表すattribute

deprecatedだけでなく、費用、単位、再送条件、thread affinity、checkpointへの保持可否を宣言へ付ける。doc、checker、editorが同じ情報を読み、説明と実装がずれにくくしたい。

## VLANG-16：一つの型を複数の視点で表示する

内部では同じデータを、編集用、読取り用、serialization用、集計用のviewとして扱える。viewごとの公開fieldと操作を型で表し、巨大recordを用途ごとに手作業でコピーする負担を減らしたい。
