# Sketch：複数の未来を比べるsimulation tree

## 使いたい体験

同じ初期状態から、操作・乱数・parameterの違う未来を計算し、途中の状態も並べて見たい。良い結果だけでなく、制約違反へ至った理由を残したい。

実際に動いた装置や利用者の行動を戻す機能ではない。入力済みの観測と仮のworld modelに対する探索を組み立てる。

## 組み合わせる構想

- 型付きdata snapshotと永続graph。
- simulation clockと明示的なrandom stream。
- branch search、best-so-far、状態の共有cache。
- 単位・区間・不確実性を持つ値。
- checkpoint外の候補catalogと試行budget。
- 差分を連動表示するchartとscene。

## 擬似コード

~~~text
hyper var runs = RunCatalog();
hyper var exploration = PhysicalWorkBudget(limit);

let origin = snapshot.capture(world);
let input = ObservationTape.freeze(observations);

for policy in policies {
    let candidate = Trial.fork(origin);

    let result = candidate.evaluate {
        let clock = SimulationClock(start);
        let random = RandomStream(seed).split(policy.stableKey);

        simulate(modelVersion, policy, input, clock, random,
                 until = horizon, budget = exploration);
    };

    runs.add(origin, policy, result, result.assumptions);
}

let comparison = compareRuns(runs, alignment = BySimulationTime);
show(comparison, with = ConstraintsAndUncertainty);
~~~

すべて仮のAPI。Trialは選択したmodelのdataを分岐させる例で、終了済みの関数stackへ戻る操作ではない。

## 共有できる計算

同じ初期rootと同じ入力prefixから生じた純粋計算を共有する。clock、乱数stream、policy、model code版が異なる場合、依存を無視して同じcacheへまとめない。

乱数はbranchの作成順ではなく、明示したbranch keyから分割できる設計を考える。特定branchの再実行に、他branchの列挙順が影響しない体験が欲しい。

## 予測と観測の照合

予測には使用した観測版と仮定を持たせる。新しい観測を受け取ったら、以前の予測の前提を表示した上で再推定する。

過去の予測を最新の観測へ書き換えて、最初から知っていたように表示しない。予測時点と対象時点の両方を残す。

## branchの採用

採用できるのは、simulation上の次の計画、parameter、または純粋modelの状態。現実の装置へ操作を出す場合はIntentとして別に作り、現在の観測と再照合してから実行する。

探索中に見つけた最良案は「調べた範囲で最良」と示す。予算終了・未探索の選択肢・数値誤差を隠して最適性を断定しない。

## memoryをどう見せるか

catalogの各runが保持するroot、共有page、独自page、traceを表示する。利用者がpinしたrunはcache evictionで失わない。

全tickのworldを保持するmodeと、疎なsnapshotと入力tapeから再計算するmodeを選べるようにしたい。再計算可能性は決定性契約で確認し、物理入力をもう一度取得する前提にしない。

## もう少し広げるなら

policy探索、反例の最小化、観測に対するparameter推定、誤差帯付きのplotをつなぐ。ある案が採用されなかった理由を、費用・制約・不確実性の差分として読めると面白い。
