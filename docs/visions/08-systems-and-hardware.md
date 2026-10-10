# OS・hardware・storage・実行基盤

arena/FFI/SIMD/JIT等の候補を土台にした自由構想です。REWINDの外で起きたことを復元する能力と、外部資源を扱う能力は分けて考えます。

## VSYS-01：他applicationへ埋め込むVM

C/Rust等からVMを作り、typed引数で関数を呼び、結果・診断・記録を受け取る。複数VMの隔離、SDK選択、資源の解放を管理できるembedding APIが欲しい。

## VSYS-02：Wasm等のsandbox extension

追加機能を隔離したmoduleとして読み込む。公開interface、容量、effect、versionを決め、application全体と同じnative権限をpluginへ渡さずに拡張したい。

## VSYS-03：非同期file pipeline

大きなfileをchunkで読み、変換し、書き出す処理を組む。read/write/flush/durabilityを別に扱い、完了の意味と途中cancelの結果が分かるAPIにしたい。

## VSYS-04：filesystem eventの観測

file変更やdirectory更新をeventとして受ける。coalescing、rename、欠落を扱い、record内の通知と現在のfilesystem状態を照合できるようにしたい。

## VSYS-05：archiveとbundleのstream処理

zip/tar等を全件展開せずに読む・作る。metadata、path、checksumを扱い、必要なentryだけを取り出してdata pipelineへ渡したい。

## VSYS-06：object storageと大容量blob

S3等のobject、multipart upload、range read、version IDを扱う。送信済みpartとVM内bufferを分け、再開・照合・cleanupを状態として見たい。

## VSYS-07：共有memoryとIPC

別processと型付きmessageやbufferを交換する。physical共有領域の変更は観測/外部操作として扱い、VM内snapshotへ取り込む境界を明示したい。

## VSYS-08：serial・USB・sensor接続

deviceから入力を読み、commandを送れる。simulation用deviceと実deviceを同じtyped interfaceへ接続し、送信済みcommandをrevertで取消したふりをしない設計にしたい。

## VSYS-09：GPU computeとdevice array

device上の配列、kernel、転送、同期を扱える。hostのcheckpointで何を保持しているか、device memoryをいつ解放できるかをprofileへ表示したい。

## VSYS-10：CPU/deviceの計算配置

同じoperationをCPU/GPU等へ配置し、転送量や起動費用を含めて比較する。自動選択の理由を見られ、再現性が必要なら固定policyを選びたい。

## VSYS-11：memory階層とspill

巨大なimmutable datasetや履歴を、RAM・圧縮領域・local storageへ明示policyで配置する。logicalな保持量と実RAM消費を区別し、必要なpageだけを読みたい。

## VSYS-12：checkpointの圧縮と重複排除

共有だけでは削減できない履歴を、圧縮・content deduplicationで保持する。復元費用と節約量を確認し、保持中のcheckpointを勝手に削除しない方式にしたい。

## VSYS-13：out-of-core数値処理

全配列がRAMに入らなくても、block単位のlinear algebraや集計を行う。disk I/Oとnative workを合わせてplan表示し、途中結果の保存と再開を選びたい。

## VSYS-14：再現性を優先する実行mode

worker数、reduction順、kernel選択等を固定するprofileを選べる。速さを優先するmodeとの差を明示し、同じ入力でどこまで再現できるかを説明したい。

## VSYS-15：cross-targetのartifact作成

開発machineと異なるOS/architecture用のbundleを作れる。portable codeとplatform依存adapterを分け、必要なABI/libraryと未検証部分を一覧で見たい。

## VSYS-16：実行中のcode更新とstate移行

長い処理を止めずに、互換な関数やcomponentを新versionへ切り替える。旧call frame・checkpoint・native resourceをどう扱うかを表示し、移行不能なstateは明示的に残したい。
