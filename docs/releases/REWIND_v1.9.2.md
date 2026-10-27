# REWIND v1.9.2 — 範囲読み出しと終了 task の入力解放

## FileHandle の読み出し

snapshot と VM の仮想 file に対する read / readBytes は、要求した範囲を含む page だけを読む。従来の全 file を一度 Vec に展開してから範囲を切り出す処理を削除する。返す buffer は `min(count, fileLength-position)` とし、EOF / zero count を保つ。offset + count の overflow は終端へ clamp する。

変更されていない page の Bytes は共有 storage に残る。小さい read の費用は読み出す page 数に従い、全 file の byte 数に比例するコピーをしない。読まない spill page には触れない。要求範囲の page が欠損・不正なら失敗し、位置を進めない。

File.openSnapshot が既に VM 内にある file を開く場合は、既存 persistent PagedFile を共有する。後の write / truncate は COW で旧 snapshot と独立する。host file の openSnapshot は引き続き全体を観測して snapshot にする。初回 host snapshot の RAM が bounded になったと主張しない。

## task の完了

1.9.2 以降の task が Done になると、実行開始用の global bindings / scope ID、Function の引数、Send の入力値を捨てる。observable result、failure、task ID、join / select / timeout の関係は維持する。完了 task の結果を複数回 await する動作を保つ。開始前の checkpoint は独立した scheduler を保持し、復元後に元の引数で実行できる。

これは実行用の入力の解放である。結果自体、未観測の失敗、ユーザーが持つ task handle、checkpoint の scheduler、channel queue の全回収を実装した版ではない。旧言語版の artifact の scheduler accounting は従来の扱いを保つ。

## 検証・残り

page をまたぐ read、EOF、zero count、巨大 offset、未要求 spill page の欠損、仮想 file 更新・checkpoint / revert を検証する。大きい引数の task が完了すると current scheduler の保持量が減り、repeated await と cold checkpoint、source-free replay が同じ結果を返すことを確認する。

次の単位では完了 task / channel / group の到達可能性と保持、checkpoint に入る不要 heap、共有 buffer accounting、bounded host file snapshot と trace streaming、native work / 公平性を続ける。GUI・アルゴリズム・v2.0 統合の到達条件も未完了のまま継続する。
