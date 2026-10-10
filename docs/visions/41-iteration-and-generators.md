# iterator・generator・列挙の体験

既存collectionやstreamの発展案。同期的な列挙、失敗する列挙、中断可能な計算を、値の寿命と版の契約まで含めて使いやすくしたい。

## VITER-01：列挙の能力を表すtrait

一方向、両方向、長さ既知、random access等を能力として表す。algorithmは必要な能力だけを要求できる。

exactな残り長と推定size hintを区別する。能力を持たないiteratorにも利用できる代替algorithmをlibraryが提供したい。

## VITER-02：失敗を失わないiterator

elementとerrorを返すfallible iteratorで、変換のerrorを明示的に伝播する。collectは完成した値、部分結果、失敗位置を返す方式を選べる。

errorを単にendとして扱わない。部分結果を採用するpolicyと、作成中の値を破棄するpolicyをownerに合わせて選ぶ。

## VITER-03：borrowした要素を順に渡す

一時viewを次のstepまで貸すlending iteratorを考える。再利用buffer、tree cursor、native chunkを無条件にcopyせず処理する。

stepを進めた後へ借用を逃がせないcontractを使う。所有値が必要な時は、captureまたはcopyの費用を明示する。

## VITER-04：版に固定した列挙

永続collectionの特定rootへiteratorを固定する。現在のcollectionを更新しても、列挙中の版は変わらない。

変更を追うlive iteratorは別contractにする。古いrootを保持するiteratorのmemory費用も確認できると便利。

## VITER-05：安全なdrainとextract

条件に合う要素をownerとして取り出す操作。途中でerrorやcancelが起きた場合の、残りcollectionと取得済みの値を定義する。

checkpointが旧版を保持している場合、shared dataとphysical ownerの違いを検査する。affineな外部handleを過去版から重複取得しない。

## VITER-06：無限列同士の公平な積

無限iteratorの組合せで、外側の最初の要素だけを永遠に調べることを避けたい。対角順等のfairな列挙をstrategyとして選ぶ。

有限のlexicographic順序とは異なる結果順を明示する。探索で使う場合、buffer・budget・途中のbookmarkを表せるようにする。

## VITER-07：typed generatorの返値

yieldする型と、最終的にreturnする型を別に持つgenerator。要素列と、集計や終了理由を一緒に返したい。

所有権を持つ返値は一度だけ取り出す。普通のiteratorへ変換した時に最終値を捨てるか、別のownerへ残すかを選ぶ。

## VITER-08：双方向のgenerator入力

resume時に値を渡すgeneratorを考える。parser、対話計算、stepごとのsimulationを型付きで構成できる。

sendする型、yield型、return型をsignatureへ表す。再開していないgeneratorへ同時に複数入力する操作はownerの契約で防ぐ。

## VITER-09：generator dataのcheckpoint

純粋なcaptureだけを持つgeneratorの進行stateをdataとして保存し、再計算や分岐へ使う。

一般のstackful continuationやnative frameも保存できるという意味ではない。保存可能な変換方式と、複製できないownerを区別する。

## VITER-10：短絡処理後のcursor

find、takeWhile、tryFold等が終了した後、cursorがどこを指すかを明確にする。検査したが消費しなかった要素を扱いたい。

peek bufferのownerと、physical inputを読んだ事実は分ける。streamを戻したい場合は明示的なtapeに記録する。

## VITER-11：融合とchunk処理の説明

map/filter/foldを一つのloopへ融合し、型付きchunkやSIMDを使う候補をplanとして見たい。

callbackの評価回数、error位置、短絡の順序を保つ。effectのあるcallbackを順序変更可能な純粋計算と同じ扱いにしない。

## VITER-12：並行列挙と順序policy

独立したelementを並列に変換し、入力順、完了順、key順等で返す。順序保持のbuffer費用も利用者が選べる。

落ちたelementとcancelされたelementを区別し、iteratorの終了に未完了taskを残さないstructured contractを使いたい。
