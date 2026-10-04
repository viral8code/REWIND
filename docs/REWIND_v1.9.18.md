# REWIND v1.9.18

## 型の異なる Task の完了待ち合わせ

`std.task.selectReady<A,B>(left:Task<A>,right:Task<B>)->Task<Int>` と `left.selectReady(right)` を追加する。cold な待ち合わせ Task を start / await すると両入力を start し、完了した入力の index（左0、右1）を返す。scheduler が両方の完了を同時に観測した場合は左を選ぶ。入力の failure / cancellation も完了として通知し、その詳細は選択後に入力 Task を await して取得する。

待ち合わせは入力の result を複製・消費せず、observed にも変更しない。DbCursor / Model など affine な result を持つ Task も利用できる。待ち合わせ自身の cancellation は入力に伝播しない。不要な入力は呼出側で cancel / join する。既に完了した入力は再度選ばれるので、イベントループは処理済みの Task を次の待機 Task に置き換える。

pending の入力を GC の到達可能な identity として保持し、checkpoint は cold / running / waiting と入力の結果を従来通り保持する。入力の unobserved failure を隠す機能ではない。Task API は internal region に限定し、external region・guard・cleanup 内の制約を維持する。language 1.9.18 以降。

## GUI の空 poll 記録

同じ request / polling mode で連続した「入力なし」を run length で保持する。checkpoint cursor は圧縮後の record index ではなく実際の poll 回数を示す。二分探索で record を取得するため、圧縮記録の途中への revert、既存 run の末尾で新しく poll した後の revert、continueInput、offline replay の順序を保持する。event と mode の変化は圧縮しない。

一つの run は最大100万 poll、journal は最大100万 record。総 poll 数の cursor overflow を拒否する。memory admission は圧縮 metadata / 索引を含み、入力を消費する前に最大 event を予約する。既存の repeat field がない trace は1回の poll として読む。新 trace は現行 runtime を使用する。repeat=0、過大 run、繰返し event / blocking read を拒否する。ユーザーの commit を自動で捨てる変更はない。

10101回の空 poll を一つの112-byte論理記録（索引を含む）として保持する回帰を追加する。これは論理会計であり RSS ではない。機能の回数 / 順序を削減する圧縮ではない。

## 検証と残る作業

異なる型・affine result、tie、入力 failure、入力 / 待ち合わせの cancellation、checkpoint、旧 language rejection、source-free と両記録 mode の replay を検証する。空 poll の途中への restore、伸長、high water、旧 trace・不正 trace、budget admission を検証する。

GUI の非同期入力待機、GUI / DB / HTTP の一体的なアプリ例、HTTP server / TCP、追加 kernel の協調動作、長時間の resource / memory 検証と v2.0 の到達条件は継続する。今回の待ち合わせ API だけでは同期 GUI nextEventAny が VM task を進めるようにはならない。
