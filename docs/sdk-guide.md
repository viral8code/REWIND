# REWIND SDK 1.7.1

REWIND 1.7.1、Linux x86_64 / Windows x64 のSDK。bin/rewind は compiler と runtime を兼ね、check/test/doc/build/run/debug/LSP を提供する。stdlib の公開APIは [std-api.json](std-api.json)、module ごとの文書は [text](std/text.md)、[number](std/number.md)、[collections](std/collections.md)、[json](std/json.md) などを参照。

## 単一ファイル

binをPATHへ追加し、`rewind run main.rw` / `rewind compile main.rw` / `rewind run main.rwc`を使う。`rewindc main.rw` / `rewind main.rwc`も利用できる。manifestなしのstdはcompilerに内蔵され、sdk-install不要。最初の手順は[入門](getting-started.md)。

## 導入

配布者の公開鍵を別経路で確認する。SDK 内に秘密鍵はない。

```sh
/path/to/sdk/bin/rewind sdk-verify --sdk /path/to/sdk --public-key PUBLIC_KEY_HEX
/path/to/sdk/bin/rewind sdk-install --root /path/to/project --sdk /path/to/sdk --public-key PUBLIC_KEY_HEX
/path/to/sdk/bin/rewind check --root /path/to/project
/path/to/sdk/bin/rewind run --root /path/to/project
```

project の rewind.toml は language="1.7.1"、source_root="."、entry="main.rw"、effects を宣言する。導入は署名付き std を project の vendor に固定し、lock を更新する。SDK の場所を後から変えても project は動く。SDK 内の例を writable directory にコピーしてから sdk-install する。

## 言語と実行

```rewind
import std.number as number;
fn main()->Int effects {output} {
    match number.decimal("42") {
        Ok(n)=>{Out.println(n);publish;return 0;},
        Err(_)=>{return 2;}
    }
}
```

Int は checked signed64、Float は binary64、String は UTF-8、Bytes は byte列。List/Map/Option/Result、generic/trait、record/enum、closure、if/while/for、再帰、match を使える。let は binding の再代入を禁止し、var は許可する。mutable owner の受渡しは move/borrow/freeze を使う。std の効果/所有権/変換契約は API 文書に従う。

commit/revert/resume は計算状態と仮想I/Oの履歴を扱う。Out/File 書込みは publish まで外部へ反映しない。既に publish した外部状態は戻らず、複数ファイルとstreamを一括 atomic にしない。In/Env/Time 等の観測は record/replay の対象。main は publish を明示し、0 は成功、1..63 は業務状態。

`run --record TRACE` と `replay TRACE` で観測を検証する。`build --output ARTIFACT` の結果は `run-artifact ARTIFACT --allow-effects EFFECTS` で source なしに実行できる。artifact/replay/lock は exact compiler に依存するため、更新時に rebuild/update/再記録する。

runtime の executionSteps と CLI の --task-steps は有限の実行予算を指定する。大きな入力の費用はデータ構造と履歴に依存する。任意精度整数と無制限の再帰はまだ提供しない。SQLite と PostgreSQL は `std.db` で利用できる。PostgreSQL の TLS、credential alias、cursor は [1.7.1仕様](REWIND_v1.7.1.md) に従う。ネイティブ GUI と記録可能な HTTP/HTTPS client は利用できる。HTTP の byte / deadline / concurrent operation 制限は [1.6仕様](REWIND_v1.6.md) に従う。

主要な処理としてsort/search/sequence、heap/deque/Union-Find、整数/剰余演算、Fenwick/segment tree、BFS/DFS/Dijkstra、byte scannerを同梱する。shortest実行例でsource0の最短距離を求める。native配列はpersistentなページ単位で保持し、get/writeの木探索と返すpayloadの費用がかかる。primitive Mapはpersistent AVL、user Ord Mapは線形の費用契約に従う。std.streamのtoken/UTF-8/writerとIn.readChunk/Out.writeBytesを追加した。publish済み操作はrevert後も再送せず、新しいpending操作だけを確定する。

37moduleを同梱する。KMP/Z、bitset/sparse min/rollback DSU、topological/Bellman-Ford/SCC/MST/LCA、CSV/matrix/DPを追加した。heapはsparse persistent AVLでlanguage0.9.9のVMがsafe pointで回収する。`--steps`と`--native-work`、予算と未保証範囲は[v0.9.9実装状況](v0.9.9-status.md)を参照。

1.0のprivate field、where/factory推論、累積予算、構造化publish失敗、互換性の範囲は [1.0仕様](REWIND_v1.0.md) を参照してください。

現在の構文・動作は[言語リファレンス](language-reference.md)、診断とWindows配布は[1.5仕様](REWIND_v1.5.md)を参照してください。Windowsのbinにはrewind.exe/rewindc.exeがあります。
