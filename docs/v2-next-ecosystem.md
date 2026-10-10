# 次版構想：配布・開発品質・標準ライブラリの継続運用

状態：採用候補、未実装。版番号は未割当です。[全体計画](ROADMAP_v2-next.md)を参照してください。大型project管理は引き続き後順位です。

2.0にはcompiler/VM、SDK、署名付きlibrary、project/lock、fmt/doc/LSP、test、record/replay、Windows/Linux配布、HTML/PDF入門書があります。以下は利用者が更新・配布・調査を続けやすくする差分です。新しい構想は公開CI/CDを今動かす指示ではありません。

## DIST-01：SDKと実行環境の用途別配布

開発用SDKと、compile済みapplicationを実行する最小runtimeの分離を検討します。まずartifactが実行時に必要とする情報・library・native依存を調べ、source-free動作の現在の保証を維持します。

候補はapplication artifact、runtime、設定例、依存license、checksum、互換性manifestをまとめたoffline bundleです。runtimeを使い回す方式と同梱する方式のサイズ・更新責任を比較します。起動時にnetworkから依存を取得する仕様にはしません。

**受入：** compilerなしのclean環境で起動できる。必要なnative依存と対応OS/archが一覧で分かる。欠損・改変・非互換を起動前に診断する。

## DIST-02：Windows配布の確認範囲

現在のWindows版を土台に、空白/Unicode path、長いpath、drive、console encoding、GUI起動、TLS証明書設定、DB依存の配置を確認するmatrixを作ります。

zip展開だけで使える版を維持する案を優先し、installer、file関連付け、署名は必要性が確認できた場合の別単位です。管理者権限を前提にしません。GUIのconsole表示方針は、CLI用途と分けて設計します。

**受入：** source/artifact、native GUI、HTTP/DBの実行例が同じ配布物で使える。fixtureのみの確認と実Windows確認を区別する。確認できていない構成を対応済みとしない。

## DIST-03：互換性matrixと変更検出

language、artifact、trace、SDK signature、model format、native依存のversionを別軸で管理します。version番号だけで「全互換」と判断しません。

候補は公開API差分の自動報告です。field/type/effects/generic bounds/error code/default/parameter名を分類し、互換性判断は人が確認します。source修正が必要な変更と、artifact再compileが必要な変更を区別します。

**受入：** 意図したAPI変更が報告される。旧artifact/traceの既知fixtureを読むか、明示的に拒否する。古い形式を黙って現行形式へ読み替えない。

## QUAL-01：property testと縮小する失敗例

assert/testは既存です。入力generator、seed、case数、縮小を明示するpureなproperty helperを候補にします。

初期は整数、Bytes、短いListとoperation列に絞ります。縮小自体のsteps/native work/時間を制限します。VM状態やphysical入力を勝手にresetできるとは仮定せず、外部操作を含むpropertyは専用fixtureを使います。

**受入：** seedと最小失敗入力を保存して再現できる。List/Mapの操作列を単純な参照実装と比較できる。checkpoint/borrowの不正を生成した場合も意図した拒否とruntime不具合を区別する。

## QUAL-02：観測fixtureと失敗注入

HTTP/DB/GUIのfixtureは既存の仕組みを再利用します。送信前失敗、部分応答、結果不明、cleanup失敗を利用者のapplication試験へ提供する候補です。

fixture APIはphysical backendと型・error contractを揃え、現実の失敗を模した範囲を記載します。native接続を成功したふりで生成する公開APIにはしません。recordにcredentialを入れず、fixture dataにも機密値を保存しません。

**受入：** SVC-02/DB-02/STATE-02の分岐を試験できる。fixtureから実networkへfallbackしない。利用者が本番設定と取り違えにくい明示選択にする。

## QUAL-03：coverageとtest failureの調査

line/function/branchの実行情報を既存profile/traceから取得できるか調べます。可能ならtest reportへまとめ、coverageのためにdebug記録全体を常時生成しません。

generic特殊化、macro等を将来追加した場合のsource対応、generated code、短絡演算、match guard、取消で中断したcaseを扱います。coverage百分率を正しさの保証として扱いません。DAPへ渡す失敗記録はbounded・redactedにします。

**受入：** 手で数えられる小さなsourceの実行位置と一致する。無効時の費用を増やさない。CLI/JSON/HTMLの失敗codeと位置が一致する。

## EXT-01：native library拡張の契約

汎用FFIは後順位の調査候補です。まず追加codec/solver等を組込みadapterとして提供する範囲を明確にします。任意pointerやREWIND callbackを外部threadへ渡す仕組みを先に公開しません。

必要な契約は値のmarshal、affine handle、所有権移動、shared bufferの寿命、work/memory計上、cancel、cleanup、panic/例外境界、thread利用、ABI versionです。adapterはpure、観測、不可逆physical操作のどれかを宣言します。pureと称するlibraryのglobal stateや外部アクセスも調べます。

**受入：** 一つのcodec等を契約に沿って統合できる。VMへ不正pointerを持ち込めず、revertで解放済みhandleが復活しない。契約が未成立なら動的FFIは提供しない。

## DOC-01：実装とreferenceの継続照合

現行bookのAPI取込みと実行例検証を再利用します。新APIにeffect、所有権、単位、上限、失敗後state、費用、最初に使えるlanguageを必須化する案です。

syntax説明とAPI説明の重複を減らし、候補APIを現行referenceへ混ぜません。error codeから原因・関連位置・例・移行先へ辿れる索引を候補にします。HTMLはoffline検索・keyboard操作、PDFはTOC/リンクを維持します。

**受入：** 実装signatureと説明が一致する。例は期待結果と失敗条件を持つ。OS固有部分とfixture例に確認範囲が書かれている。

## 任意の後続候補

- macOS/ARM：まずnative GUI、TLS、DB、配布依存を一覧化し、VMが動くだけで全対応としない。
- localization：Unicode処理は既存。message catalogとnumber/date表示を、計算値のserializationと分けて検討する。
- accessibility：既存backendを土台に、component/layout/table追加時のrole、focus、keyboard、読み上げ情報を確認する。
- library catalog：検索可能なAPI一覧と用途別の例を先行し、registry、依存solver、巨大build systemは後順位を維持する。

## 採否の順序

DIST-03 / DOC-01 → QUAL-01/02 → DIST-01/02 → QUAL-03 → EXT-01。

native拡張と新OSは費用が大きいため、具体的な不足と配布責任を確認してから別案にします。公開物を作成・統合する時点では最新のユーザー指示を再確認します。現在はcodex/developへの計画保存だけを行います。
