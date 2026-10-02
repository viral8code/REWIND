# REWIND v1.8.6 — 正規表現

`std.regex` は immutable な `Regex` と capture span を提供する。`compile` / `compileOptions` は UTF-8 text 用、`compileBytes` / `compileBytesOptions` は raw Bytes 用。異なる mode への入力は `RegexType`。既存 String equality、正規化、Map key の挙動を変更しない。

## 検索と保存

`find` / `findFrom` と byte 版は `Result<Option<Match>,StdError>`。`span` は全体、`group` は index 0 の全体と任意 capture を返す。未参加の optional group は None、参加した空 capture は空 Span。`groupCount` は group 0 を含み、`groupNames` の無名 group は None。

全 span と検索 start は byte offset、start inclusive / end exclusive。text の start は UTF-8 scalar boundary が必要。`text` / `bytes` で安全に抽出する。検索開始以前の anchor / word-boundary context を保持し、単に入力を slice して検索しない。

`next` は非空 match の end、空 match の次の scalar / byte を返す。終端の空 match は None。繰り返し検索で空 match に停滞しない。Options は caseInsensitive / multiLine / dotAll の明示的な Bool。capture は leftmost-first の regex-automata PikeVM に従う。backreference と lookaround は対応せず `RegexSyntax`。

Regex は checkpoint、freeze / thaw、task、Map key、record / replay に対応する。equality / ordering は pattern source、mode、flags を比較する。異なる source の意味的同値判定は行わない。`toJson` / `fromJson` は pattern / text / caseInsensitive / multiLine / dotAll の厳密な五つの field。VM serialization は source と flags のみを保存し、復元後の NFA は初回操作時に予算内で再構築する。

## 資源契約

pattern は 1,024 bytes、入力は 1 MiB、compiled NFA は 512 KiB、capture は group 0 を含め 32 個、構文 nesting は 32 まで。型・offset・syntax・NFA size・group・入力上限を typed error で区別する。compiler scratch は保守的に 48 MiB を事前検査する。検索の work は NFA state 数 × input bytes に比例して事前課金し、capture cache の scratch も検査する。バックトラッキングの指数的探索を行わない。既定の累積 work budget で全上限まで実行できることは保証しない。

compiled plan は Arc で共有し、検索 cache は呼出しごとに解放する。checkpoint の plan を変更せず、予算超過は fatal のまま Result に隠さない。復元された未構築 plan は最大 NFA 分の retained memory を保守的に予約する。共有 allocation の正確な集計と長時間実行の work policy は v1.9 の監査対象。

`engineVersion()` は regex-automata 0.4.18 / regex-syntax 0.8.11 / regex Unicode 16.0.0 を返す。std.unicode の normalization / segmentation は Unicode 17.0.0 であり、同一データ版とは扱わない。Bytes の既定 character class は Unicode 無効で、text は Unicode 有効。

## 検証・配布

Unicode byte span、optional / named capture、raw byte、空 match の進行、検索 start の context、不正構文・上限、事前予算拒否、checkpoint、Map、freeze / task、JSON と source-free replay を検証する。SDK に API baseline と regex サンプルを収録し、公開 48 modules と tests を同梱する。Linux / Windows の CI が成功した frozen commit を main に統合して Release を公開する。

incremental JSON / CSV と v1.9 / v2.0 の到達条件は既存計画を継続する。nested pattern の網羅性判定で見つかった問題は v2-design の言語監査に記録し、今回の wrapper は nested match で回避する。
