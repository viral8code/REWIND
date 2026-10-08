# REWIND v1.9.52 — Native clipboard gestures

Selected language 1.9.52 enables Ctrl+C, Ctrl+V and Ctrl+X for enabled, focused
textbox / textarea widgets on Windows Win32 and Linux X11. Copy reads the
selection in the last published scene, using Unicode scalar positions. Empty
selections leave the clipboard unchanged. Cut copies first and emits a Delete
key only if clipboard ownership was acquired. Paste emits ordinary `text`
input; it never directly mutates a VM view.

VM commit / revert restores the view and its edit model. The OS clipboard is
external user state: revert, begin and replay do not restore, clear or recopy it.
Published input observations retain pasted text for deterministic replay. An
application that wants new input after revert uses `gui.continueInput` or
`guiWindows.continueInput`, according to its input API. Replay consumes those
observations without opening a native window or reading the clipboard.

Windows accepts CF_UNICODETEXT, bounded UTF-16 allocation and valid Unicode.
Linux owns CLIPBOARD and serves TARGETS / UTF8_STRING, and requests UTF8_STRING
asynchronously from other applications. Replies must match the pending request
and focused widget; late replies after two seconds are discarded. A timed-out
request can be retried with another Ctrl+V. Unsupported formats, incremental
transfers, malformed text, NUL, unavailable clipboard and more than 4096 UTF-8
bytes produce no edit. Text insertion retains existing single-line / multiline
and grapheme-editing contracts.

An X11 copy is owned by its live window. Closing that window releases selection
ownership; clipboard-manager persistence is not implemented. Copy/paste gesture
support does not add a programmatic OS clipboard API, file clipboard formats,
IME composition or accessibility integration.

Each enabled native host reserves 32 KiB in the runtime memory budget for bounded
clipboard storage and transfer buffers before preparation. OS resources remain
subject to their platform ownership and close contracts. Earlier selected
language versions retain their existing keyboard behavior.

Publication requires native Unicode transfer between independent windows on both
platforms, VM undo, debug / compact recording, source-free disconnected replay,
and both extracted SDKs. See the shipped `gui-clipboard` example. This increment
does not assert v2.0 completion.
