# Sketch：部分失敗を残さないstream import

## 使いたい体験

大きなCSV/JSONを少しずつ読み、検証して複数indexへ登録する。batchの途中で失敗したら、そのbatchだけは登録前へ戻し、原因と入力位置を返す。

ファイル全体をRAMへ載せず、成功したbatch、失敗したbatch、停止した位置を利用者が確認できる処理を想像する。

## 組み合わせる構想

- bounded stream：取得・decode・parseの流れ。
- schemaと複数errorの検証。
- attempt block：batch内の失敗単位。
- 複数indexの永続collection。
- checkpoint外state：試行統計と報告。
- 仮想入力：同じ失敗batchを再現するtape。

## 擬似コード

~~~text
hyper var report = ImportReport();
var records = IndexedCollection<Row>();

for batch in source.batches(maxRows, maxBytes):
    let result = attempt {
        let rows = schema.validateBatch(batch)?;
        for row in rows:
            records.insert(row)?;
        records.checkIndexConsistency()?;
        Ok(batch.identity)
    };

    match result:
        Ok(id):
            report.accepted(id);
        Err(problem):
            report.rejected(batch.identity, problem);
            if policy == Stop:
                break;
~~~

報告がcheckpoint外にあるのは、戻したbatchの失敗を忘れないため。処理対象のrecordsは通常modelとして戻す。

## batchを戻す範囲

record集合とそのindexを同じ失敗単位にする。途中で一つのindexへだけ登録されたstateは、Err後に残さない。

入力をphysical sourceから取得した事実は戻さない。取得済みbatchを必要な範囲だけ保持し、再検証するときはそのdataか仮想tapeを使う。

## 後から再開する場合

sourceのidentity、schema version、最後に受理したbatch、残った入力等をまとめる。ただしremote sourceが変わり得る場合は、同じ入力の続きだと保証できる条件を確認する。

保存したrestart dataを読み込むことと、実sourceへ再接続することは別操作にする。過去の報告をそのまま現在sourceの成功保証にしない。

## 全体成功が必要な場合

batchごとにcurrent modelへ反映する方式とは別に、新しいimmutable rootへすべて登録し、完成後にrootを採用する方式も考える。

この方式なら、現在の公開modelを処理途中で変更しない。全入力分の保持が必要な場合と、pack/storageへspillする場合を費用で選びたい。

## DBへ送るなら

attemptのVM内rollbackを、DB transactionの代わりにしない。batchをtypedな保存Intentへ変換し、backendのtransactionで確定した結果をreceiptとして報告する。

結果不明なら「失敗batch」として無条件に再送せず、照合待ちの状態として保持する。中断後の再開にもその状態を使う。

## もう少し広げるなら

同じrecordの重複処理、schema違い、source別のerror傾向、dry-runと本登録の差分を可視化する。synthetic datasetと仮想入力を用意すれば、実dataなしで同じpipelineの挙動を調べられる。
