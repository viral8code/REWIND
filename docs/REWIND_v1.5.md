# REWIND v1.5.0 草案

状態を巻き戻せる領域と、外部へ直接作用する領域の境界を言語と実行時に導入する。HTTP と DB は次の版でこの境界を利用する。

## 明示領域

`external { ... }`（構文は実装時に確定）では、外部 effect を持つ API を実行できる。checkpoint の作成・revert・resume・branch・publish をこの領域の中で混在させない。外部領域からはコピー可能な結果を通常の VM 状態へ返し、native connection 等の資源は型付き handle として管理する。

通常の heap や未公開 I/O は従来どおり巻き戻せる。外部操作の成功を publish の待ち行列へ偽装しない。DB transaction の commit / rollback は DB 側の操作であり、VM の commit / revert と別の API として扱う。

## 記録・再実行

操作 ID、操作の fingerprint、結果を外部 ledger に残す。同じ checkpoint に戻って同じ操作を通る場合、観測済み結果を利用して外部操作を二重実行しない。異なる要求へ ID が再利用された場合は拒否し、意図的に新しい操作を行う経路を明示する。native handle は記録された ID と実接続の寿命を分離する。

Replay は記録した結果を使い、ネットワーク送信・DB 書込み・接続を行わない。credential は通常の出力・診断・記録へ露出させない。本文や行データの記録は利用者が指定した上限と記録方針に従う。

## 失敗・資源

外部 effect は CLI / manifest で明示許可する。正常終了、エラー、キャンセル、resource の scope 終了で接続を解放する。外部作用が成功した後のローカルエラーで、外部を自動 rollback したと主張しない。部分失敗と送信結果が不明な失敗を区別し、自動再送を既定で禁止する。

## 検証

境界の静的検査、同じ経路の再実行と異なる要求、record / replay、資源の close 後の handle、失敗とキャンセル、heap / checkpoint を保持したときの native resource 寿命をテストする。v1.6 の HTTP と v1.7 の DB は別の仕様と契約テストを追加する。
