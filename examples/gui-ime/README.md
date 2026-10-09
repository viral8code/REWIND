# Native IME input

Run `rewind run main.rw --allow-effects gui`. Select a Japanese input method,
type `ka` and confirm `か` with Enter. The program commits that input, restores
the empty View and prints `undo`. Type `ki`, confirm `き` and the program closes.

Composition is native transient state. It is drawn at the published input cursor;
only confirmed text enters the VM input journal. Publishing a changed focused
input while composition is active cancels the old context. A VM checkpoint does
not preserve an OS conversion session. Recorded confirmed input replays without
an input method, window or original source.

Linux uses an available XIM service with preedit callbacks and keeps committed
input compatibility when that style is unavailable. The test fixture uses real
IBus/Anthy on a private display and bus. Windows uses a separate IMM context for
each native window. Native context tests and committed Unicode tests are distinct
from testing a complete Japanese conversion engine on a Windows desktop.
