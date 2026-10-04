# Task readiness

`rewind run main.rw` は異なる型の Task を待ち合わせ、`ready` / `42` を出力する。通知は result を取り出さないため、選択後に各 Task を await する。両方が完了していれば左が選ばれ、waiter の cancellation は入力を cancel しない。GUI 非同期待機は今後の版で追加する。
