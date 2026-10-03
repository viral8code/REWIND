# REWIND v1.9.10

## 明示的な task 切替え

`import std.task as task; task.yieldNow();` は現在の VM task を ready に戻して scheduler へ渡す。Unit を返し、追加の Task / host worker を作らない。main task と async 関数で使え、tasks effect が必要。external 領域、cleanup、match guard での task 切替えは拒否する。関数経由の external 呼出しも検査し、動的な呼出しには実行時の internal 境界を適用する。

1.9.10 以降の既定選択は、最優先の ready task の中で task ID を巡回する。明示・記録済みの schedule choice は優先し、選択候補の順序は既存と同じ。高い優先度を設定した task に対する優先順位は変更しない。過去の language を使う program の既定選択は維持する。VM loop すべてに自動 preemption を追加した変更ではなく、長い処理は handoff / await を使う。

GUI の nextEvent / nextEventAny の待機は VM を占有する。GUI と VM task を併用する loop は pollAny / task.isDone() / task.yieldNow() を組み合わせる。旧ガイドの「yield」は明示 API として未提供だったため、この版で名前・effect と動作を具体化した。

## 協調的な数値処理

`std.numericAsync` は async 関数 dot / matmul を提供する。FloatArray は値として渡し、入力の native storage version を共有する。呼出しで Task<Result<T,StdError>> を作り、spawn / await を使う。await の外側 Result は TaskError、内側 Result は数値の StdError。

| API | 計算・保持契約 |
| --- | --- |
| dot(left,right) | 同形状の1次元 FloatArray。最大4096積ごとの native chunk の間で handoff。contiguous は page 範囲で走査し、strided view は順序を保つ。Neumaier の sum / correction を chunk 間に保持 |
| matmul(left,right) | rank 2、形状 (m,k) と (k,n)。最大4096積和ごとの chunk の間で handoff。出力 (m,n) の初期化は同期的な O(m*n)。変更範囲の page と tree path を COW で更新し、一つの chunk で各 page の copy / hash を一度にまとめる |

同期版と同じ要素の計算順序を維持する。値・shape・進捗は VM 状態であり、checkpoint の対象。純粋な計算に外部操作 ID / 観測 journal を使わず、別の worker thread を起動しない。task.cancel() は次の safe point で TaskError::Cancelled となり、途中の出力を返さない。既存の shape / dtype / finite / overflow / capacity と work / memory 予算を適用する。chunk の前に scratch / COW 更新容量を検査する。予算超過を通常の数値 Result に見せない。

4096 は work の区切りであり、OS の停止・スケジューリングを含む実時間の上限を保証しない。既存の synchronous API の動作は維持する。solve / QR / eigen 等の協調化はこの版の対象に含めず、後続で継続する。予算は RSS の測定ではなく保守的な admission units であり、共有された数値 page の重複計上と履歴 metadata の費用は引き続き監査する。

## replay の予算と診断

記録した native_work / execution_steps を replay の開始時に復元する。従来は task-step と history 予算を復元しても、これらが既定値に戻り、大きい計算だけ replay 中に失敗する場合があった。異なる明示予算、不正な型・0・範囲外の記録値を ReplayMismatch として拒否する。未指定の記録は既定の設定を維持する。

replay / schedule の照合失敗を通常の子 task failure へ変換しない。最初の不一致の位置を保持し、その結果として変わった最終状態の診断で上書きしない。最終照合は execution / state / result の相違を区別して表示する。

## 検証と配布

同期版との bitwise な内積・行列積、補償和、負 stride / transpose / broadcast、chunk がセルや page をまたぐ場合、空配列・shape error・非有限値・保持済み snapshot を検証する。三つの ready task の巡回、途中キャンセル、GUI の入力と終了、checkpoint の前後、100万 step を超える compact replay、予算改変と最初の schedule 不一致も検証する。

SDK の numeric-async 例は入力を共有した計算・foreground task・増分 publish と revert を含み、source-free artifact と debug / compact replay で実行する。Linux / Windows の SDK に module、API snapshot、例と説明を含める。再現用の速度・foreground handoff・peak RSS 測定は scripts/benchmark-numeric-cooperation.py を使う。

GUI の clipboard / dialog / menu / form / table / IME / accessibility、残る native kernel と共有 storage の費用、HTTP server / TCP、sparse / FFT / 勾配・optimizer と統合の到達条件は継続する。版番号の更新をもって v1.9 全工程や v2.0 の完了とはしない。
