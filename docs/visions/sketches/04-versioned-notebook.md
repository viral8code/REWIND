# Sketch：cellの結果を版として持つnotebook

## 使いたい体験

cellを変更・再実行しても、以前の結果を失わない。code、入力、結果の版を持ち、二つの計算を比較してからcurrentの結果を選ぶ。

後続cellが古い入力へ依存している場合はstaleを示す。古い結果を参考表示できることと、現在の確定処理へ使えることを分けたい。

## 組み合わせる構想

- code/library identityを持つcell。
- Snapshot<T>とValueAt<T, Version>。
- 依存情報を持つpure call。
- 明示的なLazy<T>とcache。
- typedなIntentとReceipt。

## 擬似コード

~~~text
hyper var runs = NotebookRunCatalog();

fn evaluate(cell, inputs):
    let execution = cell.compile(profile = PureAnalysis)?;
    let run = execution.runWithSnapshots(inputs)?;
    runs.add(cell.identity, run);
    return run.resultView();

fn adopt(cell, run):
    run.validateDependencies(currentInputs)?;
    currentOutputs.set(cell.identity, run.outputSnapshot);
    return Ok();
~~~

cellのruntime実行を任意の時間まで戻すのではなく、入力と結果のsnapshotを版として保持する例。

## 依存の見え方

cell AからB、BからCへ値が流れる。Aのcodeを変えたら、B/Cの結果へ古い依存versionを表示する。

値が変わらなければ結果を再利用するpolicyと、codeが変われば必ず再実行するpolicyを比較する。cacheの理由は説明できる形にしたい。

## 副作用のあるcell

file保存やDB更新を、pure analysis cellの同じ実行modeへ混ぜない。最初はIntentを作ってpreviewするcellと、明示的に実行してReceiptを受けるcellを分ける。

古いrunを選択しても、receiptの物理的な操作履歴は戻らない。画面に「この結果は過去の入力で、保存は既に実行済み」と示せるようにする。

## memoryの扱い

全runの巨大配列を無制限に保持するのではなく、利用者がpinした結果、共有page、消えてよいcacheを分ける。

full結果をpackへ移す、thumbnailやaggregateだけ保持する等を選ぶ。縮小した結果から何が再計算・再取得になるかも表示する。

## もう少し広げるなら

二つのnotebook branchを並べ、最初に変わった結果とparameterを探す。query plan、chart、solverの説明をcell結果に付ける。

文書exportでは、再計算に必要な非機密入力と、結果を見るだけの保存dataを区別する。外部sourceへ自動接続せず、読める形で研究・教材・調査資料を渡したい。
