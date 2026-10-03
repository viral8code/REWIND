# REWIND v1.9.0 — 言語監査と GC の割当圧力

v1.9 の最初の公開単位。GC・性能・公平性・GUI・アルゴリズムの全工程を完了した版ではない。後続 patch で [到達条件](ROADMAP_v2.md) の検証を続ける。

## match

1.9 以降の網羅性・到達可能性検査は、入れ子の Option / Result、generic enum、tuple、record のフィールドを組み合わせて判定する。例えば `Result<Option<T>,E>` は `Err(_)`、`Ok(None)`、`Ok(Some(_))` で網羅できる。Bool の組合せや Int の半開区間も検査する。guard がある arm は網羅性の根拠にしない。先行する guard のない arm に完全に含まれる arm と空・逆順の区間は拒否する。List は固定長 pattern と wildcard を扱う。

解析には work 250,000 / 再帰深さ 256 の上限を置き、超過を `PatternAnalysisBudgetExceeded` として報告する。再帰的な型や空の型を含む全ての型の inhabitation を証明する仕様ではない。旧 artifact の言語版は旧網羅性規則で検査する。

native の opaque 型は record pattern で内部を分解できない。標準 factory / accessor を使用する。不正 artifact の未対応 literal pattern も compile 時に拒否する。

## import のスコープ

module 内の名前変換を宣言位置と lexical scope に従って行う。後の match arm、block、loop、closure の同名束縛は、それ以前・外側の関数名を変えない。`let source=source();` の initializer は外側の関数を参照する。module 所有のトップレベル変数も module ごとに分離する。

## GC

従来の 256 heap 割当の条件に、前回の成功した collection 以降の値 payload の割当・正の増加が 4 MiB に達する条件を追加する。判定用 byte 数は保守的な payload 費用で、物理 RSS の測定や共有 buffer の実割当量ではない。少数の大きい Text / Bytes / native array の割当でも次の VM safe point で collection を試みる。heap_set が失敗した場合は値と圧力計数を変更しない。

collection は VM の global、scope、frame、cleanup、branch、scheduler の根を保つ。checkpoint は独立した persistent heap を保持し、current heap からの削除で過去の値を失わない。work 上限、admission budget と埋込み API の外部 root 指定は継続する。この変更は最大 RAM、観測 journal の解放、巨大な単一割当の自動回収を保証しない。

## 検証と後続

nested coverage / unreachable / guard / interval / record / list、import scope、opaque 型、source-free record / replay、大きい割当・更新と checkpoint / budget 失敗を検証する。標準 API の互換性、外部 DB / HTTP / GUI、署名付き SDK は各 OS の release 検証に含める。

次の監査は観測・trace の bounded spill、file reader、checkpoint に含まれる不要 storage、共有 buffer の accounting、native work と UI / task の公平性、GUI の不足、アルゴリズムを対象にする。既存 byte observation spill と接続資源回収を再利用し、実装済み部分を作り直さない。
