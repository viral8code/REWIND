# REWIND v0.8 作成案

作成日: 2026-09-30。状態: **草案**。v0.7 から独立した `codex/rewind-v0.8` ブランチで実装する。

## 採用する実装範囲

1. 開発用依存。manifest の [dev_dependencies] は test/doctest だけで import できる。通常依存と合成した署名済み graph を単一 lock に固定する。推移依存・mirror・trust・失効の検査を省略しない。通常実行の import graph は通常依存から到達できるものに限定する。
2. 仮想 I/O の対話実行。`repl --root DIR` は完全な一行入力を逐次検査し、成功した入力だけを session に採用する。失敗入力の計算・仮想書込みは取り消す。Checkpoint を入力間で使え、Host への publish は常に仮想化する。実装は VM による成功 prefix の再実行と観測再利用を許容する。標準入力は対話コマンド用に予約し、プログラムの入力は EOF。新しい関数定義で過去の意味が変わる入力を拒否する。
3. 読み取り専用 trace の互換期間。0.8.0 は indexed format 1 の 0.7.0/0.8.0 を debug/timeline で表示する。artifact 実行・replay・lock は exact compiler のまま。compatibility は表示と実行を区別する。
4. inspection export。source-free 記録を非実行 JSON bundle に変換する。元記録・Host は変更せず、元の compiler と digest を保持する。export から replay/artifact 実行はできない。秘密値の伏せ方を維持する。

## 完了条件

開発依存は通常 run から見えず test/doctest から使える。lock と署名検査は mode に依存しない。対話入力の失敗後に既存の変数・Checkpoint・仮想ファイルを復元できる。観測済みの値を Host から再取得しない。0.7 記録はソースなしで読めるが 0.8 replay では拒否する。export の改変で実行権限を得られない。

## 境界

対話実行は accepted prefix を再実行するため、実行予算を入力ごとに適用する。未完の複数行編集、定義の置換、live task の対話的継続、任意の compiler への実行形式変換は初版の対象外。borrowed return・動的 trait object・暗号化 trace は v0.9 の採否検討へ送る。ネットワーク、FFI、OS threads、JIT は導入しない。
