# 版を区別する容量付きmemo cache

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[VCACHE-02/03/04/06/12](visions/26-versioned-computation-cache.md)の明示keyによる基本部分をstd.memoへ実装した。依存読取りの自動追跡、関数の自動memoization、offline cache、hyper領域は含めない。

## Keyの意味

key(domain,revision,input)は用途の名前、code/libraryの版、入力identityを明示してKeyを作る。domainは1〜128 scalar、revisionは1〜256、inputは0〜4,096。inputはcontent hash、data version、必要なfieldを明示形式で表した文字列等、callerが同値とみなす範囲を選ぶ。

三つのStringはJSONの配列としてencodeし、区切り文字を含むkeyの衝突を避ける。同じ入力でもrevisionが違えば別entry。inputの生成費用や、同じkeyが同じ純粋な計算結果を意味することはcallerの契約。捕捉値を変更したのにkeyを変えない誤りを自動検出しない。

domain/revision/inputでmetadataを読める。Keyは権限や署名ではなく、cache内の一致条件。巨大な入力をlibraryが毎回暗黙にhashすることはない。

## 保持上限と順序

create<T:Share>(capacity,weightLimit)で、1〜65,536件と正の重み上限を指定する。weightはcallerが付ける正の整数で、物理byte量を測定した値ではない。実際のVM/native memory予算は引き続き適用する。

putは重みを先に検査し、不正ならcacheを変更しない。既存keyを置換し、件数と合計weightの両方に収まるまでleast-recent entryを外す。外したKeyを順に返す。置換対象自身はeviction一覧に含めない。上限がInt最大値でも加算がoverflowしないよう、weightLimit-weightを使って事前に空きを作る。

getのhitは項目を末尾へ移し、peekは順序を変えない。keysはleast→most recentのmetadata列。length / weightは現在の件数と重み合計。単一のevictionやtouchでは全keyを列挙せず、orderedMapの両端操作とリンク更新を使う。

## 計算と失効

resolveはhitなら保存値を返し、missならpure Send+Shareな引数なしcallbackを一回呼んで保存する。Resolved<T>はvalueとhitを持つ。weightはhitの場合も検査する。cache内で既に持つ重みをhitの引数で書き換えることはない。

TをResult<U,E>にすれば、入力に依存する回復可能なvalidation失敗も明示してcacheできる。callback panicとfatal budget失敗は捕捉・保存しない。network failureを自動的に同じ意味の結果として保持する機能ではない。

removeは一件を解除して値を返す。invalidateはpure predicateでKeyを選んでから解除し、解除したKeyを返す。callback評価中にcacheを段階的に書き換えない。clearは全entryを解除して重みを0にする。更新途中のfatal予算失敗までatomic recoveryを追加するものではない。

## 所有権・費用・巻き戻し

返す値はShare。可変ownerが必要な場合はFrozenを保存してcallerがthawする。evict/remove/clearしても、取得済みの値やsnapshotが保持するdataは有効なまま。cacheが登録を外したことと、全payloadが物理的に解放されたことを同一視しない。

Cacheは通常のVM ownerであり、内容とLRU順序はcommit/revertの対象。main task専用の大域領域やcheckpoint外stateを追加しない。function等をfreezeできないという既存の制限も維持する。

lookup/touch/removeは少数のMap操作、k件のevictionはk回の両端解除。keys/invalidateは全件を調べる。key生成は指定Stringの長さに比例するcodec費用を持つ。freeze/copy/native pageの費用も通常の予算へ含む。実測の高速化やGC完了を宣言するものではない。

## 確認source

libraries/std/tests.rwへ4件追加した。版の区別、LRU、keyの区切り衝突、weightと置換、Int最大weight、typed error前の入力不変、Errのcache、hit時のcallback省略、選択失効、snapshotと取得済みrootを扱う。実行は省略。

利用例は[versioned-cache.rw](../examples/versioned-cache.rw)。SDK moduleへ登録したが、SDK/API生成・公開は行っていない。
