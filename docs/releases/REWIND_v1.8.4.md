# REWIND v1.8.4 — 日時・暦・IANA タイムゾーン

v1.8 の標準型整備として `std.datetime` / `std.clock` / `std.dbDatetime` を追加する。既存の `std.external.millis()` と DB の `DbValue` variants は変更しない。日時変換は pure、実時計の観測と DB 操作は external とする。

## 値と範囲

- `Instant` は POSIX epoch の整数秒と 0..999999999 のナノ秒を保持する。不変の native 値で、UTC の Gregorian 年 1..9999 が範囲。浮動小数点への変換は行わない。うるう秒は非対応で、入力を拒否する。
- `Duration` は i64 秒と同範囲のナノ秒に正規化する。`duration(0,-1)` は `(-1,999999999)`。加減算・符号反転・Instant 加算で overflow を検査する。経過時間の加算は暦月の加算や DST を跨ぐ「現地で翌日」とは意味が異なる。
- 型の比較、Map key、hash、generic、List、freeze / thaw、task 移動、checkpoint、artifact / replay を既存の値の仕組みに統合する。serialized metadata の不正な範囲や非正規 Duration を拒否する。payload は固定長で、変換時の小さな scratch を実行前に予算へ算入する。

## API

`std.datetime` の `instant` / `duration`、`parse` / `format` / `formatOffset`、`seconds` / `nanoseconds` / `durationSeconds` / `durationNanoseconds`、`add` / `difference` / `addDurations` / `subtractDurations` / `negateDuration` を提供する。計算結果は `Result<...,StdError>`。既定の暗黙時刻・タイムゾーンは持たない。

`Calendar(year,month,day,hour,minute,second,nanosecond)` を `resolve(calendar,zone,ambiguity)` または `resolveOffset(calendar,offsetSeconds)` で Instant にする。calendar のフィールドは constructor で指定できるが、解決時に Gregorian 日付と全範囲を検査する。`calendar(instant,zone)` / `calendarOffset` は `LocalTime(calendar,offsetSeconds,weekday,dayOfYear)` を返す。weekday は Monday=1..Sunday=7、dayOfYear は 1..366。歴史的な offset は秒単位を維持する。

`parse` は完全な RFC3339、ASCII、`T` / `Z`、明示的 offset と小数部 1..9 桁を要求する。余分な桁を切り捨てない。unknown-offset `-00:00`、無効日、leap second、範囲超過を拒否する。`format` は UTC `Z`、`formatOffset` は whole-minute offset を要求し、local year の範囲も検査する。Instant の JSON は RFC3339 String とし `toJson` / `fromJson` で往復する。Duration は `durationToJson` / `durationFromJson` で整数 seconds / nanoseconds の object とし、読込みでは正規化済み範囲とフィールド数を検査する。

## タイムゾーンと DST

固定した chrono 0.4.42 / chrono-tz 0.10.4 に同梱する IANA データを使い、ホスト OS の local zone / zoneinfo に依存しない。`databaseVersion()` で同梱 IANA 版を得る。データ更新は処理系の版更新として行う。変換済み Instant は zone 名を含まず、同じ時刻は同じ key となる。zone が必要な業務データでは名前も別途保持する。

- 重複時刻は `Ambiguity::Reject` / `Earlier` / `Later` で明示的に選択する。Earlier / Later は UTC Instant の順序。
- 欠落時刻は全 policy で `DateTimeGap`。暗黙に次の時刻へずらさない。
- 無効 zone は `DateTimeZone`。誤った日付は `DateTimeCalendar`、未解決重複は `DateTimeAmbiguous`、無効 offset は `DateTimeOffset`、overflow は `DateTimeRange`。

## 時計と DB

`std.clock.now()` は `external,clock` effects を持ち external region が必要。SystemTime を一度観測して秒とナノ秒の一組を external ledger に記録する。revert は観測 cursor を戻し、同じ記録を再利用する。実時計を巻き戻さない。offline replay では時計を再観測しない。monotonic interval 計測や sleep / timer はこの API に含めない。

`std.dbDatetime.parameter` は PostgreSQL `timestamptz` 用の RFC3339 Text を作る。finite binary timestamp の signed microseconds since 2000-01-01 を Float を経由せず encode / decode する。サーバの session zone に依存せず、結果は UTC RFC3339 Text。nanosecond が 1000 の倍数でなければ拒否する。`infinity` と処理系範囲外も拒否し、曖昧な `timestamp without time zone` は Instant に自動変換しない。

`sqliteParameter` は RFC3339 Text でナノ秒まで保存する。DB の日時算術は別契約で、この Text の桁数維持を SQL 側で保証するものではない。`cell` は Text を検査して Instant に戻し、Null / 型不一致 / 列範囲を DbError として区別する。DB transaction と VM checkpoint は独立し、書込みは revert されない。

## 配布と検証

SDK に 46 個の公開 module と `tests`、API baselines、日時 / SQLite / PostgreSQL のサンプルを収録する。source-free 実行・record / replay、freeze / task / Map、DST overlap / gap（30 分遷移と日付丸ごとの欠落を含む）、負の epoch、overflow、不正 serde、実 PostgreSQL と DB を切り離した replay を検証する。Linux / Windows 両方の CI が成功してから main に統合し Release を公開する。

v1.8 の Unicode / regex / incremental stream は次の patch に残る。v1.9 の GC・性能・GUI と v2.0 の到達条件は既存計画に従い、この patch の追加だけでは完了扱いとしない。
