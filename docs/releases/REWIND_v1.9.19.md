# REWIND v1.9.19

## Native GUI の非同期待機

`std.guiWindows.nextEventAnyAsync()->Task<Result<WindowEvent,GuiError>>` は application task から作る cold Task。start / await した後、VM scheduler がメインスレッドで非 blocking な native poll を行う。待ち合わせ中も ready な VM Task と HTTP / DB driver を進め、待機だけのときは既存 host 待機と同様に短く sleep する。replay 中は sleep せず記録を使う。kernel 一つの処理中まで preempt する変更ではない。

同時に input を待ち続ける Task は一つ。既存の pending waiter がある場合は `GuiWaitBusy`、未公開なら `GuiWindowNotPublished`、全 host が閉鎖済みなら `GuiClosed`、scripted event の末尾なら `GuiWindowEventTapeEnd`。入力 event を取得した後は同じ Task が pure decoder を実行し、型付き WindowEvent を返す。別の子 Task / mapper を残さない。decoder が実行される前でも、既に捕捉済みの入力は journal に記録されている。

cancel / requestCancel は次の入力を読み取る前に処理する。入力待機の cancellation は HTTP / DB Task を自動で cancel しない。不要な Task は application が cancel / join する。factory・画面 stage・同期 input は従来どおり application task に限定し、background Task に GUI 操作の許可を広げない。external / guard / cleanup の Task 制約を維持する。

## Journal と budget

同期 poll と非同期待機は mode を区別して記録する。同じ mode の空 poll は run length を保持する。非同期待機の terminal input error も記録し、入力 host がない replay / revert で同じ結果を返す。旧 trace の新 field は false / None として読む。不正な error、mode、繰返し terminal error / event、run length と cursor overflow は拒否する。

各 native poll は work を一つ課金する。入力を消費する前に最大 event・escaped JSON・Task 引数に対する64 KiBの native allocation を予約する。後続 decoder の VM work / memory には通常の budget が適用される。journal metadata の会計は実際の Rust struct size と run index、文字列 / event の費用を含む。これは論理保持量の検査であり process RSS そのものではない。checkpoint の cursor、high water、公開済み画面と host 資源の境界は維持する。

## 統合例と検証

`examples/gui-async` は parameterized SQLite の日本語の値を保存し、VM model を revert した後も保存値が存在することを検査する。新しい DB 操作を始める場所は `external fresh`。その後 GUI の loading と Cancel を表示し、HTTP と GUI Task を異なる型のまま待ち合わせる。GUI 操作で HTTP を cancel、または HTTP の status / failure を表示し、画面・DB 資源を cleanup する。HTTP の cancellation は受信側の処理の取消しを意味しない。

`smoke-gui-async-sdk.py` は実 HTTP server の応答を保留し、実 Win32 / X11 ウィンドウへ pointer input を送る。pending の request 中に Cancel が処理され、DB の保存値が保持されること、終了後に server / DB / source がなくても debug / compact replay が同じ出力を返すことを配布済み SDK で検査する。Linux の配布検証は Xvfb を用い、Windows は native desktop を用いる。fixture の型付き event / terminal error、cold cancellation、checkpoint、source-free、主 task / effect 制約、native 待機・閉鎖の回帰を維持する。

これは最小の GUI / HTTP / SQLite 統合であり、PostgreSQL の編集、多画面の loading / retry / error UX、HTTP server / TCP、追加 kernel、長時間の一般 container / resource の保持量、v2.0 の残る到達条件は継続する。今回の機能だけで v2.0 の完了とはしない。
