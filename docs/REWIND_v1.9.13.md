# REWIND v1.9.13

## GUI の scene 組立て

`std.gui.scene` は従来と同じ JSON を返す。widget ごとに15項目の VM Map / Json を作る処理を、借用した widget と scroll storage からの直接の native serialization に置き換える。公開 API、View の所有権、Frame wire、色・selection / focus / scroll の表現を変えない。表の白い cell に対する既定色と同じ style 設定も省く。

serializer は描画・入力・OS resource に触れない pure な処理で、VM の入力値から毎回結果を作る。cache や host の状態に依存しない。scene を作っただけでは外部画面は変わらず、従来どおり present / publish を使う。checkpoint / revert と source-free replay の値は従来の source 実装と byte 単位で一致する。resize 後の一時的な geometry 不整合を scene で新たに検査せず、既存の stage 側の検証を維持する。

## 容量・予算

最大2048 widgets、serialized JSON は1 MiB。出力 byte 数を allocation しない writer で先に数え、上限超過は従来と同じ `StdError("ByteLimit",0)`。範囲内の出力だけ、必要な長さの buffer を native memory admission 後に確保する。タイトル、caption の UTF-8 / JSON escape を含めた二回の走査であり、費用は widget 数と出力 bytes に比例する。native work は入力の文字数と widget の metadata に応じて事前に課金する。work / history memory 予算超過は従来の fatal 診断であり、GUI の Result へ隠さない。

内部 primitive は widget / map を借用し、型の違う値や欠けた field を `GuiInvalidScene` として拒否する。未知の record を任意 JSON へ変換する API にはしない。所有権・Secret・field 型を迂回して表示へ送らない。primitive は language 1.9.13 以降で提供する。古い language / artifact はその版の std source / 実装を維持する。

## 検証・測定

従来の source serializer を独立した fixture に残し、全種類の control、Unicode、JSON escape、selection / focus / scroll、削除、色変更、revert、resize 途中、ByteLimit を比較する。source を削除した debug / compact replay、native work の拒否、不正な widget 型、既存のフォーム・表と実 native window の回帰を含める。

`python3 scripts/benchmark-gui-model.py target/release/rewind --baseline /path/to/v1.9.12/rewind --output gui.json` で測定する。1000行中4行の表と3項目のフォームを更新・構築・serialize し、一つの checkpoint を保持する。ソースを削除して実行し、初期化を含めるが compile 時間は除く。warm-up 後に3回の中央値とピーク RSS を採る。native window の paint、OS 入力の遅延や高負荷の DB / HTTP と同じ指標ではない。

GUI と通信・DB の統合、clipboard / dialog / menu、IME / accessibility、追加の協調的な数値 kernel、sparse / transform / 勾配計算、v2.0 の到達条件は引き続き実装する。
