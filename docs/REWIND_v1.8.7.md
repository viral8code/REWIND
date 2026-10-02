# REWIND v1.8.7 — 増分 CSV

`std.csvStream` を追加する。既存の `std.csv.parse` / Table は変更しない。`reader()`、`feed(&mut reader,Bytes,finish)`、`next(&mut reader)`、`finish`、`cancel`、`position` は `Result<...,StdError>` を返す pure API。file / HTTP の chunk 取得は呼出し側の明示的な作用とし、CSV parser 自体は外部資源を所有しない。

## 状態と文法

UTF-8 field、comma、quoted field、二重 quote、quoted newline、LF / CRLF を扱う。UTF-8 scalar、quote、CRLF は任意の chunk boundary をまたげる。unquoted quote、閉じた quote の後の通常文字、単独 CR は error。空入力は 0 rows、空行は一つの空 field、末尾 comma は最後の空 field。数値・null・型を自動推論せず String の row として取り出す。

feed は成功時だけ状態を更新する。失敗時は当該 chunk 全体を消費せず、元の pending field と queue を維持する。`next` は一つの `List<String>` を移譲し、queue から取り除く。None は現時点の queue が空という意味で EOF を意味しない。`finish` が EOF を検査し、残った row を確定して parser を閉じる。閉じた後も queue は drain でき、feed は CsvClosed。`cancel` は pending / queue を捨てて閉じる。

position と error offset は stream 全体の 0-based byte offset。quote / trailing text / newline は問題の byte、EOF error は入力終端。UTF-8 error は失敗した field の開始位置。position が Int に収まらない場合は CsvPositionLimit。失敗した feed の position は元に戻る。

Reader は freeze / thaw、task、checkpoint、record / replay に対応する。revert は pending field と drain 状態を復元する。外部で確定した操作を CSV reader の復元で取り消すことはない。checkpoint に保持された parser / row の storage は、履歴が参照する間は解放しない。

## 上限・費用

- chunk は 65,536 bytes、decoded field は 1 MiB、decoded row は 4 MiB、row は 4,096 columns まで。
- 未取得 queue は 32 rows / decoded text 計 8 MiB まで。空 field の metadata も VM の memory admission に含める。
- queue limit では既存 row を drain し、同じ chunk を再送する。一つの chunk だけで上限を超える場合は、chunk を小さく分けて feed / drain を繰り返す。入力全体の一括保持を必要としない。

pending は immutable な 4 KiB page を共有し、追加時は末尾だけをコピーする。完成した field / row は確定時に UTF-8 検査と出力 allocation を行う。未完の巨大 field を各 feed で再連結しない。row / queue は Arc で共有し、clone / advance は入力全体をコピーしない。

chunk scanning、共有 metadata、完成候補の field / row の work と native scratch を事前検査する。予算超過は fatal で、typed CSV error に変換しない。固定上限とは別に Runtime の累積 work / memory budget が適用され、大きい入力は明示的な予算設定が必要。返却 List と reader の metadata、snapshot による保持も数える。

serialization は pending、文法状態、queue、position を保存する。復元時に field / row / queue の個数・サイズと状態の整合性を検査し、無制限の wire 配列を受理しない。

## 検証・配布

全 split point の UTF-8 / quote / CRLF、atomic error と absolute offset、EOF / empty field、queue backpressure、field / column / wire 上限、page 共有、pending checkpoint、freeze / task、実 file の chunk 読込みと file-free replay、source-free SDK を検証する。SDK は公開 49 modules と tests、API baseline、csv-stream example を収録する。Linux / Windows CI 成功後に main 統合・Release 公開する。

incremental JSON と v1.9 / v2.0 の到達条件は継続する。v1.8 工程全体の完了とは扱わない。
