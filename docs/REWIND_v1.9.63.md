# REWIND v1.9.63 — 公開画面のOSアクセシビリティ

開発中の機能単位。Linux / Windows の配布検証を終えるまで公開完了とは扱わない。

## 画面の意味と外部操作

1.9.63以降の実行では、native windowの公開済み画面をOSのアクセシビリティサービスに提供する。
LinuxはAT-SPI2、WindowsはMSAA `IAccessible` を使う。
window、label、button、checkbox、textbox、textareaの役割・名前・有効状態・checked・focusを提供し、装飾用rectは除く。
button / checkboxの名前はcaption、入力欄の名前はwidget IDとする。入力欄には人が読めるIDを付ける。
Windowsは入力値を `accValue`、LinuxはTextの文字位置・選択範囲とDescriptionで提供する。
位置の単位は既存GUIと同じUnicode scalarとclient pixelsである。

標準GUIのViewやsceneの契約は変更しない。未publishの編集、VM heap、checkpointはOSへ渡さない。
支援ツールによる既定操作は通常のpointer入力としてdispatchされる。既存の記録付き入力、
`continueInput`による新しい入力の受付、source-free replayを維持する。
revertだけでは公開済み画面は戻らず、復元したViewをpresentしてpublishすると画面とOSの意味情報が更新される。

LinuxのTextは読み取り用の部分実装。全Text / Component / UI Automationの対応を約束するものではない。
編集・選択の設定は既存のGUI APIとキー入力で行う。表示名・状態・子一覧の変更はOSの通知として送る。

## 寿命と費用

providerは描画側と同じimmutable Frameを共有する。過去の画面をOS clientのために複製しない。
window closeでFrameと待機中の操作を解放する。Linuxでは接続を停止してからcallbackを処理し、最後にinterfaceを登録解除する。Windowsの保持済みinterfaceは閉じた状態を返し、
Linuxはobjectを登録解除して専用bus connectionを閉じる。支援サービスやCOM apartmentをcheckpointで復元しない。

上限は既存sceneの1 MiB / 2048 widgets。Windowsのinterface handleは4096個、Linuxの待機中actionは64個、
一回のpollで処理するnative message / callbackは64個に制限する。
Linuxは過去のwidget IDをdefunctとして残し、currentとretiredを合わせて4096 IDを超える前にconnectionを張り直す。
この場合は古いOS参照が切断され、clientは公開画面を再取得する。画面の履歴を保持するものではない。
各windowのhost admissionに16 MiBを追加予約する。これはOS共通cacheやプロセス全体RSSの厳密な上限ではない。
Linuxのprivate GLib contextはX11の待機と合わせてpollする。アクセシビリティサービスのない環境では
初回のpresentで不在を確認し、同じwindowの各再描画で接続を繰り返さない。

LinuxはOSのGIO / GLib / AT-SPI2とsession busを動的に利用する。追加のPython実行環境をREWIND runtimeに要求しない。
Python GIとlibatspiは配布試験の独立clientにだけ使う。WindowsはOSのoleacc / oleaut32 / ole32を利用する。

## 現在の検証と未完了項目

Linuxの実bus / registry、system libatspi client、GIO clientでUnicodeの名前、役割・状態、Textのscalar範囲と選択、
無効状態のaction拒否、変更通知、native actionのdispatchを確認した。
debug 1.9.63のsource-freeプログラムでは両記録モードの操作・VM復元・サービスなしreplayを確認した。
実X11のcaptureでI/Wと界/語が別の字形になることを確認した。これは全Unicodeのglyph coverageの保証ではない。

Windowsのprovider・native COM client試験・font capture試験はmetadataの型検証済みであり、実OS検証はまだ必要。
展開後SDK、全回帰、両OSの公開CIは別の受入とする。実Windows日本語変換エンジンと数値・通信・GUIの統合反復負荷も
v2.0の残件として引き続き扱う。

SDKのdistribution上限は512 MiBとする。debug-builtの二つのコンパイラと文書が従来の256 MiBを超えたための変更で、hashは引き続きstreamingし、全ファイルのbytesを同時にメモリへ読まない。LinuxのGUI試験にはprivate runtime/cacheと明示したAT-SPI busを使用し、並列fixtureのsocketを共有しない。
