# REWIND v2.2実装計画：画面構成・可視化・DB helper

状態：設計案。GUI-01にweighted layout、DB-01に列名index、bounded record mapper、single-row/schema batch adapterのsource・利用例を一部追加しました。到達条件と受入は未完了です。v2.2の到達条件と残りの受入は未完了です。基準は公開済みv2.0.0です。[全体計画](ROADMAP_v2-next.md)の第2段階です。

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

既存`gui.arrange`は一定列数のgridです。**部分実装（2026-10-12）：** flatな`gui.splitRow` / `splitColumn`は最大256個のweightとgapを受け取り、4096 pixel以内の領域を子Rectへ分割します。このflat APIには個別寸法制約はありません。layout treeでは`gui.sized(child,weight,min,max)`が子のmain-axis寸法を0..4096 pixelで制約し、`min == max`は固定寸法です。`gui.crossSized(child,min,max)`はrowの高さ・columnの幅を制約します。`gui.weighted`の既定範囲は0..4096です。`gui.row` / `column`は最大32階層のtreeとして合成し、`applyLayout`でatomicに適用します。`arrangeRow` / `arrangeColumn`は全ID、重複、View内bounds、非ゼロ寸法を事前確認してからgeometryだけを更新し、失敗時にViewを変更しません。剰余pixelはrowなら左から、columnなら上から配り、既存widget IDに結び付いたfocus/edit stateを維持します。[例](../examples/gui-layout.rw)。

nested row/column layout treeとatomicなView適用をsourceへ追加しました。node tableは最大1024 node・深さ32、1 containerは最大256 childです。main-axisのweight/min/maxはlayout treeで使えます。gridはnested row/columnで構成できます。`overlayLayout`は2..256のchild treeを同じRectに配置し、`padded(layout,left,top,right,bottom)`は四辺のinteger insetでinner boundsを作ります。node数、depth、残りのinner寸法は構築時・適用時に検証し、不足時にViewを部分更新しません。自然寸法測定、resize時の自動再配置は未実装です。

実装済みの純粋計算は整数pixelのgap、main-axis min/max/fixed寸法、cross-axis min/max制約、余りを配分するweight、overlay childへの同一bounds割当て、padding insetに絞ります。cross-axisの最大値を超えた余白はrowなら下側、columnなら右側へ残し、子は上端または左端に揃えます。文字の自然寸法やDPI依存測定を純粋計算の内部で問い合わせません。必要な測定値は既存の観測境界を通して入力します。領域不足時はminimum合計とgapを満たせなければ失敗し、余りpixelはrowなら左から、columnなら上から配ります。clipの扱いとresize時の再配置は未決です。

配置木には安定したIDを与えます。重複ID、不正寸法、座標overflow、深さ・子数・総要素数の上限を検証してからViewを更新します。検証失敗時に部分的な配置を公開しません。再配置でfocus、selection、scroll、IME compositionをどう扱うかを既存edit stateと照合します。

**作業順序**

1. 既存gridの費用とfocus維持規則を確認する。
2. layout木と純粋な領域計算をREWIND libraryで試作する。
3. 深い木の走査・コピーが費用上限を超える場合だけnative primitiveを検討する。
4. resizeと複数windowに適用し、例とAPI文書を追加する。

**受入条件**

- 狭いwindow、ゼロ領域、最大座標、深さ上限で結果または明確なエラーを返す。
- 同じ木と入力領域から同じ結果を得る。revertでlayout modelを戻せる。
- overlayの各layerが同じrectを受け取り、重複IDや不足boundsでViewを部分更新しない。
- padding適用後のinner boundsが全childで1px以上となり、不足時はViewを部分更新しない。
- cross-axis minimum未満は失敗し、maximum超過はleading edgeへ揃えて縮め、Viewを部分更新しない。
- 部分更新と全更新で同じ配置になり、focusと入力中の文字を不要に失わない。
- layout失敗でnative windowの直前の公開状態を壊さない。

## GUI-02：再利用component

まず入力欄とlabel、検証message、button、一覧領域を組み合わせる例から、componentの契約を抽出します。新しいUI専用構文や実行時reflectionは導入条件にしません。

**部分実装（2026-10-12）：** `std.gui`に`ComponentScope`、`componentId`、`componentPrefix`を追加しました。UTF-8 byte長を含むprefixでscope/local IDのpairを一意に符号化し、同じ画面で同じ部品形状を複数回組み立てるID衝突を避けます。scopeは通常のVM値で、状態やnative resourceを保持しません。[例](../examples/gui-component-scope.rw)。これはID基盤であり、共通render/reducerやform/table componentの完了を意味しません。
**部分実装（2026-10-14）：** `std.guiField`を追加しました。ラベル付き単一行入力の状態、描画、編集、validation errorをVM値として保持し、renderはI/Oを行わず、scopeごとにinput / validationのIDを分けます。

componentはID、VM内のstate、配置入力、eventからstate/actionを返す処理、Viewへ反映する処理を分けます。component IDと子IDの衝突規則を固定します。actionは呼出側が処理し、renderがSQLやpublishを隠れて実行しない設計にします。native resourceやaffine値をcomponentの共有可能なstateに閉じ込めません。

**受入条件**

- 同じcomponentを同一windowに2個置いて入力・検証・focusが混ざらない。
- 入力stateをcheckpointへ保持してrevertできる。外部保存済みであることの表示は別の観測情報として扱う。
- 再描画でIME compositionや選択範囲を破壊しない。
- 初期版は例から共通化した少数のcomponentに絞り、巨大なwidget体系を先に作らない。

## GUI-03：form/tableの編集統合

`guiForm`は既存の整数・実数・timestamp等の検証とエラー表示を再利用します。関係検証と、検証済みのtyped JSONから呼出側recordを作るpureな明示adapterを提供します。検証失敗で編集中のtextを勝手に正常値へ書き換えません。custom validatorはpureな処理として扱い、callbackが許すeffectを型検査できるかを先に確認します。

**部分実装（2026-10-12）：** `guiForm.createWithRules`へ最大128個の宣言的なrelationを追加しました。`Equal`は互換scalar fieldのtyped value、`IntLessEqual`はInt field、`RequiredWhenChecked`はcheckbox依存必須を検証します。rule参照とkindをschema構築時に検査し、`validate`と`toJson`で適用します。custom validatorは未実装です。入力textはrelation failureでも保持されます。[例](../examples/gui-form-relations.rw)。

**部分実装（2026-10-12）：** `guiForm.toValue`は`toJson`の全field・relation検証に成功した後にだけ、利用者が渡したpure callbackを一度呼びます。typed JSONからapplication recordを作るmapping errorとform validation errorを`SubmitError`の別variantで返します。libraryはreflectionや暗黙変換を行いません。[例](../examples/gui-form-typed.rw)。

**部分実装（2026-10-12）：** `guiForm.toSchemaValue`はtoJson、利用者が渡す`std.schema`、pureなtyped mapperの順に検証し、Form / Schema / Conversion errorを区別します。全検証が成功するまでmapperを呼ばず、form modelを変更しません。[例](../examples/gui-form-schema-validation.rw)。

**部分実装（2026-10-12）：** `guiTable.createWithIds`はcaller-suppliedなunique row IDを保持し、row action・選択・編集をbase rowへ結びます。sort後も同じTable model内でidentityを保ちます。filter後は`rowIndex`が表示位置を返し、非表示rowには`None`を返します。従来の`create`が作る`row:<index>`はmodel内のbase position IDであり、Tableを再構築した後のidentityではありません。

**部分実装（2026-10-12）：** row IDとcolumn indexをキーにしたboundedなstaged-edit overlayを追加しました。editCell / editCellIdで値を保存し、cellと描画はoverlayを優先し、changesは最初の編集順のfrozen snapshotを返します。元のFrozen cell matrixは変更・複製しません。編集は最大4096 cell、値の合計4 MiB、1 cell 4096 byteまでです。元値への復元はoverlayから除去し、最大4096件を再indexします。`sortByText`はstaged valueで安定sortし、ID・selection・差分overlayを維持します。`filterByText`は一列のcase-sensitive部分文字列filterを行い、query一致列の編集またはclearEdits後に表示対象を再計算します。filter中もeditCellIdはstable IDで非表示rowを編集できます。sort/filterは各々8192 source rowsまでです。native text inputとの接続、typed table adapter、削除・取消を含む完全な編集event lifecycleは未実装です。[編集例](../examples/gui-table-edits.rw)、[sort例](../examples/gui-table-sort.rw)、[filter例](../examples/gui-table-filter.rw)。

`guiTable`のviewportと表示行制限は維持します。列幅は実装済みで、正のrelative weightを指定し、render時にinner width全体へ割り当てます。`guiTable.toValue`はstable row IDで1行をfrozen column snapshotへ変換し、純粋mapperでtyped recordを構築します。staged edit値とfilterで非表示の行も扱い、変換失敗にはrow IDを含めます。編集draftの開始・更新・確定・取消をmodel側へ追加しました。残るGUI-03はGUI text inputからのevent連携、`std.schema`を使うapplication validationとの接続、削除時の編集取消を含む完全なnative editing lifecycleです。formのtyped record adapter、tableのstable row ID・差分overlay・stable text sort・bounded text filterは実装済みです。表示位置をrowのidentityとして扱わず、選択・編集中cell・DB保存対象をrow IDで対応させます。

sortByTextは全source row、filterByTextはfilter列を全走査します。filterは呼出し時にだけ適用し、frameごとには再実行しません。active filter columnの編集とclearEdits後はmembershipを再計算します。描画はviewport内だけです。大規模DBの全件取得をlibrary側で自動実行しません。

**受入条件**

- sort後もrow IDで同じselection/edit targetを追える。filterで非表示になったselectionは解除され、row IDから非表示状態を識別できる。
- model draftの開始・更新・確定・取消をstable row IDで区別し、staging失敗時はdraftを保持する。
- 編集開始、確定、取消、検証失敗、IME確定前のキー入力をnative eventから区別できる。
- viewport外のrowを毎回Viewへ展開しない。
- 指定列weightの合計割り当てがinner widthと一致し、隣接列にgapやoverlapがない。
- typed row mapperがcolumn orderとstaged valueを受け、失敗時にはstable row IDを返してtableを変更しない。
- 整数overflow、空値、必須、field間矛盾、無効timestampを診断できる。
- row ID、column ID、初期値の不正を公開前に検証する。

**部分実装（2026-10-14）：** `guiTable`にセル編集の流れを追加。draftのセルを`prefix:edit`のtextboxで描き、`dispatch`がtext/keyを`setDraft`へ反映、Enterで確定、Escapeで取消、F2で開始。Action `EditStarted/EditCommitted/EditCancelled/EditRejected`は行IDと列を持つ。確定失敗は`EditRejected`でdraftを保持。複数行は拒否（`GuiSingleLine`）。IME preeditはnative層が処理。行削除は未対応（tableはimmutable）。`EditRejected`の経路は未テスト。

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

**実装済み（2026-10-14）：** 線分primitive `gui.line`（Linux X11 / Windows GDI、foreground色、bboxと向き）と`std.plot`（`series`/`axisOf`/`chart`/`project`/`polyline`/`scatter`/`svg`/`render`）を追加。`polyline`はClip（Liang–Barsky）と画素列ごとの極値保持（先頭・最小・最大・末尾）で線分数を1..2000に制限し、`svg`と`render`は同じ線分を使うため座標が一致する。NaN/Infinity・空・長さ不一致・サイズ不正・overflowは`PlotError`で拒否。Xvfbのスクリーンショットで折れ線の向きと1px線を確認。**制約：** `Frozen<List>`の`get`が呼び出しごとに全長を計上するため、既定の`--native-work`では数百点でも予算を超える。テストは`--steps 100000000 --native-work 100000000`で実行する（10万点は別実行で通過、約7.6分）。`axisOf`を値渡しにして`thaw`で線形化する案は、署名変更になるため未実施。

## DB-01：record mapping

**部分実装（2026-10-12）：** `std.db.indexColumns`は最大256個のcursor column名を一度検査し、重複を拒否してimmutable `ColumnMap`を返す。`columnNumber`は同じ結果を使って名前からindexを引く。新しいpureな`mapBatch`は既存の`DbBatch`を借用し、最大64行を利用者mapperでrecordへ変換する。最初の失敗をbatch内row index付きで返し、入力batchとcursorを変更しない。SQLiteの`mapScore`とPostgreSQLの`mapStatus`がtyped accessorを明示し、共通helperへ渡す。自動ORMやreflectionは追加しない。\n\n**部分実装（2026-10-12）：** `std.db.validateRow`は最大256列の一行を`std.schema.Schema`で検証します。`validateBatch`は列名の重複を一度検査してから最大64行を入力順に検証し、`MappedBatch<Json>`とbatch内row error indexを返します。Null/Bool/Int/Float/TextをJSONへ写し、Bytes/Privateは拒否します。SQLite exampleはshared schema batch検証後に明示recordへ変換します。SQLite Decimal BLOB adapter、実DB受入は未実施です。

**確認済み（2026-10-14、SQLite）：** [tests/db_row_mapping.rs](../tests/db_row_mapping.rs)が実際のSQLite memory DBで、`indexColumns`、`mapBatch`、`columnNumber`を実行します。NULL cell、`Int`の最大値9223372036854775807、mapper失敗時のbatch内row index（`RowMapError::Row`）とerror code（`DbType`）、重複列名の拒否、存在しない列名の拒否、失敗後も借用元の`DbBatch`が使えることを確認しました。`std.db`の標準ライブラリtest（`validateRow`、`validateBatch`、64行上限）も通過しています。

**未完了：** SQLiteでは日時（timezone offsetとnanosecondsの往復、不正値のRowMapErrorで行番号と列を報告）、Decimal BLOBの往復（9007199254740993.25、浮動小数点列は拒否）、mapper失敗後のcloseCursorと同一接続での再利用を確認済みです（[tests/db_row_mapping.rs](../tests/db_row_mapping.rs)）。PostgreSQLでの同じ確認はCI専用のままで、接続先が必要です。

既存のtyped cell変換に、行から利用者のrecordを作る明示mapperを組み合わせます。自動ORMやreflectionを前提にしません。まず利用者の関数でmappingを書く例を作り、重複する列検証だけをlibraryへ抽出します。

列名mappingでは重複名、欠落、NULL、型不一致、整数範囲、日時の単位・timezone、浮動小数点と正確なdecimalの違いを扱います。decimalには既存の`Decimal`と`std.dbDecimal`を再利用し、暗黙にFloatへ変換しません。SQLite BLOBとPostgreSQL NUMERICの既存表現を尊重し、新しい対応型が必要な場合だけ別案へ分けます。

cursorを全件materializeする便利関数を既定にしません。1行ずつ、または明示上限付きbatchをmapperへ渡します。失敗にはrow位置・列名・期待型・error codeを持たせ、SQL parameterや実値を既定で載せません。

**受入条件**

- SQLite/PostgreSQL双方でNULL・重複alias・大整数・日時・変換失敗を確認する。
- mapping途中の失敗でcursor/connectionの所有権を失わず、cleanupが完了する。
- 結果のrecordはVM内の値としてcheckpointに保持できる。physical cursorを再生可能値と誤認しない。
- 大件数でも設定したbatch容量内で処理できる。

## DB-02：transaction helper

**実装済み（2026-10-14）：** `std.dbTransaction`は、物理的なBEGIN / COMMIT / ROLLBACKを`std.db`のtaskに任せたまま、要求と応答の状態だけを追跡します。`Transaction`は通常のVM値で、`requestCommit` / `requestRollback`で`Committing` / `RollingBack`へ移り、`completeCommit` / `completeRollback`にtaskの結果を渡して`Committed` / `RolledBack`へ進みます。完了待ちの二重要求、終了後の再要求、要求の無い完了通知は`TxError`で拒否し、状態を変えません。失敗は`DbError.phase`で分けます：`NotSent`は元の状態へ戻し、`Unknown`は`Unknown`（再試行もrollbackも許可しない）、それ以外は`Failed`（rollbackは可）。`close`は未完了のhandleを`NotFinished`として報告します。`finished`は業務失敗とcleanup失敗を別々に保持します。dropによる自動commitはありません。`connectionReusable`は終了が確定した状態だけ真です。

確認：状態遷移の標準ライブラリtest3件と、実際のSQLiteでcommit・rollback・二重要求・古い完了通知を確認する[tests/db_row_mapping.rs](../tests/db_row_mapping.rs)。**未完了：** PostgreSQLでの確認、cancel・接続断・commit結果不明を実backendで再現する確認、`defer`でのrollback（非同期完了の待機）。callbackで全操作を隠す形は、所有権とeffectの契約を先に設計するため作っていません。VM revert後は古い`Transaction`値が残り得ますが、終了済みの物理transactionは復活せず、次のcommit / rollbackはbackendのエラーとして`Failed`になります。

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

**部分実装（2026-10-14）：** `std.dbMigration`は計画と検証だけを担います。`migration`はid（1〜64文字の英数字と`_.-`）、backend（`sqlite` / `postgres`）、1〜256個のSQL command（各1〜65536 byte）を検証して不変の`Migration`にします。SQLは利用者が書いたものをそのまま保持し、分割も生成もしません。`checksum`はid・backend・各commandとその境界から作る16桁のhex（2本の多項式hash。偶発的な編集の検出用で、暗号用ではありません）。`plan`は、idが重複せず昇順であること、適用済み行が先頭からの接頭辞で`checksum`が一致すること、全migrationが指定backendであることを確かめ、未適用の末尾を返します。編集は`ChecksumMismatch`、列に無いidは`UnknownApplied`、飛ばされたidは`Gap`、順序違反は`Unordered`です。version tableの`createTableSql` / `selectAppliedSql` / `recordSql`を返します。

確認：純粋な計画の標準ライブラリtest3件と、実際のSQLiteで「2件を各々transactionで適用し、再実行では未適用0、編集したmigrationを実行前に検出、2つ目のcommandが失敗した3件目はtransaction全体がrollbackされ、1つ目のcommandで作ったtableも残らない」ことを確認する[tests/db_row_mapping.rs](../tests/db_row_mapping.rs)。**未完了（自動runnerとして公開しない理由）：** 同時runnerをDB側で排他できることの確認（PostgreSQLのadvisory lock、SQLiteの書き込みlock下の挙動）、PostgreSQL DDLのtransaction性と再開条件、接続断・二重runnerの再現。このmoduleはversion tableへの記録と各migrationの適用を利用者のプログラムが明示的に行う前提で、importしただけではschemaを変更しません。

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

**部分実装（2026-10-14）：** [examples/notes-sqlite](../examples/notes-sqlite/README.md)がSQLite memory DBで、migration（`std.dbMigration`）の適用、入力検証（タイトルが空なら保存しない）、`std.dbTransaction`で2件を1つのtransactionにまとめた保存、`mapBatch`での一覧、2件目が失敗した保存のrollbackと件数の確認までを実行します。出力は[tests/db_row_mapping.rs](../tests/db_row_mapping.rs)で固定しています。この例はコンソール版で、GUIと保存結果不明時の表示、window終了時のcleanup、PostgreSQL差分は含みません（いずれも未完了）。

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
