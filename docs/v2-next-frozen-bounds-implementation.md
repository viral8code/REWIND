# Frozen<T>のgeneric bound判定

更新日：2026-10-11。状態：実装中・実行検証待ち。

## 変更

データsnapshot libraryの前提として、Send/Shareの判定を修正する。従来のtransfer_boundedは、外側の要求がShareの場合、Sendだけを持つ型parameterを置換しない。そのためFrozen<T>や、それをfieldに持つrecordを調べる途中でSendへ要求が変わっても、T:Sendを証明できなかった。

型parameterのboundを型graphの各位置で評価する。Frozen<T>の内部ではSend、通常のimmutable record fieldでは呼出側が要求したSend/Shareを維持する。可変ListやMap自体をShareに変更しない。borrow、function captures、resource、循環型に対する既存判定を維持する。

## 呼出しとrecordの型引数

関数呼出し、module alias経由の呼出し、record/struct constructorのSend/Share制約にも、呼出側のgeneric boundを渡す。2.0.0のlanguage modeでは、recordの具体化後の全fieldがShareなら、Frozen<T>内部にある非Shareの型引数も受理する。従来受理したrecordを追加で拒否せず、古いlanguage modeの型引数制限を維持する。

## 回帰case

- T:SendのFrozen<T>をgeneric関数内で二度共有する。
- Frozen<T>を持つimmutable recordも同様に共有し、元Listとthaw結果の変更から分離する。
- T:SendのTそのもの、Tを直接持つrecord、List<T>をShareとして渡すコードは拒否する。
- boundのないTを持つFrozen<T>、Frozen<FileHandle>もShareとして受理しない。

## 検証状態

実装とtests/generic_frozen_bounds.rsをcodex/developへ保存する。作業環境はcontainer_creationで失敗し、端末・filesystemの実行toolが利用できなかった。GitHub経由でsourceを確認したが、以下のcommandは未実行。テスト成功・format確認・性能改善を主張しない。

~~~sh
cargo test --test generic_frozen_bounds
cargo test --test language_v05
cargo test --test language_v06
cargo fmt --check
~~~

次にstd.snapshotとstd.historyからこの判定を使う。実行環境が復旧したらまず上記を検証し、さらにlibrary/CLI/artifact/replayの回帰を確認する。公開CI/CD、main統合、Release、版更新は行わない。
