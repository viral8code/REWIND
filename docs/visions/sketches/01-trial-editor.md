# Sketch：複数案を比べて採用する設定editor

## 使いたい体験

入力欄を変更するたび現在の設定を書き換えるのではなく、複数の案を保持する。案ごとに検証、計算preview、差分を見て、採用した案だけをcurrent modelへ反映する。

Undoでcurrent modelを戻しても、案の一覧と比較結果は残る。そこから別案を再検討できるeditorを想像する。

## 組み合わせる構想

- checkpoint外state：候補catalogと試行回数。
- Snapshot<T>：採用前・採用後のmodel版。
- Trial<T>：隔離した編集と、採用する変更案。
- Decision<T>/Stale<T>：検証の前提と失効。
- ChangeSet<T>：差分表示と適用。

## 擬似コード

~~~text
hyper var candidates = CandidateCatalog<Settings>();
hyper var tries = 0;
var settings = loadInitialSettings();

fn propose(edit):
    tries += 1;
    let base = snapshot.capture(&settings);

    let trial = trial.from(base, |draft| {
        edit(draft);
        let validated = validate(draft)?;
        let preview = calculatePreview(validated)?;
        return Ok(preview);
    });

    candidates.add(base, trial);
    return trial.summary();

fn adopt(candidateId):
    let candidate = candidates.get(candidateId);
    let checked = candidate.revalidateAgainst(&settings)?;
    commit beforeAdoption;
    checked.apply(&mut settings)?;
    return Ok(snapshot.capture(&settings));
~~~

このcodeはAPI形状の例であり、関数定義syntaxも含めて現行でcompileできるものではない。

## 状態の分担

| 値 | 戻したいもの | 保持したいもの |
|---|---|---|
| settings | Undo対象のcurrent model | snapshotで保持する過去版 |
| candidates | 通常Undoでは戻さない | 候補とその根拠 |
| tries | 通常Undoでは減らさない | 試した回数 |
| trial draft | 採用前は隔離 | 採用・破棄までの変更案 |
| preview | 前提が変わればstale | 比較用の旧結果 |

## 採用前にcurrentが変わった場合

候補を作った後、別案を採用してcurrentが変わることがある。そこで、候補のbaseと現在modelの差を確認する。

衝突しないfieldだけならrebaseする、衝突なら利用者へ比較を出す、計算の前提が変わったら再previewする、といったpolicyを選ぶ。型としては、検証済み候補でも現在に対して常に有効ではない。

## GUIの見え方

左にcurrent、中央に候補、右に差分とpreviewを置く。候補を消してもcurrentは変わらず、Undoしても候補catalogは消えない。

候補の説明には、作成したevent、前提version、変更field、計算に使った仕事量を添える。modelとOSへ公開した画面は別なので、採用・Undo後の描画は改めて公開する。

## もう少し広げるなら

候補同士のmerge、条件別のおすすめ、同じsubtreeのpreview cache、履歴summaryを組み合わせる。大量候補では一覧用summaryと、詳細snapshotを分けて保持する。

外部へ保存する場合は、modelを採用する操作と保存Intentを作る操作を分ける。DBやremote設定をVMのUndoで戻したことにはしない。
