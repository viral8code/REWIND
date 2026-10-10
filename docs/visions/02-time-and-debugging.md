# 時間・分岐・デバッグの新しい体験

REWINDの特徴を、命令を前後に動かすだけでなく「理由を調べる」「別の可能性を比べる」道具へ広げる自由構想です。

## VTIME-01：値が変わった理由を尋ねる

変数を選ぶと、その値を作った代入、条件分岐、入力、呼出しを遡って見られる。「最後に書いた行」だけでなく、値に寄与した計算のつながりを表示したい。

## VTIME-02：実行されなかった理由を尋ねる

buttonが有効にならない、分岐へ入らない、taskが起動しない理由を、条件値と依存関係から説明する。実行されなかった行を選ぶことで、どこで可能性が消えたかを調べたい。

## VTIME-03：条件付きの過去検索

「初めて残高が負になった」「このrecordが追加された」「taskが待機したままになった」等を条件で検索する。現在からstepを何千回も戻す代わりに、該当eventへ移動したい。

## VTIME-04：因果関係のgraph

task、観測、message、checkpoint、publishをgraphでつなぐ。wall timeで近いというだけで原因扱いせず、awaitやデータ依存による関係を見たい。

## VTIME-05：二つの実行を差分表示する

修正前後、異なる入力、異なるscheduler選択を並べて、最初に状態が分かれたeventへ移動する。結果が同じでも仕事量や保持量が増えた箇所を比較したい。

## VTIME-06：過去から仮説を分岐させる

記録の途中で入力値やpureな処理を変え、別の計算結果を試せる。仮説の実行は外部送信を停止したsandboxとして扱い、実際に公開した世界との違いを明示したい。

## VTIME-07：branchの三者比較とmerge支援

共通checkpoint、branch A、branch Bのstateを比較し、変更の衝突を確認できる。VM内の値には利用者のmerge関数を使い、確定済み外部作用はmergeの対象と別に扱いたい。

## VTIME-08：checkpointへ説明を添える

checkpointに名前だけでなく、作成理由、関連record、入力event、UIのthumbnailを添えられる。Undo一覧が「checkpoint 42」ではなく「数量を変更」のように読めると使いやすい。

## VTIME-09：時間軸のmemory地図

どのcheckpointからどのpageへ参照が残るかを、時間軸と共有graphで見る。履歴を削除した場合に何が他のrootへ残るかを、説明付きで調べたい。

## VTIME-10：公開する作用のpreview

publish前に、出力、file変更、GUI差分を操作一覧として確認できる。previewは対応するpending操作の説明であり、SQL/通信を試しに実行して結果を見る仕組みとは分ける。

## VTIME-11：値の変更を追うwatchpoint

特定field、row ID、collectionの条件が変わったeventで止まる。任意callbackを実行するwatchではなく、記録内の変更を検索して、書込み場所と旧新値を示したい。

## VTIME-12：failureから再現例を切り出す

失敗に必要だったsource、非機密入力、scheduler選択だけをまとめる。巨大な記録全体を共有せず、原因を保った小さなreproducerを作りたい。

## VTIME-13：task待機を時間軸でほどく

待機先、開始契機、取消要求、physical完了、cleanup完了を別に表示する。「Taskが終了した」と「外部資源が解放された」の時間差が分かる画面にしたい。

## VTIME-14：stateに対する時間query

「checkpoint間で追加された要素」「直近10回で変わったfield」「値の最大変化」を記録へqueryできる。sourceのロジックを書き換えず、調査用のpure queryで答えを得たい。

## VTIME-15：実行の説明を持つtest report

testの期待値だけでなく、外れた値を作った経路をreportに残す。失敗event、因果graph、関連sourceへ一つのreportから移動できるとレビューにも使いやすい。

## VTIME-16：操作履歴をそのまま教材にする

実行を前後に見ながら、Listの共有、所有権移動、revert、publishを可視化する。記録に説明文を添え、同じ教材をofflineで再生して言語の考え方を学びたい。
