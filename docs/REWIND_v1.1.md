# REWIND 1.1.0

v1.1は実行時診断と開発支援、Windows x64配布を重点にした版です。作業branchはcodex/develop。1.0のCheckpoint/publish、private field、generic・所有権、34 moduleの公開契約を継承します。

## 診断

VMで失敗した直後、cleanupやscheduler切り替えの前に発生位置とcallerを保存します。最大32frame、内側から外側の順。source-free成果物でもsource名と位置を保持します。引数・local・capture値を表示せず、既存のsecret maskを維持します。terminal制御文字はtext rendererで無害化します。cleanupの失敗は主エラーを置き換えずcausesへ保持します。

DiagnosticRecordにはserde-defaultのframes/hints/frames_truncatedを追加します。空の追加fieldはwireから省略し、旧記録の読み取りに対応します。同じcompiler版のrecord/replayで診断を含めた再現性を検査します。REWIND側のDiagnostic値の既存fieldは変更せず、追加metadataはCLI/record JSONから読みます。

ゼロ除算とsigned64 overflowを分離し、DivisionByZero/IntegerOverflow/IndexOutOfBounds/AssertionFailed/Panic/UnknownName等のコードと対処ヒントを提供します。全メッセージをこの分類へ統一するわけではなく、既存のtyped budget/task/publish分類は維持します。未知名の候補は可視のlexical bindingから最大256候補・64文字・編集距離1〜2で選びます。

native内部の全関数や、defer等を評価する補助Engine内の完全なcall stackは対象外です。安全に自動retryできると決めつけず、publish_failureのphase/appliedを保持します。

## エディタと整形

manifestなしでもstdio LSPのincremental sync、std import、補完・定義・参照・rename・signature help・既存quick fixを利用できます。診断にhintsを添え、documentFormattingを追加します。Windowsのdrive/verbatim pathとUTF-8 percent-encoded URIを処理します。UTF-16 incremental editの既存契約を維持します。

formatterは複数行String内部の末尾空白を保持します。インデント整形の範囲は維持し、一行のコードを任意に分割する整形器へ変更しません。

## Windows SDK

Linux x86_64に加えてWindows x64 MSVCのZIPをReleaseへ配布します。bin/rewind.exe・rewindc.exe、34module、署名、API文書、言語リファレンス、例を含めます。Windows runnerで全適用可能な回帰テスト、標準契約、署名/API、展開後のCLI、空白・日本語path、CRLF入力、record/replay、source-free実行、既存fileの置換を検証します。片方のbuildが失敗した場合Releaseは公開しません。

sourceのFile pathは従来どおりroot相対の`/`区切り。Windowsでは大小文字の区別や共有・権限はHost filesystemに従います。複数fileの一括atomic性は追加しません。journal spill失敗時はopen fileを閉じてから削除します。MoveFileExWのkernel32 linkageを明示します。NUL入りpathとWindowsのdevice名・末尾dot/space等を拒否し、Win32のpath解釈で別の対象へ書き込まないようにします。

WindowsではUnix owner/modeの検証を前提にするdisk cache認証を無効にし、sourceを再検査します。Linuxの認証付きcacheは1.1 manifestも対象にします。WindowsのOS memory ceilingは今回追加せず、--memory-mibはunsupported。VM予算は両OSで有効です。署名鍵は各platformのrelease単位で、各checksumと公開鍵を分けて配布します。

## ライブラリと文書

std.text.sliceはUnicode scalarの開始・終了境界だけを走査し、全入力のoffset表のallocationを廃止します。範囲・空文字・終端・多byte文字の契約を維持し、O(n)追加offset storageをO(1)へ減らします。公開APIを増やす変更は今回の重点にしません。

[言語リファレンス](language-reference.md)を正規の入口として用意し、実行可能なコード例を回帰テストします。生成済みのmodule別API文書と併せてSDKへ同梱します。1.0の成果物・traceは1.1で再生成してください。型付きResultや予算エラーを含む既存の意味は変更しません。

project管理、プロセス実行、ネットワークは今回拡張しません。外部process/相手側の状態をcheckpointのように巻き戻せるとは扱いません。
