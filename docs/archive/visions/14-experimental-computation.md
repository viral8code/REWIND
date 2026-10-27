# 実験的な言語・計算モデル

実装の容易さでは選ばない自由構想です。REWINDの現在の設計へそのまま足せるとは限らず、別のexperimental modeや研究用libraryという形も想定します。

## VEXP-01：higher-kindedな抽象化

値型だけでなく、ListやResult等の型constructorをparameterとして扱う。map/traverse等の共通処理を、具体的なcontainerごとに繰返し書かずに済むと嬉しい。

## VEXP-02：証明を持つ値

「この列はsorted」「このdivisionはゼロ除算しない」等の根拠を型として渡す。実行時の検査から証明を作る方法と、compile時に示す方法をつなぎたい。

## VEXP-03：session type

通信の「送る→受ける→終了」という順序を型へ表す。protocolの次に許す操作が補完され、状態違反をcompile時に見つけたい。

## VEXP-04：lensと双方向変換

大きなmodelの部分viewを読んで更新し、変更を元modelへ戻す。GUI表示や設定editorで、読取りと書戻しが整合する変換を合成したい。

## VEXP-05：logic programming

関係と条件から値を探索する処理を、宣言的に書ける。通常の関数と組み合わせ、探索順、解の取得、cutoffを利用者が調べたい。

## VEXP-06：graph rewriting

式、workflow、scene等をgraphとして書換えるruleを定義する。適用箇所、衝突、停止条件を見ながら、変換過程をREWINDの履歴へ残したい。

## VEXP-07：multi-stage programming

既知のparameterから専用の処理を組み立て、後で残りの入力を計算する。生成codeをtypedな値として扱い、展開したsourceや効果を確認したい。

## VEXP-08：可逆な関数の契約

逆変換を持つ関数を定義し、往復の整合を検査する。情報を失う操作では追加dataを要求し、checkpoint保存による復元と数学的な逆変換を別の道具として使いたい。

## VEXP-09：quantum circuitのmodel

gate、measurement、noise、circuit変換をdataとして扱う。まずsimulatorで探索し、実deviceへ送る場合は別のphysical境界として接続したい。

## VEXP-10：複数の仮説を一度に計算する

configurationの違いを複数branchとして保持し、共通計算を共有する。どの条件で結果が変わるかを、全組合せを独立実行するより分かりやすく調べたい。

## VEXP-11：probabilistic programming

分布、観測、latent variableをmodelとして記述する。samplingや推論方式を選び、各結果の根拠・seed・診断を残したい。

## VEXP-12：制約に基づく修正候補

設定やmodelの不整合へ、満たすべき条件から修正候補を提案する。候補の理由と変えるfieldを表示し、利用者が選ぶまで外部操作や実data更新を行わない体験にしたい。

## VEXP-13：semantic code database

sourceの型、参照、effect、生成由来をqueryできる。text検索では答えにくい「この能力を使う公開API」「この型を返す全経路」を調べたい。

## VEXP-14：proof-carrying artifact

artifactへ型・効果・資源の契約に関する検証可能な情報を添える。loaderが必要な性質を確認でき、署名があることと意味が正しいことを別に評価したい。

## VEXP-15：宣言的なstate migration

旧stateから新stateへの対応を宣言し、保持中のcheckpointやlive componentを移行する。移行可能な値と再接続が必要なresourceを分け、各結果をpreviewしたい。

## VEXP-16：data layoutを選べる値

同じlogical record列を、AoS/SoA、packed、columnar等の物理配置へ変える。意味と公開APIを保ちながら、CPU/deviceへのアクセスに合うrepresentationを選びたい。
