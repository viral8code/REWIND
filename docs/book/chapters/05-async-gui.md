# 第17章 非同期処理・cleanup・取消

## Taskの二段階

async fnの呼出しはTaskを作り、spawnで実行へ進め、awaitで結果を待ちます。awaitは`Result<T,TaskError>`を返します。

~~~rewind async
async fn compute(n:Int)->Int effects {} {return n*2;}
let task=spawn compute(21);
match await task {
    Ok(value)=>{assert_eq(value,42);Out.println(value);},
    Err(_)=>{panic("task failed");}
}
publish;
~~~

TaskのT自体がResultなら、awaitの結果は二重のResultです。外側はtaskの待機失敗や取消、内側はそのAPI自身の失敗です。通信やDBの例で`take(take(await task))`になる理由はこの二段階です。

REWINDのVMは決定的な協調schedulerを使います。asyncを書くことがOS thread上のCPU並列計算を意味するわけではありません。外部I/OのworkerとVM taskの実行は別の層です。

## timeoutとisDone

Taskのlogical timeoutはwall clockの秒数ではありません。HTTPのtimeoutMillisは実時間の期限です。両者を同じ意味で扱わないでください。`task.isDone()`は待機せず完了を調べますが、完了を確認するループだけで他taskへ適切に制御を渡せるとは限りません。

ChannelとTaskGroupは、結果の通知や複数taskの寿命管理に使います。待機関係に循環があり進行できなければTaskDeadlockになり、waitGraphを調べます。取消したこととcleanupが完了したことを区別し、cancelAndJoinやjoinなどで終了を確認します。

## defer

deferはscope終了、return、失敗のcleanupを登録します。次は出力順を確認する例です。

~~~rewind defer
fn work()->Unit effects {output} {
    defer ||->Unit{Out.println("cleanup");};
    Out.println("work");
}
work();
publish;
~~~

出力はwork、cleanupです。deferもprintlnをpublishへ自動変換しません。cleanupで失敗した場合、元の失敗が主診断に残り、cleanupの失敗はcausesへ追加されます。エラーの原因が二つあるとき、最後の一つだけを見ると問題を見失います。

## 数値計算と応答性

同期native kernelの実行中は、任意の命令位置でtaskを切り替えるとは限りません。GUIと重い計算を組み合わせるなら、numericTransformAsync、numericReduceAsync、numericShapeAsync、numericInputAsync、numericAsync、graphLargeAsync、autodiffAsyncなど、必要な協調APIを選びます。

例えば要素変換と集約の対応するAPIは4096論理要素以下のstepへ分けます。これは全native APIが一律に同じstep幅で動くという保証ではありません。各APIの費用契約を確認します。async版が作る途中状態も履歴・予算の対象です。取消で部分結果を完成済み配列として返すことはしません。

# 第18章 ネイティブGUIを作る

## model、View、OSの画面

GUIの状態は三層に分けると理解しやすくなります。modelは数値や入力内容、Viewは部品と配置、OSの画面はpublish済みsceneです。modelとViewを変更しても、presentとpublishをしなければ画面へ確定しません。

最初の例はウィンドウにラベルとボタンを表示し、閉じる入力を待ちます。実行にはgui許可とネイティブ表示環境が必要です。

~~~rewind gui-minimal
import std.gui as gui;
fn checked(value:Result<Unit,gui.GuiError>)->Unit effects {} {
    match value {Ok(_)=>{},Err(e)=>{panic(e.code);}}
}
match gui.window("Hello GUI",320,180) {
    Err(e)=>{panic(e.code);},
    Ok(view)=>{
        checked(gui.label(&mut view,"message","Hello, REWIND",20,20,280,40));
        checked(gui.button(&mut view,"close","Close",20,90,120,40));
        checked(gui.present(&view));
        publish;
        var running=true;
        while running {
            match gui.nextEvent() {
                Err(e)=>{panic(e.code);},
                Ok(event)=>{
                    match gui.dispatch(&mut view,event) {
                        Some(gui.Action::Close)=>{running=false;},
                        Some(gui.Action::Activate(id))=>{if id=="close"{running=false;}},
                        _=>{}
                    }
                }
            }
        }
        checked(gui.close());
        publish;
    }
}
~~~

windowはViewを作るfactoryです。OSのウィンドウ表示はpresentでsceneを準備してpublishした時点で行われます。nextEventは入力を待ち、dispatchは座標やキーを部品に対応するActionへ変換します。

## GUIの容量

一画面の各軸は64〜4096 client pixelsです。部品は最大2048、idは空でなく128 UTF-8 bytes以内、captionは4096 bytes以内です。sceneは1MiB以内です。idの重複や部品の範囲外配置はResultで拒否されます。

Window APIの複数画面は最大16、合計画素は16,777,216までです。単一画面用とnamed-window用の入力・closeを混ぜず、対象を明示してください。ネイティブのOS標準部品すべてを自動で提供するAPIではなく、canvas sceneを中心に構成します。

## OSの条件

WindowsはWin32/GDI、LinuxはX11を使います。LinuxではDISPLAY、対応する24bit画面、libX11、font環境が必要です。Wayland環境ではXwaylandなどの条件を確認します。GUIを使わないCLI実行のためにX11を必ず起動する必要はありません。

日本語入力にはOS側のIME/XIMサービスも必要です。Linuxのnative file dialogはGTK3の条件があります。両OSで実装された機能と、すべてのOS設定・font・入力法で完全に同じ見た目になる保証は別です。

## fixtureで検証する

GUIの入力は記録可能です。`--gui-events EVENTS.json`を使う試験では、OSウィンドウを開かずにevent列を与えます。これはmodel更新と終了の回帰試験に便利ですが、fixtureが通るだけでネイティブIMEや画面の描画が試験できたことにはなりません。

# 第19章 Undo・入力欄・複数画面

## Undoの順番

Undoではcheckpointへmodel/Viewを戻し、既に受信した入力を読み飛ばし、戻した画面をpresent・publishします。単にrevertするだけではOSの画面は変わりません。

~~~syntax
revert ready;
gui.continueInput();
// 復元したmodelに基づくViewを準備
checked(gui.present(&view));
publish;
~~~

continueInputは、この実行ですでに届けた入力だけを読み飛ばします。replayでも同じ契約です。これを省くと、巻き戻した入力cursorが古いクリックをもう一度読む設計になり得ます。ユーザーが「Undo」を押した直後に古い「Add」が再度効く動作を避けるための区別です。

## 入力欄の編集

textBoxは単一行、textAreaは複数行の入力です。selectionはscalar offsetでcursorとanchorを持ちます。graphemeEditingをtrueにすると、結合文字や絵文字をcluster単位で編集できます。位置の表現はscalarのままです。

Ctrl+C/V/Xによるclipboard操作が使えます。コピーは公開したsceneの選択範囲から行います。貼り付けの確定text入力は記録されますが、OSのclipboard自体をrevertするものではありません。

IMEの未確定文字列はOS側の変換状態です。確定した文字だけがVMのtext入力になります。未確定状態をsnapshotへ含めません。変換中に異なる入力状態をpublishすると、旧contextをキャンセルする場合があります。日本語入力を扱うときは、確定textとcomposition中の表示を混同しないでください。

## 複数画面と他task

std.guiWindowsで名前付き画面を扱います。同期のnextEventAnyは入力待ち中にVM taskを進めません。他taskと組み合わせる現在のAPIにはnextEventAnyAsyncがあります。古い案内の「handoffは今後追加」は現在の制限ではありません。

GUI更新とpublishはapplication側で行い、通信・DB処理taskからはChannel等で結果を通知する構成にすると、UIと外部作用の境界を保てます。同期的な重い計算をイベントハンドラーへ直接入れると、async入力を使っていても応答性を失う可能性があります。

## form、table、menu、dialog

guiFormは入力項目と検査、guiTableは表の表示、guiMenuはメニューとshortcut、guiDialogはnative file選択の補助です。gui.overlayは同じ寸法と編集modeのsceneを合成し、下の画面のfocusとscrollを保持します。

file選択はgui、external、tasksの操作で、領域内でTaskを作り外でawaitします。選んだパスにアクセスするFile権限は別途必要です。dialogでcancelしたことはfileへの書込みではありません。選択結果は観測として再利用し、replayでOS dialogを開き直しません。

## 練習

Counterアプリケーションを作り、AddとUndoの二つのボタンを置いてください。Undo後にmodelは戻るが画面が戻らない場合、presentとpublishを確認します。保存ボタンを追加したら、「Undoは保存済みfileも戻すのか」をアプリケーションの仕様として明示してください。

# 第20章 HTTP・TCP・サービス

## HTTP client

std.http.getはTask<Result<HttpResponse,HttpError>>を返します。領域内で送信を開始し、外側でTaskとHTTP結果の両方を処理します。timeout、response容量、状態codeを明示します。

本文末尾の実習では、URLを引数で受け取る完全なHTTP clientを掲載します。getはredirectやretryを自動で行いません。HTTP 404などのstatusと、接続・TLS・期限による失敗は分けて扱います。サーバーのstatusが200だったことと、bodyが正しい形式であることも別です。

HTTPSでは証明書chainとhostnameを検証します。CA指定やcredentialは設定用APIを使います。診断を避けるためにTLSを検証しない接続へ変える設計にしないでください。

## 逐次送受信

downloadとreadでbodyをchunkごとに受け取れます。readのEOFはNoneです。upload、write、finishは送信側を分けます。接続はSendでShareではなく、scope終了やcloseで解放します。Resultから取り出す所有権にも注意してください。

buffered HTTP要求のmaxBytesは最大4MiB、timeoutは最大120,000msです。downloadのread chunkは1〜65,536 bytes、全体は最大1GiBです。期限は将来のreadも含むtransfer全体へかかります。native同時操作数も有限です。API契約の上限以内でも、work/memory予算によって失敗する場合があります。

writeの受付成功が、相手applicationで処理済みという意味になるとは限りません。送信後timeoutになった場合は結果が不明かもしれません。revertで送信を消してretryすることはできません。

## TCPとHTTP server

std.tcpはbyte streamを扱います。readはmessage境界を保証しません。アプリケーションのprotocolで長さや区切りを決め、分割・結合されたreadを処理します。HTTP用途にはhttpServerとhttpRouterを選べます。

httpRouterはmethodとpathを対応させる補助です。認証、認可、入力schema、業務transactionまで自動で完成するframeworkではありません。外部から受ける入力の上限と検査をアプリケーションに設けてください。

未応答requestを捨てたとき、listenerが生きていればoneshotの破棄により空の503応答となります。listener自体を先に閉じる場合は接続終了になります。どちらも成功した200応答として扱いません。取消の後に不要なrequest resourceを残さない設計が重要です。

## recordとretry

記録付き実行では観測を保持し、replayで送信を再実行しません。replayが成功したことは、実サーバーへもう一度送信できたという意味ではありません。通信をやり直す操作はfreshで明示し、相手のidempotency keyなどの重複防止も検討します。

同期client、外部worker、VM task、GUIのevent loopの責任を分けると、途中取消やshutdownを扱いやすくなります。閉じたnative socketはrevertで復活しません。
