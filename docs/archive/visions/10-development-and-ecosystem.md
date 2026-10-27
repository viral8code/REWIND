# 開発環境・配布・言語間連携

大型project管理を今の優先実装へ繰り上げるものではありません。将来あったら使いたい開発環境と、REWINDを使う入口の自由構想です。

## VDEV-01：履歴を持つnotebook

説明文、code、表、chart、実行記録を一つの文書へ置く。cell実行のstateと外部作用を表示し、どのcellの結果へ依存するかを辿りたい。

## VDEV-02：再現可能なnotebook export

source、非機密入力hash、library版、期待結果をまとめる。別machineで再計算する場合と、保存済み結果だけを見る場合を選べる文書にしたい。

## VDEV-03：local/browser playground

installなしで短い例を型検査・実行できる。Fileやnetworkを許さない初期環境から始め、full SDKとの差を機能一覧で確認したい。

## VDEV-04：意図で探すAPI catalog

関数名ではなく「重複を除きたい」「日時をDBへ保存したい」から例を探せる。型、effect、所有権、容量の条件でも絞込み、現行SDKのsignatureへ辿りたい。

## VDEV-05：対話的な練習問題

間違えたcodeの型・効果・実行結果を調べ、最小の反例から学ぶ。正解文字列と一致するかだけでなく、別の正しい解法も受け入れる教材を作りたい。

## VDEV-06：意味を保つrefactoring

名前変更、関数抽出、module分割、型の移動を、ownership/effectと一緒に確認する。変換前後のsignatureや実行例を比較して、機械的なtext置換より安全に整理したい。

## VDEV-07：sourceとstateのworkspace

source、型、module、task、checkpointを一つの画面で関連付ける。現在の値からcodeへ、codeから保持中のstateへ移動できる開発環境にしたい。

## VDEV-08：依存の来歴を辿る

libraryのsource、署名、生成手順、native依存をgraphで見る。なぜその版が選ばれたかと、変更したときの影響を確認したい。

## VDEV-09：hermetic buildとbuild再生

compiler、SDK、入力、設定を固定し、同じartifactを再生成する。network取得とcompileを分け、必要な依存をofflineに保存したbuildを使いたい。

## VDEV-10：library registryと権限説明

libraryを用途、signature、対応language、effectで探す。導入だけでcodeを実行せず、必要な外部能力とnative依存を確認してから組み込みたい。

## VDEV-11：applicationの起動設定を束ねる

artifact、runtime設定、権限、GUI起動、resourceを一つのmanifestへまとめる。利用者に長いoption列を毎回入力させず、実際に許す能力は確認できる起動体験にしたい。

## VDEV-12：Python/R/.NET等とのtyped bridge

既存の分析libraryやapplicationからREWIND関数を呼べる。戻り値、array共有、所有権、外部作用を明示し、別runtimeのobjectをそのままcheckpointへ保持する仕様とは分けたい。

## VDEV-13：browser用runtime

VMと純粋libraryをbrowserへ載せ、描画・storage・networkはbrowser側adapterを通す。desktopと同じsourceの共有部分と、環境依存の部分を見分けたい。

## VDEV-14：mobile用runtimeとUI adapter

mobile環境の入力、lifecycle、storageへ接続する。suspend/resumeとREWINDのresumeを混同せず、OSによるprocess終了から復元するdataを明示したい。

## VDEV-15：syntaxを理解するdiff/merge

関数や型を移動した差分を、text位置ではなく構造で比較する。partial fragmentや生成codeを整理したときも、意味のある変更だけをreviewしたい。

## VDEV-16：localで育てる検証loop

保存時に影響する型検査・例・testだけを選び、結果をsourceへ返す。遅い公開CIに頼らず、利用者が明示的に有効化したlocal環境で素早く確認したい。
