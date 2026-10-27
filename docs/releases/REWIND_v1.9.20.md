# REWIND v1.9.20

## Task の実行予算と寿命

language 1.9.20 以降は命令数カウンタを Task が所有し、checkpoint / branch の clone と共有する。同じ Task を revert / resume しても累積の `--task-steps` はリセットしない。実行位置と値の復元、予算の消費を区別する。

終了した Task とその全保持先が到達不能になれば、既存 scheduler の回収とともにカウンタも解放する。checkpoint に残る Task はカウンタを保持する。過去の全 Task ID を別表に永久保持しない。巻き戻した後に新規作成した Task は新しいカウンタを持ち、再利用された scheduler ID の過去の消費量を継承しない。VM 全体の累積 execution / native work 予算は従来どおり巻き戻らない。

`rewind profile` の `task_instructions` は current scheduler が保持する Task の累積命令数を示す。回収した Task を含む全体の合計は `task_instruction_total` で確認できる。checkpoint のみが保持する Task は current の表には含まないが、復元すれば同じカウンタが再び表示される。1.9.19 以前を指定したプログラムは旧 ID 別の予算・profile 契約を維持する。

snapshot に跨るカウンタの共有、最後の保持先を捨てた際の解放、ID が再利用された新規 Task の独立性、短い Task を繰り返した際の profile の保持量、source-free debug / compact replay を検証する。

これは scheduler の quota metadata の改修であり、外部観測・接続の長時間運用、HTTP server / TCP、残る GUI 統合・kernel の分割、一般 container の保持量など、v2.0 の残る到達条件は継続する。
