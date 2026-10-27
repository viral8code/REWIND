# 金額・暦・文書・日常データのlibrary

一般的な型やcodecの上に、意味を持つデータとoperationを置く自由構想です。計算policyと外部へ確定する操作は分けて扱います。

## VDOMAIN-01：通貨付きMoney

通貨、桁、丸め、換算時点を持つ金額型を使う。Decimalがあるだけでは防げない通貨混同を避け、計算途中と表示時の丸めを分けたい。

## VDOMAIN-02：複式の記帳model

仕訳、勘定、期間、訂正を型付きrecordとして扱う。貸借の整合をpureに検査し、保存済みの記録と編集中の記録を別のstateとして見たい。

## VDOMAIN-03：利息・返済・償却の計算

day count、複利、端数、支払scheduleをpolicyとして指定する。各期間の根拠を表で出し、条件を変えた場合の差を比較したい。

## VDOMAIN-04：税・端数・配賦policy

明細/合計のどこで丸めるか、配賦の残額をどう扱うかを宣言する。policyのversionを結果へ添え、同じ入力でも規則変更で結果が変わった理由を説明したい。

## VDOMAIN-05：休日・稼働日のcalendar

地域の休日、独自の休業日、営業時間を組み合わせる。timezoneと日付の違いを保ち、次の稼働日や期間内の時間を計算したい。

## VDOMAIN-06：繰返し日程の型

毎月末、隔週、特定曜日、除外日をmodelとして表す。DSTや存在しない日付のpolicyを明示し、個々の発生日時へ展開した理由を確認したい。

## VDOMAIN-07：意味を持つ識別番号

check digit、format、prefix、発番policyを持つIDを扱う。表示用文字列と内部identityを分け、仮番号と確定番号の違いも型で示したい。

## VDOMAIN-08：住所・連絡先・名寄せ

国やlocaleごとの構造を保った住所・電話等を扱う。normalizationの根拠と曖昧さを残し、近いという判定を同一人物という確定と混同しない処理にしたい。

## VDOMAIN-09：barcodeとQR code

型付きpayloadからcodeを生成・読取りする。容量、文字集合、error correction、表示寸法を扱い、文書やGUIへ同じmodelを渡したい。

## VDOMAIN-10：構造化した文書template

入力recordからparagraph、table、画像、条件付きsectionを作る。単なるString置換ではなく、未指定fieldや単位の違いを検査してreportへ出したい。

## VDOMAIN-11：文書の署名と検証

文書hash、署名、証明書、検証時点を扱う。内容が一致すること、署名者の根拠、時点の意味を分けて表示し、private keyは公開データと別に管理したい。

## VDOMAIN-12：lot・数量・換算

品目、lot、数量、単位、期限をtyped modelで扱う。換算policyと合計の根拠を保持し、単位が同じでも異なる品目を混ぜない計算にしたい。

## VDOMAIN-13：価格・条件・優先順位のrule

期間、数量、属性から適用ruleを選ぶ。競合や重複を検出し、どのruleが価格を決めたかを利用者へ説明したい。

## VDOMAIN-14：XML/EDI等の構造化交換

schemaに従う階層データと交換messageを扱う。namespace、encoding、必須section、精度を保ち、内部recordとのmappingを明示したい。

## VDOMAIN-15：version付き判定policy

判定条件をdataとして保存し、特定時点のpolicyで結果を再計算する。現在の規則と過去の判断に使った規則を並べ、修正の影響を調べたい。

## VDOMAIN-16：関係を保つsynthetic dataset

record間の参照、時系列、数量の整合を保った架空データを作る。実データを複製せず、UI・DB・集計の例や負荷試験へ使いたい。
