# REWIND v2.0.0 — 統合と受入

GUI・ネットワーク・DB・数値処理と、VMのcommit / revert / publishを組み合わせる
処理系と標準ライブラリをまとめる。
[到達条件](ROADMAP_v2.md)と[公開判定](v2-status.md)と[受入照合](v2-acceptance.md)を
実装・検証へ結び付ける。Release workflowで両OSの条件を確認した同一commitを
mainへ統合し、署名付きSDKを公開する。

## この版で整合させるもの

compiler / language / std / lock / SDKを2.0.0へ揃え、現行API・効果・所有権・
失敗と費用の契約、source-freeの実行とreplay、署名付き配布を照合する。
既存の下位languageのartifactを、現在のデフォルトlanguageへ読み替えない。

- `std.taskError`は子taskの元の診断と型付きbudgetを保持する。
  数値ラッパーの`StdError`投影は具体的なcodeと行を保持し、全文や原因ツリーを保持しない。
- 大きなList / Mapのfreezeとiteratorをnative work・history memoryで制限する。
  ページ共有とCOWで元データの変更から独立させ、深さ・循環・資源捕捉の制限を維持する。
- native資源回収は各hostに共通するrootsを一度走査する。
  checkpointはVMの所有情報を復元し、閉じた物理接続を復活させない。

nativeのコレクションfactoryから返るList / Mapと、そのOption / Result内の値は、
通常の所有値として変更できる。例えば`numeric.valuesFloat`、`shapeFloat`、
`unicode.graphemes`の返り値へ`set` / `add`でき、入力配列・文字列は変わらない。
コンテナのVM headerを1つ確保し、不変ページをCOWで扱う。要素ごとのheap化はしない。
Frozenやopaqueな構造の表現は変えず、下位languageのartifactの挙動も維持する。

## 外部作用

`external`領域の通信・DB操作はVM外の作用である。DB transactionのcommit / rollbackと
VMのcommit / revertは別の操作とする。記録付き実行では観測を保持し、replayで送信や
DB書込みを再実行しない。既に確定した作用や送信結果が不明な失敗をrevertで消さない。
`external live`は外部操作のreplay用記録を作らない。各APIの許可・予算・失敗契約を守る。

## 配布判定

Windows / Linuxの全回帰、実DB / TLS / native GUI、数値の一致・取消・回収、
実日本語入力とaccessibilityの提供範囲、展開後SDKのsource-free、署名・checksum、
公開tagとmainの一致を確認する。未検証の項目がある間は完了として公開しない。
