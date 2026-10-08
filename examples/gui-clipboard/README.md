# Native clipboard editing

Run `rewind run main.rw --allow-effects gui`. Select text in the copy window
(the initial selection is already set), press Ctrl+C, then Ctrl+V in the paste
window. The application restores its empty input and prints `undo`. Press
Ctrl+V again: VM revert restores the text model, while the OS clipboard retains
the copied text. Both windows then close.

Native clipboard gestures require selected language 1.9.52. Windows uses Unicode
clipboard text; Linux uses X11 CLIPBOARD with UTF8_STRING. Text is limited to
4096 UTF-8 bytes. Copy uses the last published scene. A copied X11 selection
is owned by the window and expires when its owner closes; no clipboard manager
persistence is promised. Replay uses recorded text input without OS clipboard
access.
