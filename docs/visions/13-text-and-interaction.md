# 文字・入力・言語・accessibility

既存Unicode/IME/accessibilityの発展形を含む自由構想です。表示、入力、検索、serializationでは同じ文字列でも必要な意味が違うことを意識します。

## VHUMAN-01：font shapingの道具

font選択、glyph、ligature、scriptごとのshapeを扱う。文字indexとglyph位置の関係を保持し、選択やhit testを正しい単位で行いたい。

## VHUMAN-02：双方向textの位置対応

論理順と表示順を明示して、cursor、selection、diagnostic位置を扱う。混在scriptのcodeや文書でも、表示上の位置から元文字へ正しく戻りたい。

## VHUMAN-03：組版のlayout engine

禁則、縦書き、ruby、hyphenation、段組をmodelとして扱う。画面とPDFで同じ文書の意味を保持し、行分割の理由を確認したい。

## VHUMAN-04：文法を扱うmessage catalog

単語置換だけでなく、複数形、語順、単位、文脈を含むmessageを定義する。parameter型を検査し、翻訳で引数が欠けた箇所を見つけたい。

## VHUMAN-05：locale-awareな入力変換

表示用の数値・日付・区切りを、localeに応じて入力として解析する。交換用の固定formatとはAPIを分け、曖昧な入力は候補として返したい。

## VHUMAN-06：入力をactionへ変換するmap

keyboard、mouse、gamepad、touchをcommandへ対応させる。device固有のeventからapplicationの意図を切り離し、利用者が割当てを変更できるようにしたい。

## VHUMAN-07：gestureとpenの認識

drag、pinch、stroke、pressureをtyped eventとして扱う。誤認識やcancelをmodelへ伝え、入力履歴を使ってUIの反応を調べたい。

## VHUMAN-08：speech-to-textのstream

音声入力から部分認識・確定結果・confidenceを受け取る。仮の文字列と確定textを分け、認識結果の訂正を通常の編集modelへつなぎたい。

## VHUMAN-09：text-to-speechと読み上げ制御

読み上げqueue、voice、発音、pause、cancelを扱う。文書位置と再生位置を関連付け、実際に再生済みの音声はphysicalな結果として区別したい。

## VHUMAN-10：Braille・触覚等への表示

同じsemantic UIから別の出力形式を作る。視覚的な位置だけに依存せず、label、関係、現在状態を伝えられるcomponentにしたい。

## VHUMAN-11：手書き数式・図形の入力

strokeから数式や図形の候補を得て、利用者が確定する。認識結果をsymbolic expressionやsceneへ渡し、元strokeとの対応を残したい。

## VHUMAN-12：IME状態を調べるworkbench

composition、candidate、確定、取消のeventを見て、backend差や入力中の不具合を調べる。実IMEの動作とfixtureを比較し、編集modelの更新を確認したい。

## VHUMAN-13：複数の操作方法を持つUI

同じ操作をtext command、button、voice、shortcutで使える。画面専用event handlerへ処理を閉じ込めず、意図と結果を共通の型にしたい。

## VHUMAN-14：keyboardだけで調べるlayout

focus順、shortcut衝突、操作不能な要素を図で見る。mouseを使わずに一連の操作ができるかを、開発時に確認したい。

## VHUMAN-15：semanticなUI試験

pixel比較だけでなく、role、label、値、focus、関連要素をqueryする。themeやfontが変わっても意味の同じUIを検証し、accessibility backendとの一致を見たい。

## VHUMAN-16：利用者が選ぶinteraction profile

文字size、contrast、動きの抑制、操作速度、入力方式をprofileにする。applicationが個別に対応するのではなく、componentへ一貫して反映できると便利。
