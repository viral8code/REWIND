# REWIND v1.9.65 — 診断と資源回収の最終整理

この版は子taskの原因が隠れる経路、native資源回収の重複走査、
OS入力とv2到達条件の監査をまとめて扱う。公開は両OSの全回帰・展開後SDK・
実サービスの受入を条件とし、個別の修正だけでは公開しない。

## taskの失敗

`std.taskError.describe` は元の診断と原因ツリー、型付きbudget、deadlockの
wait graphを所有するenvelopeとして保持する。`asStd` は数値系API向けに
診断codeと行番号を投影する。後者は全文・source・原因ツリーを保持しない。
タイムアウト、取消、Channelのclose、native work超過を区別し、retryは行わない。

自動微分のforward / backwardと数値identityの協調ラッパーでは、従来の
`AutodiffTask` / `NumericTask` の汎用名へ置き換える経路を改める。
通常の数値domain・shapeエラーと、子taskのbudget超過を区別できる。
関数の引数・返却型は維持するが、子task失敗時の `StdError.code` は変わる。
呼出側は古い汎用codeだけを前提にせず、具体的なbudget/診断codeを確認する。

## 資源回収

HTTP server、TCP、DB、HTTP streamが同じVMのlease rootsを使うため、
回収時のreachability走査を1回へまとめる。閉じた資源を復元し直したり、
checkpointだけにある古いtokenで接続を保持したりする変更ではない。
性能差とnative資源の寿命は、同時利用・取消・revert・サービスなしreplayで確認する。

## 大きなコレクションのスナップショット

`freeze` と `iter()` に流用されていたMapキー用の1万ノード上限を、
この版以降の通常のコレクション操作から外す。既存のコンパイル済みartifactと
Mapキーの複合キー上限は維持する。通常のスナップショットはnative workと
history memoryの予算で制限し、深さ64、循環、関数・資源の捕捉は引き続き拒否する。

まず全内容を検証し、コピーと一時領域の費用を予算に照合した後に結果を作る。
VMのheap edgeを持たないページ化List / Mapは不変のページを共有し、元の
コレクションの変更によってスナップショットの内容が変わらないようCOWを使う。
Listのiteratorも検証済みのページを保持する。Mapのキーや文字列の文字列挙は
新しい列挙値が必要なため、別途その一時領域と結果の予算を確認する。
検証は内容に比例する処理量を要し、上限の撤廃を無制限の処理とはしない。

## 公開までの受入

- 65,536要素のList・1万超のMapのsnapshot、元データの変更からの独立、cursorのrevert、予算不足、旧artifactの上限維持。
- 実際の失敗taskのsource/行/codeと原因保持、型付きbudget、source-freeの記録とreplay。
- 数値ラッパーで子taskのnative work上限を超えた場合の具体的なcode、正常系との区別。
- 元のlease・移動・取消・閉じた資源・checkpointと新しいreachability走査の一致。
- 最適化した同時処理のCPU / wall / RSS / GC費用、既存SDKとAPIの照合。
- Windowsの実変換サービスの受入を含むOS入力の残件と、v2の到達条件1〜7の監査。

未実装・未検証の必須項目がある間はv2.0の完了とは扱わない。

## 検証記録

実際にpanicさせたtaskのcode・message・source・行を取り出し、ソース削除後の
debug / compact実行とreplayで維持することを確認した。非同期forwardでは
1 millionのnative work上限で具体的なbudget code、3 millionでは正常終了を
確認した。元のlease検査2件・HTTP寿命と取消の12件・標準ライブラリ契約も通過した。
Linux候補の全650 integration回帰、unit / doc、標準ライブラリと展開後SDKも通過した。
公開CIでは実Microsoft日本語IMEによる未確定文字列、変換、確定の8回反復、
focus移動時の取消、window closeを確認した。句変換モードを試験側でゼロへ
固定すると、Windows標準EDITでも一音ずつ確定してしまうため、サービスの
既定モードを保持する。Unicode確定入力やmockで実変換の受入を代用していない。
最終的な両OSの全回帰・展開後SDKと公開はRelease workflowの成功を条件とする。

サービスの途中停止では、応答を返さずに接続が閉じられることを検査する。
EOF、connection reset、Windowsのconnection abortを切断として扱い、
HTTP応答やタイムアウトは成功として扱わない。失敗時は例外の型とOS error codeを記録する。

HTTP server / TCP / SQLite / HTTP downloadを同時保持する最適化したRuntime
probeでは、所有者存続中4資源、解放後0、checkpoint復元後も0を確認した。
同じ入力の逐次3回測定では、100回の回収区間が16,384 / 65,536 / 262,144要素で
0.06614 / 0.31397 / 3.52269秒から0.01647 / 0.08090 / 0.89663秒へ短縮した。
この区間の結果をアプリ全体やGC全体の速度へ広げない。協調実行の追加費用も維持する。

Windowsの実変換受入は、日本語IMEをCI用VMへ準備し、nativeのcomposition設定・
変換・確定・focus変更・window closeを8回の反復とともに検査する。設定文字列を
書き換えるmockは使わず、変換結果が未変換文字列と異なることも検査する。
CI以外のVM実行やSDK起動がOSの言語設定を変更するものではない。
実サービスがない場合の型検査や通常のIMM context試験だけで完了にはしない。

Windowsのnative windowはシステムが関連付けたIMM contextを借用し、必要時に
自前のcontextを用意する。所有権を区別して借用contextを破棄しない。
公開した入力欄のcharacter-position要求へ回答し、COM / text-service managerの
活性化と終了を対応させる。入力欄が変わった時の未確定文字列の取消と、
確定文字だけがVMへ入る契約は維持する。CIの言語サービス準備は使い捨てVMに限定する。
