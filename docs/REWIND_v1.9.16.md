# REWIND v1.9.16

## Native array 上の reverse-mode

`std.autodiff` は VM 内に保持する Tape と opaque Node を提供する。parameter / constant、同形 add/sub/mul、rank-two matmul / transpose、reshape、明示 broadcast、sum / mean、relu / sigmoid / tanh / exp / log / square を組み合わせ、one-element loss から backward する。gradients の gradient は parameter または中間 node の Option<FloatArray> を返し、constant / 未到達 node は None。relu の0での微分は0。

値と勾配は native Float64 pages。セルごとの VM object や callback を作らず、operation ごとの metadata を paged List に持つ。連続 reshape は storage を共有し、非連続 view は必要な場合だけ実体化する。broadcast の逆伝播は元の shape へ Neumaier sum する。backward は tape を変更せず、gradient の合流を native add で行う。Gradients は中間 gradient も保持するため、必要な leaf を取り出した後に所有者を捨てれば不要な page を解放できる。Tape / Gradients / optimizer は VM の GC root と checkpoint accounting の対象。

Node の constructor / fields は private。index と SHA-256 の graph key を照合し、異なる leaf、演算、parent graph、view / storage version に対応する handle を拒否する。key は cached page hash と descriptor から作り、セル全体を各 append で hash し直さない。構造が同一の graph は同じ key になり、意味が同じ handle は交換可能。checkpoint は tape と値を COW 復元する。Node は host capability や不可逆 token ではない。

parameter 名は1..128 UTF-8 bytes、NUL 不可、tape は最大100000 operation。入力の非有限値を拒否する。新しい operation はその単一呼出しが成功してから append する。backward の費用は各 forward / backward kernel の合計と node metadata、保持する中間勾配。全 API は同期処理であり、大きな単一 kernel を途中で yield / cancel できるとは保証しない。fatal work / memory budgets は引き続き適用する。

## Optimizer

`std.optimize.sgd` は named FloatArray map と同名・同形の gradient map から新しい weights を返す。`adam` と `adamStep` は bias-corrected Adam を提供し、first / second moments を native pages で保持する。正の有限 learning rate / epsilon、0 <= beta1,beta2 < 1 を要求し、epsilon は sqrt の外に加える。最大2^53-1 step。moment を checkpoint / revert できる。

名前・shape・値・configuration の typed error では入力 weights と Adam state を変更しない。全 output / moment の作成を終えてから状態を更新する。fatal VM 失敗後の復旧には checkpoint を使う。named map は model と同じ128 parameter /32 MiB encoded-size limit。余分・不足した parameter、非有限値、非表現可能な演算結果を拒否する。

## Model container と file 境界

`std.models.create/get/weights/encode/decode` は named native arrays を保持・取得・変換する。weights は parameter ごとの metadata をコピーし、native pages は共有する。model は maximum128 parameter、name は1..128 UTF-8 bytesで NUL 不可。encoding は最大32 MiBで、versioned magic、sorted UTF-8 names、rank/dimensions、logical-order little-endian Float64 bits、SHA-256 checksum を格納する。view は logical values を保存し、読込み後は独立した contiguous writable storage になる。符号付き0の bit も維持する。

decode は長さ、rank <=8、shape / numeric limit、canonical name order、重複、UTF-8、非有限値、trailing bytes、checksum を全て検証してから numeric pages を構築する。checksum は破損の検出であり authenticity signature ではない。native model はソース・実行コード・任意 host object を復元しない。encode/decode は bounded な Bytes を実体化し、streaming API ではない。

`save` は fileWrite、`load` は fileRead を明示要求する。source / SDK package の effect 宣言にも反映し、許可を自動付与しない。save は既存 File.writeBytes による VM 内の deferred write。publish 後の物理 file は revert しない。load は既存観測と replay 契約を使い、FileError を `StdError("File:"+code,0)` へ変換する。save の codec は typed error、File.writeBytes / publish の失敗は既存の fatal VM failure。file / trace / history budgets が container 上限より先に適用される場合もある。

## 検証

独立な有限差分で、matmul / broadcast / 非線形演算 / 同じ parent の再利用の勾配を比較する。transpose / non-contiguous reshape、log(exp(x))、constant、one-element loss、foreign handle、private constructor、language gate、typed optimizer error の atomicity と moment の checkpoint を検証する。model の view / IEEE bit round-trip、checksum、truncation、不正 length、重複、NaN を検証する。

`examples/model-training` は200回の Adam 更新で二つの matrix weight を学習し、loss 減少・model codec・publish 後の load を確認する。両 OS の展開済み SDK で compile し、ソースを削除して実行、model file を削除して compact replay する。繰り返す tape / gradient の保持と GC / numeric-page profile は別途測定し、数値結果だけで memory 改善を推測しない。

残る GUI / DB / 通信統合、HTTP server / TCP、長い kernel の分割・キャンセルと v2.0 の到達条件は継続する。
