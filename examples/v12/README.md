# v1.2 の表記と条件分岐内の巻き戻し

main.rwを保存して`rewind run main.rw`で実行します。出力はOddだけです。numを0b1100にするとEvenになります。

ブロックコメント、数値の桁区切り・基数、Unicode escapeと、if内のrevertを示します。Evenはpublish前なので、奇数の場合はrevertで破棄されます。
