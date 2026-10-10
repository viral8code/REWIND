# GUI・描画・文書・メディア

既存native GUIの発展形と、新しい表現領域の自由構想です。画面で編集するmodelと、実際に表示・再生・印刷した結果は区別します。

## VGUI-01：vector canvasとscene graph

path、曲線、transform、clip、layerを使って図を描ける。個々のpixelをwidget化せず、図形の選択やhit test、Undoを同じscene modelで扱いたい。

## VGUI-02：themeとdesign token

色、余白、font、角丸、animationの方針をtokenで共有できる。dark modeや高contrastを切り替えたとき、application全体が一貫して変わると便利。

## VGUI-03：制約を説明するresponsive layout

単に要素を並べるだけでなく、「このlabelと入力欄を揃える」「狭いときに折り返す」を宣言できる。配置が成立しない理由や、どの制約が幅を要求するかも表示したい。

## VGUI-04：visual UI builderとsource往復

画面でwidgetを配置し、読めるREWINDの宣言へ変換できる。手書きした部分と生成部分の対応を保ち、見た目を変更してもevent処理を上書きしない体験にしたい。

## VGUI-05：状態を保つUI hot reload

styleやpureなrender処理を変えたら、入力値・focus・scrollを保持して画面を更新する。互換でないmodel変更だけは移行方法を選び、編集中の内容を捨てずに済むと嬉しい。

## VGUI-06：宣言的animationとtimeline

位置、色、opacity、pathを時間軸で変える。計算上のanimation stateは過去を調べられ、OSへ表示済みのframeとは分けて扱えると、動作の調整がしやすい。

## VGUI-07：rich textと文書model

段落、table、link、画像、styleを持つ文書を編集する。文章の構造を保ったcopy/paste、差分、Undo、exportを使い、巨大Stringの書換えだけでeditorを作らずに済みたい。

## VGUI-08：階層・巨大データのview

tree、outline、階層table、百万行規模の一覧を、必要な部分だけ読んで表示する。展開状態・選択・編集rowを安定したidentityで追い、DB cursorとの接続も自然にしたい。

## VGUI-09：型付きdrag-and-drop

fileだけでなく、row、図形、文書fragment等を型付きpayloadとして移動できる。application内のmove/copyと、他applicationへ渡す形式の変換を分けたい。

## VGUI-10：command paletteと操作履歴

menu、shortcut、button、検索から同じcommandを実行できる。commandに説明・有効条件・Undo方針を持たせ、「今なぜ実行できないか」も利用者へ示したい。

## VGUI-11：chartの連動とbrush選択

chart上の範囲選択がtableや他chartのfilterへつながる。選択したデータの出自、集約条件、元の値へ戻る操作を保持し、分析画面を少ないコードで組みたい。

## VGUI-12：3D viewと形状操作

mesh、camera、light、material、選択を扱える。数値simulationの結果を3Dで見たり、geometryの編集状態をcheckpointへ保持したりできると用途が広がる。

## VGUI-13：audio graphとsignal editor

buffer、filter、波形、再生位置をgraphとして組む。信号処理の計算結果を巻き戻して調整し、speakerへ再生済みの音は別のphysicalな結果として扱いたい。

## VGUI-14：video frameとannotation

frameをchunkで読み、検出領域・字幕・時間markerを重ねる。annotationはUndoやbranch比較を使え、大きなvideo全体をmemoryへ読み込まずに編集したい。

## VGUI-15：印刷layoutとreport designer

page、余白、改page、header/footer、tableの繰返し見出しをmodelで扱う。preview、PDF生成、実印刷を段階として選び、画面用layoutとの違いを明示できると便利。

## VGUI-16：操作可能なaccessibility tree

widgetのrole、label、関係、keyboard操作をcomponentから宣言できる。screen reader用の表示をpreviewし、視覚的な見た目と操作可能性を同じ開発画面で確認したい。
