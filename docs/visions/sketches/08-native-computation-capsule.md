# Sketch：寿命を閉じ込めるnative計算capsule

## 使いたい体験

既存の数値libraryや画像libraryを使いたい。native memoryの寿命、例外的な失敗、callback、snapshotとの関係を、呼出し側が毎回手書きしなくて済む形を考える。

物理arenaを通常VM heapと同じように保存する仕様ではない。native呼出しの間だけ貸す値と、VM側へ戻せる結果を分ける。

## 組み合わせる構想

- 型付きmodule signatureとcapability。
- arena、borrow、generative region type。
- shape・dtype・strideを持つarray view。
- native費用の表示とcooperative cancellation。
- 値の配置とcopy policy。
- adapterを差し替える純粋計算contract。

## 擬似コード

~~~text
module LinearSolver = NativeCapsule.load(signature, selectedBackend);

fn solve(input: Frozen<Matrix<f64>>) -> Result<Owned<Vector<f64>>, SolveError> {
    return LinearSolver.withRegion(region => {
        let borrowed = region.borrowReadOnly(input);
        let workspace = region.allocateWorkspace(requiredWorkspace(input));
        let output = region.allocateVector(input.rows);

        let result = LinearSolver.factorAndSolve(
            borrowed, workspace, output,
            cancellation = CooperativePoll);

        result.checkStatus()?;
        return region.copyOut(output);
    });
}
~~~

仮の構文。regionのstorageを指すborrowがこの関数から逃げないことを型で検査する想像。

## snapshotとnative memory

Frozen入力をnative側へread-onlyで貸せるなら、ownerを呼出しの終わりまで保持する。外部libraryが入力を書き換えるcontractなら、明示的なcopyまたは独占所有の別APIを使う。

返却するOwned値はVMが寿命を追えるstorageへ移す。zero-copy移譲を許す場合は、解放関数、thread制約、allocation種別をownerに持たせる。任意のnative pointerをsnapshotへ埋め込める設計にはしない。

native呼出し中のworkspaceはcheckpoint対象にしない。結果を得てVMの値へ変換する段階で、rollback可能なmodelへ採用する。

## 失敗とcallback

libraryのstatus codeを型付きerrorへ変換する。契約違反・メモリ破損の疑いを通常の数値収束失敗と同じcatchで続行しない。

callbackを使うlibraryは、呼出し期間だけ有効なcallback leaseを受け取る。callback内から許すREWIND effect、呼出しthread、再入可能性をsignatureへ表したい。

## cancellationの限界

poll hookがあるlibraryなら、要求を協調的に渡す。hookがないblocking callは、キャンセル要求後も実行が続くことを表示する。

進行中のnative呼出しを強制破壊してarenaだけ解放する操作にはしない。process隔離を選ぶ場合は、その境界のprotocolと結果不明状態を別に設計する。

## 費用とbackendの選択

計算時間だけでなく、copy in/out、workspace、alignment調整、layout変換の費用を見たい。小さい入力では純粋REWIND実装、大きい入力ではnative backendを選ぶpolicyをlibraryへ渡せると便利。

backendが異なる場合の数値許容誤差と決定性の範囲もcontractへ持たせる。速いbackendへの切替えを、bit単位で同じ結果という約束へ読み替えない。

## もう少し広げるなら

複数capsuleのworkspace共用、device owner、columnar dataset view、ABI互換性検査を組み合わせる。一般利用者向けの簡単なAPIの裏側で、寿命と転送費用が確認できる体験を目指す。
