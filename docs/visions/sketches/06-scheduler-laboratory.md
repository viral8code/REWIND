# Sketch：taskの実行順を探索するlaboratory

## 使いたい体験

同じ入力とcodeに対し、taskの進む順序を変えてraceや待機循環を探す。失敗した順序を小さな記録にし、code修正後の結果を並べて確認する。

randomに何度も実行するだけでなく、「どの選択を変えたか」「どの範囲を探索したか」を説明するlaboratoryが欲しい。

## 組み合わせる構想

- task群のsavepoint。
- 仮想入力とsimulation clock。
- branchの探索strategy。
- 時間上の不変条件。
- causal graphとfailure縮小。
- checkpoint外の探索統計。

## 擬似コード

~~~text
hyper var findings = FindingCatalog();
let initial = group.captureDataAndSchedule();

for schedule in strategies.enumerate(initial):
    let run = sandbox.run(initial, inputTape, schedule);
    let violation = monitors.check(run.events);

    if violation.exists():
        let reduced = shrinkSchedule(run, violation);
        findings.add(reduced, violation);
~~~

これは実taskを任意の瞬間に巻き戻す現在APIではない。対象group、停止点、保存可能stateを定める構想。

## physical操作を含むcode

探索中はfixture/simulation adapterへ接続する。現実へ送った操作を探索ごとに取消す方式にはしない。

元の記録がある場合は、観測dataを仮想入力へ移す。新しいscheduler選択と矛盾する観測があるなら、その試行の限界を示す。

## 何を失敗とするか

deadlock、期限超過、不正state、終了しないcleanup等を条件として書く。modelの一時状態と最終状態だけでなく、event順の性質も確認する。

打切りは反例とは別の結果にする。反例が見つからないことを全順序の正しさと同じ意味にはしない。

## 縮小するもの

不要なtask、入力event、scheduler選択、待機を取り除く。失敗の原因を保つ短い順序へ縮小し、sourceと関係するeventを残す。

縮小にも予算を持たせる。失敗記録の巨大な全stateを無制限に複製しない。

## codeを修正した後

旧新runを並べ、最初に異なる状態や待機条件を探す。新しいcodeで同じ選択列が適用できない場合は、その理由も表示する。

値の修正だけでなく、work、保持memory、取消応答の変化を調べる。backendが異なる場合も、同じ意味のeventへ対応させたい。

## もう少し広げるなら

protocol model checking、effect contract、state machineの反例生成を同じ画面へ接続する。testの失敗からlaboratoryを開き、調査結果を新しいfixtureとして保存したい。
