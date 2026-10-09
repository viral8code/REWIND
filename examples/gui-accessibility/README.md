# OSアクセシビリティとVMの巻き戻し

`rewind run main.rw --allow-effects gui`で実ウィンドウを開く。
「保存」を押すと表示を確定し、Viewを戻して再公開する。もう一度押すと終了する。
OSの支援ツールからも同じ操作を利用できる。入力欄のwidget IDは人が読める名前として「Email address」を使用する。

実OSの操作は外部入力であり、revertでクリックを取り消さない。復元するのはVMのViewと未確定の処理で、
再度publishした画面だけがOSへ反映される。記録済み入力のreplayにwindowや支援サービスは不要。

LinuxではAT-SPI2のsession bus、WindowsではMSAA IAccessibleを利用する。Text / UI Automationの全API対応ではない。
