# Task の予算と保持量

`rewind profile main.rw` で短い Task の繰返しと checkpoint を確認する。`task_instructions` は current scheduler の保持する Task、`task_instruction_total` は回収済みを含む全体の命令数。同じ Task を復元すると予算も共有され、履歴を捨てた後の新規 Task は独立した予算を持つ。
