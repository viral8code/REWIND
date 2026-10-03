# REWIND v1.9.6 — compact 記録と逐次ダイジェスト

## 記録方式

```sh
rewind run main.rw --record trace.json --record-mode compact
rewind replay trace.json
```

`--record-mode debug` は既定の方式で、命令ごとの source location / event、state delta / anchor、checkpoint inspection を保持し、ステップ移動を可能にする。debug index の 16 MiB、最終 trace の 128 MiB の上限は維持する。

compact は VM 命令ごとの task ID / PC を固定幅の little-endian byte 列として SHA-256 へ逐次入力し、命令数と digest を記録する。per-step event / debug snapshot は保持しない。artifact fingerprint、観測記録、schedule choices、最終 Runtime state digest、成功 / 診断、秘密操作の masked audit は保持する。checkpoint / revert をまたいでも累積の実行履歴 digest は戻さない。

replay は記録した方式を復元し、命令数 / digest、最終状態と結果を照合する。明示した CLI 方式が trace と異なる場合と未知の方式は ReplayMismatch。record_mode のない同 compiler の trace は debug として扱う。compiler version 一致、trace の署名検証、観測の要求照合、host に fallback しない規則は継続する。

compact はステップごとの local variable inspection を提供しない。`rewind debug` や debug-session のステップ表示に利用しようとした場合は CompactTraceNoDebugHistory で、debug 方式の再記録を案内する。`rewind profile` は独立して利用できる。

`rewind debug FILE.rwc` と `rewind profile FILE.rwc` も source-free artifact を受け付ける。debug は従来の source 実行と同様、virtual publish で inspection を出力する。profile は通常の公開規則で実行し、task / storage metrics を出力する。

## 費用・保証範囲

命令履歴の hash state は定数サイズで、値を hash 用の event にコピーしない。record / replay しない通常実行は、この hash 更新も行わない。schedule choices、secret audit、external / file / clock / GUI 等の観測、artifact と最終値にはそれぞれ既存の保持・容量契約が適用される。compact の選択で全 Runtime memory が定数になると保証しない。

最終状態 digest は従来と同じ Debug representation の byte 列を、全体 String へ整形せず fmt sink で逐次 hash する。stdout / stderr journal と file page も writer sink へ逐次読込みする。Debug wire と順序を変えず、spill した journal を含めて以前の byte 列との一致を検証する。観測 export の DOM / 16 MiB 上限はこの版で撤廃しない。

TraceBudgetExceeded の診断は HistoryStorage から分け、checkpoint の破棄だけでは debug trace の容量が戻らないことを伝える。詳細表示が不要なら compact 記録を案内する。既存の TaskError の予算分類では HistoryStorage を維持する。

`scripts/benchmark-recording.py` は同じ source-free artifact の 1,000 回の整数 loop を記録し、各記録の replay も検証する。同じ Linux x86_64 / Rust 1.98.1 optimized build、3 回の測定の中央値は、debug が 0.418 秒 / 170,992 KiB peak RSS / 9,968,382 byte trace、compact が 0.017 秒 / 12,288 KiB / 1,293 byte だった。記録時間に最終 trace の書出しを含み、compile と replay の時間は含めない。詳細 inspection を省く場合のこの workload の測定であり、外部観測や大きい最終状態まで同じ費用になるという保証ではない。

## 受入検証

長い loop、checkpoint / revert、source-free artifact、命令数 / hash の改変、記録方式の不一致、task / channel の scheduling、失敗時の診断を検証する。実 HTTP の一回の要求と revert による結果再利用を両記録方式で確認し、server と source を取り除いた replay でも要求を再送しない。file 観測は元の input を削除して再現し、公開済み host file の変更を replay が上書きしないことを確認する。

## 継続事項

streaming observation / trace、host file snapshot、task quota metadata、native kernel の応答性とキャンセル、残る GUI / algorithm / 数値・通信の統合条件は継続する。compact は全観測の streaming 形式を実装したという意味ではない。
