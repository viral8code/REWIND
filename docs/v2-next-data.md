# 次版構想：表データ・数値処理・一般ライブラリ

状態：採用候補、未実装。版番号は未割当です。[全体計画](ROADMAP_v2-next.md)と[性能計画](v2-next-performance.md)を参照してください。

JSON/CSVと増分parser、BigInt/Decimal、IANA timezone、Unicode/regex、dense/sparse配列、線形代数、統計・分布、FFT、自動微分、optimizer、model保存、graph/flow/matching等は既存です。以下は新機能の数を増やすためではなく、既存機能間の変換と反復記述を減らす候補です。

## DATA-01：型付きschemaと変換

JSON/CSV/DB/formに散在するfield検証を共通schemaへ寄せる案です。初期は利用者が明示したschemaとmapperに限定し、reflectionや自動deriveを必須にしません。

必須/任意/NULL/default、整数範囲、正確なDecimal、日時単位、unknown field、重複名、nested pathを規定します。unknown fieldを黙って削除するかどうかは明示policyにします。schema versionとデータversionを分けます。

**受入：** 同じrecordへのJSON/CSV/DB mappingを例示する。NULLと空文字を混同しない。巨大なerror一覧はboundedにする。Secret fieldの実値を診断へ出さない。

## DATA-02：列指向の小さなtable

型付き列、列名、行数、NULL bitmapを持つtable候補です。`List<Map<String,Json>>`だけで全cellを保持する方式に限定せず、numeric列には既存native配列を再利用します。

初期操作は列選択、行filter、列追加、明示型変換、headとbounded batchです。可変tableと共有snapshotの所有権を固定し、revert時の共有pageを維持します。text/Decimal/日時列の表現と会計を先に測ります。

**受入：** column長不一致、重複名、NULLを検出する。不要なper-cell object化を避ける。小規模入力の開始費用と大入力のmemoryを記録する。

## DATA-03：group/aggregate/join

DATA-02の後続候補です。group keyの型、NULL同士の対応、行順序、安定性、aggregateの空入力、overflowを規定します。joinの多対多による出力増大には明示上限を設けます。

最初はhash/order方式を決める前に、既存MapのOrd契約と比較費用を調べます。sum/mean/varianceには既存checked算術とonline momentsを再利用します。暗黙のFloat変換でDecimalの精度を落としません。

**受入：** 小入力の単純な参照実装と照合する。重複key、NULL、偏ったkey、空入力、出力上限を確認する。algorithmとscratch費用をAPIへ記載する。

## DATA-04：streamから表へのpipeline

CSV/JSON stream、DB cursor、HTTP downloadを、bounded batchでschema検証・変換へ接続する案です。物理入力の取得とpureな変換を分けます。

sourceは必ずEOF/failed/cancelledを区別し、backpressure、batch件数/byte上限、途中のerrorを規定します。入力再取得が必要な操作と、VM内snapshotだけでrevertできる操作を明示します。全件collectは利用者が容量を指定した場合だけ候補にします。

**受入：** UTF-8/chunk境界、途中変換失敗、consumer取消で資源を解放する。遅いconsumerで無制限queueを作らない。再開が同じ入力を保証できない場合はそう報告する。

## NUM-01：数値kernel間の変換削減

reshape/view、native入力、協調kernel、ADは既存です。pipelineを実例で測り、型付き配列のまま次operationへ渡せない箇所と、重複したshape検査・List変換を洗い出します。

候補はviewを受けるAPIの統一と、少数の明示elementwise operationの組合せです。任意REWIND callbackをnative workerへ移しません。fusionを導入する場合は、演算順序、丸め、work会計、取消chunk、temporary寿命を先に決めます。

**受入：** 性能計画のbaselineと誤差基準で比較する。no-copyという表現は実際に共有している区間だけに使う。profileの帰属を消さない。

## NUM-02：不足solverの選定

現行QR、least-squares、対称eigen等の範囲を確認し、rank不足・不良条件、sparse反復解法、SVD等が必要な具体例を作ります。名前だけのAPIや精度不明の独自実装を先に増やしません。

採用するsolverには定義域、convergence、tolerance単位、最大iteration、preconditioner、NaN/Infinity、残差、特異な入力を規定します。native依存libraryを使う場合もdeterminism、配布license、thread数、memory/work計上、cancel粒度を評価します。

**受入：** 信頼できる参照解と残差を照合する。収束しない結果を成功にしない。大問題で他taskの応答性を保つ。既存solverで十分なら例の整備へ縮小する。

## NUM-03：再現可能な学習・評価のhelper

既存training/optimizer/model保存に、batch分割、shuffle、train/validation分離、early stopping、評価の集約を組み合わせる案です。新しいmodel体系を先に作りません。

seed、データ順、model format、optimizer state、checkpoint保持数を記録します。best modelのVM snapshotと、公開済みmodel fileを区別します。各epochを無制限にcheckpointへ保持する既定にはしません。評価指標は分類/回帰等の実例から少数を選びます。

**受入：** 同じ設定で再現できる範囲を説明する。training/validationの混入を防ぐ。取消・保存失敗からの再開条件を示す。iteration後にgraphが不要に残らない。

## LIB-01：一般libraryの使い勝手の監査

追加algorithmの前に、既存`sort/search/graph/flow/matching/geometry/sequence`等の容量、重み型、negative値、overflow、generic制約を実例で確認します。

候補はchecked adapter、typed result、よくある組合せの例です。木のquery、offline query、数論等の追加も、現行APIにない操作だけを別作業へ分けます。単に一覧のmodule数を増やしません。

**受入：** 境界条件と費用を説明できる。小入力の独立参照解と比較できる。VMの予算を回避する無制限native処理を導入しない。

## LIB-02：binary codec・圧縮・画像

候補は明示endianの整数codec、bounded圧縮/展開、画像の読込み・保存です。用途例がある形式を一つずつ選びます。native依存が必要なら[配布構想](v2-next-ecosystem.md)の拡張契約を先に確認します。

入力sizeだけでなく展開後size、画像dimension、総pixel、metadata、chunk数に上限を持たせます。pureなdecode/encode結果とFileへの公開を分けます。codec状態をcheckpointへ保持できるか、physicalなencoder handleなのかを型で区別します。

**受入：** 不正・途中・巨大宣言入力を拒否する。出力を外部に公開する前に生成失敗を検出できる。memory/workを計上し、形式とlibrary licenseを記録する。

## 採否の順序

DATA-01 → DATA-04 → DATA-02 → DATA-03。NUM-01は性能計画に従って先行可能です。NUM-02/03とLIB-01/02は実例で不足を確認した項目から選びます。

DATA-02/03は大きな新libraryになるため、小さなschema/stream接続より先に着手しません。候補全体を一つのminor版へ詰め込まず、所有権・精度・費用の受入を満たしたまとまりで版を判断します。
