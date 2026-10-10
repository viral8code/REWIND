# Sketch：仮説と観測を並べる推論explorer

## 使いたい体験

modelの仮定を変えた推論を並べ、結果が違う理由を観測・乱数選択・parameterへ辿る。推論途中の状態を保存し、別のstrategyや観測版で試す体験を考える。

見た目のplotだけで信頼性を判断せず、診断、計算budget、未確認の仮定を同じrunへ残したい。

## 組み合わせる構想

- addressを持つrandom choiceと部分再実行。
- observeと介入の別操作。
- typed InferenceResultとsolverの停止結果。
- 永続trace、versioned cache、checkpoint外metrics。
- notebookと比較chart。
- snapshot packへの選択export。

## 擬似コード

~~~text
hyper var experiments = ExperimentCatalog();
hyper var metrics = InferenceMetrics();

let observations = snapshot.capture(dataset);
let model = compileProbabilistic(sourceVersion);
let base = model.initialize(observations, seed);

for strategy in selectedStrategies {
    let run = strategy.start(
        state = snapshot.fork(base),
        budget = PhysicalWorkBudget(perRunLimit),
        diagnostics = requestedDiagnostics);

    while run.canAdvance {
        let update = run.advance();
        metrics.record(run.identity, update.diagnosticSummary);

        if userRequestedBookmark {
            experiments.bookmark(run.exportDataState());
        }
    }

    experiments.record(
        input = observations,
        model = model.version,
        strategy = strategy.identity,
        result = run.finishWithStatus());
}

showPosteriorComparison(experiments, with = AssumptionsAndDiagnostics);
~~~

未実装の擬似API。forkするのはdata stateで、進行中のdevice kernelや外部接続を複製しない。

## 部分再計算

同じmodelの一つのlatent choiceを変更し、影響するtraceだけ再評価したい。choice集合が変わるbranchでは、新規・削除のaddressも差分に含める。

traceの共有は推論backendの確率計算に従う。単に変更されていない値を再利用すれば、推定が正しいという保証にはしない。

## runの比較

同じ観測を使ったstrategy比較と、観測が異なるmodel比較を分ける。診断が未取得のrunに、診断済みrunと同じ記号を付けない。

乱数seedが同じことだけで、異なるalgorithmの乱数利用が対応しているとは解釈しない。addressやstream keyの対応を明示した比較を使う。

## 不確実性を減らす次の観測

予測の差が大きい地点から、次の測定候補を作る。期待情報量、費用、実行可能性を計算上の案として表示する。

現実の測定を採用した場合はIntentへ渡す。取得した観測は新しい入力版になり、古いrunの観測へ黙って追加しない。

## memoryとexport

trace全体、posterior sample、要約、選んだbookmarkを別ownerにする。利用者がpinしたstateを一般cacheのevictionで失わない。

exportはcode版、観測の公開view、推論data state、diagnosticsを明示選択する。秘密dataやprovider handleを自動的に含めない。

## もう少し広げるなら

小さい反例、推論の停止理由、gradient診断、反事実の比較、試行の再現reportをつなぐ。REWINDのbranchと版付き値を、推論の説明にも使えると面白い。
