# 検証・故障探索・情報の境界

test/fixture/profileの候補より先にある自由構想です。検査を自動化するだけでなく、何を確認できて何が未確認かを説明する道具を考えます。

## VQUAL-01：状態機械のmodel checking

小さな状態機械について、到達可能な状態と禁止状態を探索する。protocolやworkflowから反例の操作列を作り、そのままREWINDの再現testとして実行したい。

## VQUAL-02：scheduler選択の系統探索

taskの進み方を少しずつ変え、race、deadlock、cancelの境界を探す。失敗した選択だけを短い記録にして、同じ順序で再現したい。

## VQUAL-03：symbolic execution

実際の値の代わりに条件を追い、特定の失敗へ到達する入力を探す。解析できた範囲と打切りを表示し、生成した入力で実行結果を確かめたい。

## VQUAL-04：coverage-guided fuzzing

parser、codec、artifact reader等へ入力を変形し、未探索の経路を増やす。失敗入力の縮小、重複分類、原因位置をまとめて見たい。

## VQUAL-05：metamorphic test

正解が簡単に作れない数値・検索・model処理を、変換前後の関係で検査する。尺度変更、順序入替え、往復変換等の性質をlibraryの契約として共有したい。

## VQUAL-06：実行方式のdifferential test

interpreter、JIT、SIMD、parallel、異なるplatformを同じ入力で比較する。値だけでなく診断・予算・所有権・recordの差が出た最初の箇所を探したい。

## VQUAL-07：architecture ruleを検査する

domain処理がGUIやnetworkへ依存しない等のルールを宣言する。module graphとeffectから違反を表示し、sourceのfolder構造だけに頼らず境界を維持したい。

## VQUAL-08：権限要求のgraph

どのentryからどの外部operationへ到達できるかを見られる。実行権限を小さくしたときに使えなくなる機能を表示し、必要性をsourceまで辿りたい。

## VQUAL-09：機密値のflow追跡

Secretが加工・連結・query・logへ流れる経路を調べる。平文を表示せずに「この出力は機密入力へ依存する」と示し、公開する境界を明示したい。

## VQUAL-10：資源寿命の証明支援

connection、arena、cursor、subscriptionについて、成功・失敗・取消の全経路で終了するかを調べる。証明できない経路を操作列として示し、cleanupの抜けを見つけたい。

## VQUAL-11：費用契約と回帰検出

関数の入力量と許容steps/native work/memoryの関係を定義する。algorithmが変わったときに、値は同じでも費用が想定を超える変更を検出したい。

## VQUAL-12：protocol互換性の試験生成

旧新schemaの組合せで送受信し、unknown field、optional、enum追加の扱いを比較する。互換性の主張を、具体的なmessage例と失敗理由へ結び付けたい。

## VQUAL-13：故障scenarioの宣言

遅延、切断、部分応答、quota、cleanup失敗をscenarioとして指定する。隔離したtest環境でどの順序に故障を入れたかを記録し、復旧処理を試したい。

## VQUAL-14：compiler変換の検証

最適化前後のIRを比較し、値・effect・overflow・safe pointが保たれるかを確認する。変換ごとの証拠を残し、最適化に由来する不具合を絞り込みたい。

## VQUAL-15：確認範囲を表す品質report

OS、backend、format、入力範囲ごとに確認済み/未確認を表示する。全test成功という一つの数字だけでなく、利用する構成の根拠を見たい。

## VQUAL-16：改変を検出できる実行記録

event列、source hash、artifact、設定を検証可能な形で関連付ける。記録の完全性とapplication結果の正しさは区別し、調査資料が途中で変わっていないかを確認したい。
