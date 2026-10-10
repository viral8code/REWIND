# 診断を値・履歴・説明として扱う

既存のcompiler/runtime診断の発展案。利用者が「どこで何が起きたか」から「何を確認し、何を変えればよいか」まで辿れる仕組みを考える。

## VDIAG-01：構造化されたDiagnostic値

message、source span、cause、関連symbol、修正候補、確認環境をDiagnosticとして返す。CLI、editor、HTML reportが同じdataを使う。

表示文をparseしてintegrationを作る必要を減らす。診断formatのversionと、machine向けIDの安定性をcontractにする。

## VDIAG-02：型の不一致を経路で説明

大きなrecordやgeneric型で、最初に違うfield、associated type、effectへ経路を示す。期待型と実際の型を同じ位置で比較したい。

型aliasの定義を無制限に展開せず、必要な場所だけ開く。簡略表示から完全な型へ辿れる体験を考える。

## VDIAG-03：未解決の制約を並べる

generic inferenceで何が不足しているかを、候補・矛盾・未確定として表示する。単に型を推論できないと返す以上の説明が欲しい。

solver内部の全logを見せるのではなく、利用者が注釈や引数を追加できる位置へ結び付ける。

## VDIAG-04：errorに付ける最小data view

境界errorへ、検証に必要なfieldだけのviewを添える。大きなobjectや機密値を丸ごとformatせず、失敗条件を読める。

表示用getterを勝手に実行しない。redaction、深さ、要素数、表示budgetを診断側のcontractにしたい。

## VDIAG-05：版の違いを持つstack frame

frameへcode版、model版、task ID、最適化情報を付ける。hot reload前のframeや、revert後に残ったerrorを誤ったsourceへ結び付けない。

sourceがないartifactでもsymbolと位置を保持できるformatを考える。source参照の保存と、source全文の公開は別設定にする。

## VDIAG-06：仮説を含む修正候補

fixを適用できる条件、変わる型・作用、確認した範囲を候補へ持たせる。利用者はpreviewと差分を見て採用する。

単なる文字列置換と、意味を確認したrefactoringを区別する。検証なしの候補を正しい修正と断定しない。

## VDIAG-07：rollbackしても残る失敗説明

attemptでmodelを戻した後も、失敗した入力版、選択肢、診断を明示ownerへ残せるようにする。

すべての失敗snapshotを自動保持するのではなく、必要な要約や選択dataを保存するpolicyを使う。診断のためのmemory費用も見たい。

## VDIAG-08：memoryを保持するrootの説明

値が解放されない理由をcheckpoint、snapshot catalog、closure、task、cache等のrootへ辿る。

利用者が保持した履歴と、解放漏れの疑いを区別して表示する。rootをdropする操作では、失われる履歴と他の共有ownerもpreviewしたい。

## VDIAG-09：pending作用と物理結果の対応

未publishの出力、実行待ちIntent、実行済みreceipt、結果不明operationを同じ診断画面で区別する。

値をrevertしても残る外部結果がある場合、その操作IDを表示する。errorを表示した事実と、programのOut bufferも別の観測として扱う。

## VDIAG-10：budget終了の内訳

実行step、native work、allocation、trace、cache等の費用を分けて、どの上限に到達したか説明する。

logicalな仕事量と物理時間の測定差を示す。再現可能な上限と、その環境で実測した上限を混同しない。

## VDIAG-11：部分的なreproducer bundle

source/artifact版、選択したsnapshot、入力tape、必要なruntime条件を束ねる。過去の失敗を小さく共有したい。

export viewで不要な機密や権限を除き、残した条件だけで再現できるかを検証する。外部接続能力をbundleへ自動移送しない。

## VDIAG-12：診断の品質を検証するfixture

同じ失敗に対して、正しいspan、説明ID、必要な関連位置、修正候補の条件が出るか確認する。

日本語・英語の文章全体を固定するtestへ依存せず、構造と利用者が必要とする情報を検査する。機密値が漏れないことも診断contractとして確認したい。
