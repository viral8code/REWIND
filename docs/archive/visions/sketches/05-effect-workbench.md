# Sketch：外部操作を組み立てるworkbench

## 使いたい体験

入力や設定から、保存・送信する案を作る。案を変更してpreviewし、実行した場合の結果や、部分失敗後の照合まで同じ画面で追う。

VMのUndoで計画を戻せる一方、実行済み操作は履歴に残る。候補の変更と、現実へ作用する操作を見分けられるworkbenchを想像する。

## 組み合わせる構想

- typed Intent/Prepared/Receipt。
- read setとversion付きDecision。
- operation依存graph。
- checkpoint外のreceipt registry。
- simulation adapterとphysical adapter。
- 補償案と照合待ち状態。

## 擬似コード

~~~text
hyper var receipts = ReceiptRegistry();
var draft = OperationDraft();

let intent = buildIntent(draft)?;
let prepared = intent.validate(currentFacts)?;
let preview = simulation.evaluate(prepared);

show(preview, prepared.dependencies);

let ticket = prepared.issueTicket();
let outcome = physical.execute(ticket);
receipts.record(outcome);

currentModel = reduceOutcome(currentModel, outcome)?;
~~~

「physical.execute」は実操作を表す仮のAPI。previewが成功したことは、実操作の成功保証ではない。

## currentが変わった場合

検証時に参照したfactsと、実行前のfactsを照合する。予定した対象が変わったら、再検証・再計画・中止等の結果を返す。

backendが条件付き書込みを提供するならその能力を使う。照合と実行が離れるbackendでは、同じ強さの保証として表示しない。

## 部分成功と結果不明

operation Aは完了、Bは未開始、Cは結果不明という状態を一覧にする。まとめて「失敗」と表示して全件再送する操作にはしない。

receiptから照合用IDを取り出し、Cの状態を確認する。確認後に継続・補償・終了を選び、その操作も新しいreceiptへ残す。

## Undoで戻るもの

draft、入力欄、未採用のIntentは通常modelとして戻る。receiptsは通常Undoで消さず、既に行った操作の履歴として表示する。

receipt自体を後で永続保存するなら、専用storeとschemaを使う。hyper stateだけでprocess再起動後も残るとはしない。

## adapterを差し替える

simulation、fixture、実backendが同じoperation contractを使う。どのadapterで得た結果かは型・metadataへ残す。

fixtureがnetworkへfallbackしないmodeを用意する。計画の生成は実能力を要求せず、実行段階でcapabilityを提供する。

## もう少し広げるなら

複数案の比較、依存graphの変更、quotaの見積り、事前検証の再利用を組み合わせる。結果不明に対する確認義務を型で表し、次の画面でも状態が失われないようにしたい。
