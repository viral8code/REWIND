# 明示的な遅延値と結果snapshot

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[VCACHE-07/06](visions/26-versioned-computation-cache.md)の明示的な遅延計算をstd.lazyへ実装した。同期のpureな計算一件を所有するLazy<T:Send,E:Share>を使い、計算の開始・参照・再計算を呼出側が選ぶ。

## 計算と観測

createはfn()->Result<T,E> effects {} captures {Send,Share}を受け、その時点では実行しない。isForcedとpeekは計算せず、結果の有無だけを読む。peekはNone、Some(Ok(Frozen<T>))、Some(Err(E))を区別する。

forceは未計算ならcallbackを一回呼ぶ。成功値はstd.snapshot.captureで保持し、Frozen<T>を返す。回復可能なErrも保持し、以後のforce/peekから同じ失敗値を返す。callback panic、freezeの制約違反、fatal VM budget failureは捕捉・cacheしない。

TはSendでよく、Listや独占ownerのmodelを返すbuilderも使える。ただし既存snapshotのcapture条件を満たす必要がある。T:Sendだけでfunctionやnative resource等を必ずfreezeできるようになるわけではない。

## 分岐と再計算

forkは必要ならforceし、結果から独立したTのownerを返す。複数のforkや元のFrozen rootは、どれかの可変値を編集しても変わらない。T自体がFrozenの場合も、既存freeze/thawの一段の型を維持する。

clearはこのLazyが保持する結果だけを解除する。未計算ならfalse、保持結果があればtrue。次のforceで同じcallbackとimmutableな捕捉を再評価する。既に返したFrozen rootやforkは有効。新しい入力を使いたい場合は、新しいcallback/captureからLazyを作る。

Lazy自体はfunctionを持つため、data snapshotのfunction制約を引き継ぐ。通常のVM ownerとして計算済み状態はcommit/revertの対象となり、checkpoint外へ保持するものではない。debug表示や通常のgetterがforceを暗黙に呼ぶこともない。

## 費用と範囲

未計算時はcallbackとcaptureの費用、計算済み時は保持結果の参照費用を持つ。forkは既存thawの費用を持ち、任意の巨大modelでO(1)とは約束しない。必要な結果rootを保持するmemoryは利用者の保持として扱う。

同期APIで、複数taskの同時計算を一つへまとめる機能、外部I/Oの遅延実行、取消やoffline保存は含めない。callbackのwork/allocationと結果の寿命は既存VM予算・回収に従う。実測のGC/速度評価は行っていない。

## 確認source

libraries/std/tests.rwへ3件追加した。peekの非評価、独立したListのfork、clear後の旧root、回復可能なErr、入れ子Frozenの型維持を扱う。compile・実行は省略。

利用例は[lazy-snapshot.rw](../examples/lazy-snapshot.rw)。SDKへmodule登録したが、SDK/API生成・公開は行っていない。
