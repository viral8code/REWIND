# 記録デバッガーとDAP

REWINDのデバッガーは、`--record-mode debug`で記録した実行を前後に動かして調べます。プログラムを再実行せず、外部操作も起こしません。

```sh
rewind run main.rw --record trace.json --record-mode debug
rewind debug-session trace.json      # 対話コマンド
rewind dap                           # Debug Adapter Protocol（stdio）
```

`--record-mode compact`の記録には手順履歴が無いため、どちらも`CompactTraceNoDebugHistory`で拒否します。

## 対話コマンド（`debug-session`）

`step` / `back` / `next-line` / `previous-line` / `continue` / `reverse-continue` / `break SOURCE:LINE` / `clear SOURCE:LINE` / `task ID` / `checkpoint NAME` / `state` / `tasks` / `files` / `diff EVENT` / `quit`。`DebugEngine`（`src/v2/v05/tooling.rs`）が実装し、DAPも同じ実装を使います。

## DAP（`rewind dap`）

| request | 動作 |
|---|---|
| `initialize` / `disconnect` | capabilityを返す / 終了 |
| `launch` | `arguments.trace`の記録を検証して開く。`root`と`verifyKey`も指定できる。ソースのlive起動はしない |
| `setBreakpoints` | 記録された命令がある行だけ`verified`。ほかは理由付きで`verified:false` |
| `configurationDone` | 先頭で`stopped(entry)` |
| `threads` / `stackTrace` | VM taskとその呼出し。呼出し元の位置は`Call`命令の位置 |
| `scopes` / `variables` | 行・field・要素をページ単位（`start`/`count`）で遅延展開 |
| `evaluate` | 読取り専用のwatch。`name`、`name.field`、`name[3]`の組合せだけ。呼出し・演算子・代入・await・publish・external・乱数/時刻は拒否 |
| `next` / `stepIn` / `continue` | 記録を前へ（終端で`terminated`） |
| `stepBack` / `reverseContinue` | 記録を後ろへ |
| `source` | root内で読めるソースだけ。source-free記録では不在を説明して失敗 |

`stepOut`、`pause`、`setVariable`、条件付きbreakpoint、memory読取りは対応しません（理由付きの失敗を返します）。ネットワークlistenerはありません。1プロセスが1クライアントを扱い、2つのセッションは別プロセスで互いに状態を共有しません。

### 値の閲覧

変数は記録時の表示文字列から展開します。Secretや環境の機密値は記録時にマスク済みです。`variablesReference`と`frameId`は、その時点のeventでだけ有効で、`stepIn`/`stepBack`/`continue`などで動くと`stale`エラーになります。循環するobjectは`cycle`として展開しません。長い値は120文字で省略し、全長を示します。表示文字列から子要素を取り出すため、文字列値が`,`や`]`を含む要素は分割が不正確になることがあります。

### 上限

1メッセージ1 MiB、ヘッダ1 KiB、JSON深さ64、スタック256フレーム、1ページ1000件、`variablesReference`は1 eventあたり16384個。超過・不正なframingはセッションを閉じます。

### 手元での確認

```text
{"seq":1,"type":"request","command":"initialize","arguments":{}}
{"seq":2,"type":"request","command":"launch","arguments":{"trace":"trace.json"}}
{"seq":3,"type":"request","command":"setBreakpoints","arguments":{"source":{"path":"main.rw"},"breakpoints":[{"line":7}]}}
{"seq":4,"type":"request","command":"continue","arguments":{"threadId":0}}
```

各行を`Content-Length: N\r\n\r\n`付きで標準入力へ渡すと、`stopped(breakpoint)`の後に`stackTrace`などを呼べます。テストは[tests/dap.rs](../tests/dap.rs)です。

### エディタ設定の例（nvim-dap）

```lua
local dap = require('dap')
dap.adapters.rewind = { type = 'executable', command = 'rewind', args = { 'dap' } }
dap.configurations.rewind = {{
  type = 'rewind', request = 'launch', name = 'Replay trace.json',
  trace = '${workspaceFolder}/trace.json',
}}
```

## 診断の関連位置

所有権・効果の診断は、主な位置に加えて関連位置（`note:`）を持ちます。JSON診断は`related`、LSPは`relatedInformation`に同じ内容が入ります。位置と種類だけで、局所値やSecretは含みません。

- `use after move`：値を移動した式
- `cannot mutate an owner while borrowed`：借用の開始位置
- `InvalidContinuation`：checkpointを作った位置と関数、revertできない理由
- `TaskDeadlock`：待機グラフ（最大16辺）
