# 数値計算と GUI・HTTP・DB の同時実行

`service-data` のトランザクション例に、独立した数値計算 task を加えた例。
materialized 入力を分割して FloatArray に変換し、sigmoid → square → mean の
forward / backward を協調実行する。値と勾配の和を解析式と照合してから
`calculated` を通知する。通知用 Channel は容量 1 で、未処理通知を無制限に蓄積しない。

HTTP の rollback / save、native GUI のボタン操作、DB の確定と VM の revert は
同じ実行中に進む。外部 DB の確定済み書込みを revert で取り消すことはしない。
この通常例は128要素の計算を1回実行する。詳細記録でも扱える有限の例にしている。
終了時には数値 task が完了していればその結果を確認し、未完了なら取消して、GUI と DB / HTTP 資源を閉じる。
Stop ボタンは処理中のサービスを取消して終了する。応答前に HTTP の接続が閉じられても、
確定済みの DB 書込みを再実行せず、次のリクエストを受け付ける。

必要な許可は `gui,external,network,db,tasks,env,fileRead,output`。
引数と PostgreSQL の CA / secret 設定は `service-data` の README を参照。
同時利用には PostgreSQL 接続の保守的な予約を含む十分な history memory が必要。

開発中の検証用 `scripts/smoke-service-numeric-sdk.py` は、独立した DB 接続で
保存結果を照合し、数値計算の完了通知、native 入力、終了時の資源解放、
source-free の記録とサービスなし replay を確認する。
長時間試験や Windows SDK での受入が済むまでは、その完了をこの例だけで保証しない。

反復負荷では `scripts/smoke-service-numeric-sdk.py` に2〜16サイクルを指定し、
16384要素の連続計算とcompact記録で検証する。通常の詳細記録例とは負荷を区別する。
同スクリプトの末尾引数 `disconnect` / `early-stop` は、受理済みリクエストの
切断後の継続と、DB処理中のStop・資源回収を検証する。
