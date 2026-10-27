# Sketch：編集途中も失わない型付きform

## 使いたい体験

modelのfieldからformを作り、入力途中の文字列、検証済みの値、採用した値を別々に扱う。deepなrecordでもpathとvalidationを共有し、Undoの対象が分かる画面を考える。

自動生成したwidgetを手書きevent処理で補い、codeやschemaを更新しても、入力を無条件に捨てない体験が欲しい。

## 組み合わせる構想

- typed field path、partial lens、invariant focus。
- normalizationと入力位置の対応。
- partial fragmentと生成hook。
- reactive graph、subscription owner、async derived value。
- Trialとdata snapshot履歴。
- version付きDecisionとeffect plan。

## 擬似コード

~~~text
hyper var history = ModelHistory<Order>();
var order = initialOrder;
var form = DraftForm.forSchema(Order);

form.bind(
    field = Order.path.customer.address,
    input = AddressTextParser,
    preserve = UncommittedTextAndCaret);

form.bind(
    field = Order.focus.amountAndCurrency,
    input = MoneyEditor,
    validate = MoneyPolicy);

on SubmitDraft {
    let edits = form.decodeAgainst(order.version)?;
    let trial = Trial.fork(snapshot.capture(order));

    trial.apply(edits)?;
    trial.validate(orderRules)?;
    show(trial.diff(), trial.validation);

    on Adopt {
        let change = trial.revalidateAgainst(order)?;
        let previous = snapshot.capture(order);
        order = change.apply(order)?;
        history.push(previous, snapshot.capture(order));
        form.acceptModel(order, preserving = OtherDraftFields);
    }
}
~~~

APIと構文は未実装の擬似例。SubmitDraftはVM modelへの採用であり、DB保存やnetwork送信ではない。

## 入力途中のstate

modelにMoney値が入る前でも、入力bufferには「-」や「1.」等の未確定文字列がある。parse errorで利用者の入力を丸ごと消さない。

caret、selection、IME compositionはviewの寿命に結び付ける。model snapshotを戻す時に、それらを維持するか再初期化するかを編集操作のpolicyとして選ぶ。

## 更新が競合した場合

background taskが同じfieldを変更していたら、formが参照した版と現在版を比較する。無関係なfieldだけ変わった場合はrebaseし、連動するinvariant unitが変わった場合は確認を返す。

型付きpathが見つかっただけで、古い入力を現在modelへ適用できるとは判定しない。Entityの削除やschema変更も別の結果にする。

## Undoと保存

Undoはhistoryに残したdata snapshotからorderを復元する。既に返ったGUI callbackのstackへrevertする操作にはしない。

DB保存はadopt後のmodel版からIntentを作り、実行結果をreceiptとして記録する。保存済みのorderを画面でUndoしても、DBが戻ったように表示しない。

## 生成部分と手書き部分

primary fragmentにはmodel、生成fragmentにはpathと標準widget、手書きfragmentにはdomain hookを置く案を考える。

生成fileの更新でcustom hookを上書きしない。hookの引数やeffectが変わった場合は、関連する位置を示して合成時に診断する。

## もう少し広げるなら

locale別の入力、screen reader向けの同じvalidation、差分preview、field単位の履歴、schema migration中の入力保存を組み合わせる。利用者が直すべき位置と、modelへ採用済みの状態を一貫して表示したい。
