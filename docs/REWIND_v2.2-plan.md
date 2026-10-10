# REWIND v2.2実装計画：画面構成・可視化・DB helper

状態：未実装の設計案。基準は公開済みv2.0.0です。APIの綴りは実装開始前に既存SDKと照合して決定します。[全体計画](ROADMAP_v2-next.md)の第2段階です。

## 到達条件

入力・一覧・保存を持つ複数画面のアプリケーションを、配置や行変換を毎回書き直さずに構成できることを目標にします。入力値、画面model、DB上の保存状態、公開済み描画がそれぞれいつ確定したかを説明できるAPIにします。

2.0にはnative window、grid配置、overlay、複数window、検証付きform、viewport付きtable、menu/dialog、IME、clipboard、accessibilityがあります。DBにはSQLite/PostgreSQL、parameter binding、prepared statement、cursor、typed cell変換、明示transaction、非同期cleanupがあります。これらの新規実装を版の成果として数えません。

## 実装前の共通確認

- SDKの既存`std/gui`、`guiForm`、`guiTable`、`guiWindows`、DB moduleを読んでからsignatureを決める。
- 公開関数にはeffect、引数の所有権、単位、上限、エラー後の状態を明記する。
- pureな配置・行変換と、描画のpublish・native入力・SQLのphysical操作を分ける。
- VM revert後も、確定済みSQLやOSの入力履歴が戻らないことを例で示す。
- GUIは既存のLinux/Windows backendの双方を対象にする。fixtureで確認した結果をnative確認済みとして扱わない。

## GUI-01：構成可能なlayout

既存`gui.arrange`は一定列数のgridです。次はrow、column、grid、overlayを入れ子にして、親の領域から子の`Rect`を計算するpureなlayout modelを検討します。

初期仕様は整数pixelのpadding、gap、最小/最大寸法、固定寸法、余りを配分するweightに絞ります。文字の自然寸法やDPI依存測定を純粋計算の内部で問い合わせません。必要な測定値は既存の観測境界を通して入力します。領域不足時の縮小順序、余りpixelの配分順序、clipの扱いを決め、backendによる差を減らします。

配置木には安定したIDを与えます。重複ID、不正寸法、座標overflow、深さ・子数・総要素数の上限を検証してからViewを更新します。検証失敗時に部分的な配置を公開しません。再配置でfocus、selection、scroll、IME compositionをどう扱うかを既存edit stateと照合します。

**作業順序**

1. 既存gridの費用とfocus維持規則を確認する。
2. layout木と純粋な領域計算をREWIND libraryで試作する。
3. 深い木の走査・コピーが費用上限を超える場合だけnative primitiveを検討する。
4. resizeと複数windowに適用し、例とAPI文書を追加する。

**受入条件**

- 狭いwindow、ゼロ領域、最大座標、深さ上限で結果または明確なエラーを返す。
- 同じ木と入力領域から同じ結果を得る。revertでlayout modelを戻せる。
- 部分更新と全更新で同じ配置になり、focusと入力中の文字を不要に失わない。
- layout失敗でnative windowの直前の公開状態を壊さない。

## GUI-02：再利用component

まず入力欄とlabel、検証message、button、一覧領域を組み合わせる例から、componentの契約を抽出します。新しいUI専用構文や実行時reflectionは導入条件にしません。

componentはID、VM内のstate、配置入力、eventからstate/actionを返す処理、Viewへ反映する処理を分けます。component IDと子IDの衝突規則を固定します。actionは呼出側が処理し、renderがSQLやpublishを隠れて実行しない設計にします。native resourceやaffine値をcomponentの共有可能なstateに閉じ込めません。

**受入条件**

- 同じcomponentを同一windowに2個置いて入力・検証・focusが混ざらない。
- 入力stateをcheckpointへ保持してrevertできる。外部保存済みであることの表示は別の観測情報として扱う。
- 再描画でIME compositionや選択範囲を破壊しない。
- 初期版は例から共通化した少数のcomponentに絞り、巨大なwidget体系を先に作らない。

## GUI-03：form/tableの編集統合

`guiForm`は既存の整数・実数・timestamp等の検証とエラー表示を再利用します。追加候補はfield間の関係検証と、検証結果を呼出側の型付きrecordへ変換する明示adapterです。検証失敗で編集中のtextを勝手に正常値へ書き換えません。custom validatorはpureな処理として扱い、callbackが許すeffectを型検査できるかを先に確認します。

`guiTable`のviewportと表示行制限は維持します。編集可能なcell、列幅指定、並べ替え・filter、安定したrow IDを追加候補にします。表示位置をrowのidentityとして扱うAPIを増やさず、選択・編集中cell・DB保存対象をrow IDで対応させます。

全行のsort/filterと描画の費用を区別します。sort/filterが全件走査であることを記載し、各frameで再実行しないよう入力変更時だけ更新します。大規模DBの全件取得をlibrary側で自動実行しません。

**受入条件**

- sort/filter後も同じrowの選択・編集対象を追える。削除済みrowの操作を検出する。
- 編集開始、確定、取消、検証失敗、IME確定前のキー入力を区別できる。
- viewport外のrowを毎回Viewへ展開しない。
- 整数overflow、空値、必須、field間矛盾、無効timestampを診断できる。
- row ID、column ID、初期値の不正を公開前に検証する。

## VIS-01：小規模なplot library

初期候補はlinear axisの折れ線・散布図とlabelです。numeric配列の結果を表示する用途に絞り、統計やFFTを再実装しません。実装に入る前に既存Viewの描画primitiveとbackend費用を調べます。primitiveが不足する場合は、native rendererの追加範囲を別の小さな設計として確定します。

大配列を`List`へ全コピーして各pixelをwidget化する方式を避けます。入力にはnative配列のread-only viewを利用できるかを調べ、表示領域に応じた点数制限・集約を用意します。集約を使うときは元データと描画データを区別し、極値を消す単純間引きを既定にしません。

NaN/Infinity、空配列、同一値だけの軸、桁違いの値、座標overflow、clip、点数上限を規定します。軸範囲の自動計算は明示的な費用として記載します。SVG等のexportは生成データとファイル公開を分けます。interactive zoomや多数のchart種別は初期完了条件に含めません。

**受入条件**

- 大入力でも描画element数が設定上限を超えない。
- 配列を変更・revertしたときに共有viewの寿命と描画結果が正しい。
- 出力や診断がSecretの値を露出しない。
- native描画とexportの座標変換が一致する。
- 必要なprimitiveの費用・backend互換を確認できなければ、VIS-01は保留理由を記録しGUI/DBの公開を妨げない。

## DB-01：record mapping

既存のtyped cell変換に、行から利用者のrecordを作る明示mapperを組み合わせます。自動ORMやreflectionを前提にしません。まず利用者の関数でmappingを書く例を作り、重複する列検証だけをlibraryへ抽出します。

列名mappingでは重複名、欠落、NULL、型不一致、整数範囲、日時の単位・timezone、浮動小数点と正確なdecimalの違いを扱います。decimalには既存の`Decimal`と`std.dbDecimal`を再利用し、暗黙にFloatへ変換しません。SQLite BLOBとPostgreSQL NUMERICの既存表現を尊重し、新しい対応型が必要な場合だけ別案へ分けます。

cursorを全件materializeする便利関数を既定にしません。1行ずつ、または明示上限付きbatchをmapperへ渡します。失敗にはrow位置・列名・期待型・error codeを持たせ、SQL parameterや実値を既定で載せません。

**受入条件**

- SQLite/PostgreSQL双方でNULL・重複alias・大整数・日時・変換失敗を確認する。
- mapping途中の失敗でcursor/connectionの所有権を失わず、cleanupが完了する。
- 結果のrecordはVM内の値としてcheckpointに保持できる。physical cursorを再生可能値と誤認しない。
- 大件数でも設定したbatch容量内で処理できる。

## DB-02：transaction helper

既存のbegin/commit/rollbackをまとめるhelperを検討します。`defer`でrollbackの非同期完了まで処理できると仮定しません。externalへのsubmitとVM側のawait、affine connectionのborrow・task移動制約を、最小の実行例で先に証明します。

初期候補は明示transaction handleと状態遷移です。callbackで全操作を隠す形は、現在のeffect/所有権契約で安全に成立する場合に限ります。

| 状態 | 許す操作・責任 |
|---|---|
| active | query/executeとcommit/rollback要求 |
| committing / rolling back | 完了待ち。同じhandleへの二重要求を拒否 |
| committed / rolled back | transaction終了。再度commitしない |
| failed / outcome unknown | backendが確実に報告した状態と結果不明を区別し、接続の再利用可否を返す |
| closed | 所有資源を解放済み。新しい操作を拒否 |

業務処理の失敗とrollback/cleanupの失敗を両方保存します。元の失敗をcleanupのエラーで置き換えません。dropによる自動commitは導入しません。既存driverのcancel・接続切断・cleanup規則を尊重し、結果不明のSQLを自動retryしません。

**受入条件**

- 成功、業務失敗、rollback失敗、cancel、接続断、commit結果不明を区別できる。
- transaction終了前にconnectionを別taskへ不正に渡せない。
- VM revertを実行しても終了済みphysical transactionが復活しない。
- 二重終了要求と未終了handleのcleanupを診断する。
- compiler変更が必要なら先に最小の所有権/effect設計を追加し、libraryだけで安全に実装できると装わない。

## DB-03：限定したmigration

明示的なmigration ID、checksum、backend別SQL command列、適用済みversion tableを使う小さなrunnerを候補とします。自動schema差分生成、SQL文字列をsemicolonで分割するparser、自動down、無断force適用は対象外です。

DDLのtransaction性はSQLite/PostgreSQLで異なるため、各commandの成功・失敗と再開条件をbackendごとに決めます。同時適用を排除するlock、version記録とSQLのatomicity、checksum不一致を検証します。DB側で同時適用を排除できる実装を確認できるまで便利な自動runnerとして公開しません。

適用開始は明示的なphysical操作です。単にmoduleをimportしただけでschemaを変更しません。VM checkpointへ適用済みという値を保持しても、DBの真の状態確認を省略しません。

**受入条件**

- 空DB、既適用、checksum相違、途中失敗、二重runner、接続切断を確認する。
- backendごとの再実行可能条件を説明する。
- 診断が接続credentialやSQL内の機密値を露出しない。
- 破壊的SQLを自動生成せず、利用者が与えたcommandと対象backendを適用前に確認できる。

## APP-01：統合例と文書

入力・一覧・保存を持つ小さな例をSQLiteで提供します。PostgreSQL差分は明示設定の別例にします。DB接続先や実データをrepositoryへ保存しません。

例では、編集取消のVM revert、入力検証、明示transaction、保存成功後の画面更新、保存結果不明時の表示、window終了とcleanupを扱います。network、DB、native入力がVM revertで巻き戻るような説明やUIを作りません。

API文書と入門書は実装済みsignatureから更新し、計画上の候補APIを現行referenceへ混ぜません。

## 順序と公開判断

1. GUI-01とDB-01の既存基盤調査・最小例。
2. GUI-02、GUI-03、DB-02を独立した小さな実装単位で進める。
3. DB-03の安全条件とVIS-01の費用を検証し、実装か保留を決定する。
4. APP-01で組合せを確認し、API文書・移行手順・ローカル検証をまとめる。

GUI-02/03はGUI-01のID・配置契約に従います。DB-03はDB-02の終了状態とcleanup規則を利用します。VIS-01は他の完了を待たせる必須項目にはしません。

新しいDB backend、connection pool、巨大ORM、GPU、多数のchart種別は、具体的な不足と費用が確認されてから次案にします。poolを追加するときも、physical connectionの寿命・credential更新・cancel後の再利用・上限会計を先に設計します。

今回実行するのは計画文書のcommit/pushだけです。公開CI/CD、release操作、mainへの統合は行いません。
