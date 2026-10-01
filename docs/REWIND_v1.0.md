# REWIND 1.0仕様

2026-10-01。compiler/language/stdは **1.0.0**。初回安定版の対象はLinux x86_64のCLI、署名付きSDK、以下の言語・ライブラリ・実行境界。ネットワークやGUIを含む全用途の完成という意味ではない。[導入](getting-started.md)、[言語とSDK](sdk-guide.md)、[標準ライブラリ](../libraries/README.md)を参照する。

## 実行と確定

`rewind run main.rw`、`rewind compile main.rw`、`rewind main.rwc`、`rewindc main.rw`を提供する。単一ファイルはmanifest不要でstdをimportできる。projectではmanifest/lock/effectsを優先する。

commitは計算状態とpending I/Oのcheckpoint。publishはその時点のpending I/Oを確定し、実行を継続する。確定した出力やfile変更はrevertで取り消さず、再送しない。後からの変更は次のpublishまでpending。virtual publish/replayは仮想Host baselineを更新し、実fileを書き換えない。

確定operation ledgerは現在のpending state、checkpoint、生存する外部branch anchorが参照するIDだけを保持する。anchorはcloneも登録された同じrootを保持し、最後のcloneが解放されるまで回収対象から除く。保持されたcheckpointはheap・journal・観測に対するretentionを生む。dropとanchor解放後のpublishで不要なledgerを回収する。

byte入力は要求サイズと実際の観測を保存し、revert/replayで再利用する。観測payloadはspill可能。EOFはstickyで、失敗後にも読み取った観測を捨ててHostを再読しない。復元時に要求サイズを変えるとReplayMismatch。観測はrecordの再現に必要なため実行中に全て捨てる設計ではなく、有限のhistory storageを使う。観測exportはbyte payload 16 MiB、完成したtraceは128 MiBまで。無制限のstream/recordは提供しない。

## 型とmoduleの境界

struct/recordのfieldは既存どおり公開が既定。`private field:Type`は定義module内だけで読書き・pattern分解できる。private fieldを持つ公開型は外部から直接constructorを呼べず、公開factoryを使用する。Frozen経由も同じ可視性を検査する。stdのcollection/parserの内部fieldをprivate化し、必要な長さ・次元等は関数で取得する。Forest等の結果データの公開fieldはそのまま利用できる。

```rewind
pub struct Bag { private items:List<Int> }
pub fn make()->Bag effects {} { return Bag(List<Int>()); }
fn compare<T>(a:T,b:T)->Int effects {} where T:Ord+Share { return a.cmp(b); }
```

generic boundは`T:Ord+Share`のように複数指定できる。functionのwhereはreturn/effects宣言の後、structのwhereは型parameterの後に置く。全boundを満たす必要があり、複数traitの同名methodは曖昧として拒否する。既存Share/Sendの所有権・capture検査を維持する。

型注釈のあるbindingとfunctionのreturnでは、期待する返却型から全parameterが決定するfactory callを明示的generic callへ展開する。例えば`let xs:List<Int>=empty();`。引数と期待型が矛盾すれば拒否する。曖昧なparameter、部分的な推論、任意の代入やcallback引数からの全面的な期待型伝播は提供しない。展開結果をartifactに保存するのでsource-free実行にも同じ型を使う。

## 予算と回収

VMは有限のexecution/native/task/history/scheduler予算を使う。`--steps N`は1.0では補助EngineとGCを含む累積Host ceilingで、source内のruntime宣言やrevertで増やせない。指定しない場合は既存の初期execution予算1,000,000とsource内宣言を使用する。`--native-work N`の既定は1,000,000。native呼出し・adapter・JSON/codec・String method/結合・collection処理・GCを論理workとして課金する。payload長やtree pathを使う保守的な単位であり、CPU時間の測定ではない。Host filesystem検証・allocator・compiler/cacheの費用を同じ精度のbyte係数で表すものでもない。

history memoryは1.0で値・空のcollection slot・persistent node・checkpoint等のallocation admissionも含めて検査する。shared rootの同一性で重複を除くが、共有subtreeの正確なresident byte計測ではなく保守的な費用。既定history memoryは512 MiB、storageは8 GiB、spill thresholdは8 MiB。VM一時値やcompilerを含むプロセス全体はLinuxのaddress-space上限で別に制限する。既定2,048 MiB、`--memory-mib N`で64〜1,048,576 MiBを指定でき、継承された上限を引き上げない。OS/allocatorによる終了はrecoverable Resultや言語のDiagnosticにはならず、pending処理の終了保証が対象。RSSの正確な計測・全platform allocator共通の復旧は提供しない。

compilerはmodule source 1 MiB、module集合256、syntaxの再帰64、ASTの深さ128を制限する。型展開・monomorphization・依存解決にも既存予算を適用する。これは許容する構文・sourceの範囲であり、無制限の再帰ではない。

GCはinstructionのsafe pointで動作し、globals、frame、closure、defer、branch、taskの引数/context/result、channel queueをrootに含める。checkpoint/外部anchorは独立したheap rootを保持する。markの子要素展開前に残workを検査し、quota不足ではsweepしない。native callbackは同期VM/Engineの予算内で実行し、その途中で外部から非同期GCを実行しない。embeddingのcollect_heapではcallerが外部live Valueを明示する。

## エラーとライブラリの失敗

fatal Diagnosticはcode、source/line/column、task、cleanup cause treeを保持する。NativeWork/Compilerの予算失敗もTaskError::BudgetExceededへ分類する。std.error.Errorはapplication用の不変envelopeで、FileError/JsonError/StdError/Diagnosticからcode/location/offset/causeを移す。Diagnosticの全cause treeは元の値に保持し、envelopeは最初のcause messageを投影する。既存のResult<...,String>を勝手に全て変更せず、必要な境界でerror.code等へ変換する。

std.sort.tryStableはfallible comparator、std.segment.tryUpdatedはfallible monoidを受け取る。借用した入力を変更せず、新しい結果を返す。最初のcallback Errを返し、range/capacity/budget失敗でも入力は保持する。予算超過はfatalであり、任意のin-place関数全体をtransactionとして復旧する保証はない。pending file/directory/handle mutationの失敗は従来どおりdelta・位置・操作IDを復元する。Host観測や消費した予算は戻さない。

公開API snapshotは公開fieldとconstructor可視性、generic bounds、effects、所有権契約を含める。`// @api functionName cost 説明`と`// @api functionName failure 説明`を追加でき、変更はapi-diff --deny-breakingで保守的にbreakingとして検出する。private fieldだけの変更は検出対象にしない。free text契約を自動証明する機能ではない。

## publish途中の失敗

競合/権限/path等の事前検証と、directory-create、parent-create、stage-open/write/sync、rename、file/directory-delete、stdout/stderr/flushの適用段階を区別する。適用開始後の失敗はPublishPartiallyApplied。Runtime.publish_failure / Error.publish_report、およびJSON診断のpublish_failureでphase/path/applied/cause/retryable=falseを取得できる。appliedは成功が確認された操作。失敗したHost呼出し自体の部分効果もあり得るので、そのpathが無変更だったと解釈しない。staging tempは所有範囲でcleanupする。

この失敗はcheckpoint外に保持し、revert後にも再publishを拒否する。replayのstream short-writeも同じ扱い。複数file・directory・streamの一括atomic性、power-loss/crash recovery、他processと競合しないpath解決は保証しない。外部状態を確認して別runtimeで判断する。

## 配布と互換性

Rust 1.98.1、Cargo.lock、compiler/language/std/lock 1.0.0を固定する。Linux x86_64 SDKはrewind/rewindc、REWIND source library 34 module、公開API baseline、文書、例を含む。checksum・Ed25519署名・archive抽出・input/record/replay・source-free artifactを検証して配布する。最低glibcは配布のBUILD_INFO.jsonを参照する。

署名鍵はrelease単位。公開鍵を信頼する配布経路で確認し、次の版の鍵を以前の鍵と自動的に同一視しない。外部SDK導入は信頼する鍵を明示する。1.0で永続的な公式鍵の自動rotation/revocation protocolは提供しない。stdの既存namespaceを自動上書きするupgrade/rollbackも提供せず、新しいproject copyで検証してmanifest/lock/vendorをまとめて切り替える。

0.xからはprivate化されたstd constructor/fieldをfactory・accessorへ移す。languageを1.0.0へ変更しupdate、check/test、artifact/recordの再生成を行う。source-free/replayのcompiler内部形式やRust embedding ABIの版跨ぎ互換性は保証しない。1.0の公開source API baselineを以後の互換性判断の基準とする。

## 継続開発の範囲

lazy segment、suffix array/trie、flow/matching、NTT/geometry、日時/decimal、incremental CSV/JSON、Unicode grapheme/regex、network/process/DB/UI、durable store/crash recovery、Linux ARM/macOS/Windowsの配布・試験は後続。既存のgraph、KMP/Z、range/rollback DSU、matrix、DP等を再実装項目にはしない。作業はcodex/developで継続し、版branchは増やさない。
