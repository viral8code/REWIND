# 入れ子のFrozen値を保つfreeze/thaw

更新日：2026-10-11。状態：source修正済み・実行検証省略。

snapshot、Map差分、履歴の組合せをsource上で追う中で、既存thawが再帰中のFrozen wrapperも外してしまう問題を確認した。例としてFrozen<List<Int>>をrecordやListに入れ、その外側をfreeze/thawすると、fieldの静的型がFrozen<List<Int>>のまま実体がmutable Listになる。

## 変更

明示freezeは型検査のFrozen<T>という戻り値に合わせて一段のwrapperを追加する。T自体がFrozen<U>ならFrozen<Frozen<U>>を作る。Frozen member viewは従来のfrozen_type_resultに合わせ、既存Frozenを二重に包まない。

明示thawの入口は既存どおり一段を外す。その後の再帰materializeでは、T内部にもともとあるFrozenをcloneして保持し、mutableへ変換しない。iteratorの要素materializeでも、要素型がFrozenの場合はその型を保持する。

これによりnested snapshotやChange<T>のTがFrozen collectionの場合も、内側の読み取り契約とwrapperが一致する。内側も編集したい場合は、さらに明示thawを呼ぶ。

## 境界

mutableな通常List/Map/structのmaterializeは従来の費用を持つ。内側Frozenの保持は既存immutable rootの共有であり、その中身を再び深くthawしない。元のrootと独立したmutable graphの規則、native resource/functionのfreeze拒否は維持する。

この修正はruntime値の一貫性に関わる。既存artifactとtraceの回帰確認は未実施。公開版や入門書の仕様を未検証のまま更新しない。

## 確認用source

tests/generic_frozen_bounds.rsに二重freeze、record field、List要素、iteratorのcaseを追加。libraries/std/tests.rwにsnapshot.capture/forkのFrozen payloadと、ChangeSet<Frozen<List<Int>>>の適用caseを追加した。実行はユーザーの方針により省略。

変更はcodex/developだけへ保存する。公開CI/CD、main統合、Releaseを行わない。
