# REWIND v1.9.59 — Bounded menus and native command keys

`std.guiMenu` supplies a VM-owned popup model with up to 256 commands, unique
IDs, enabled/checkable/checked state, explicit shortcuts, pointer selection and
Up/Down/Home/End/Enter/Space/Escape/Tab navigation. Long menus use a bounded
viewport. `setEnabled` and `setChecked` let application settings update command
state; enabling an item restores selection when an open menu has no enabled item.
Drawing composes a layer over the existing view and keeps the base
focus, text and scroll state. Invalid geometry/IDs, disabled commands and empty
menus produce defined results before mutation. Menu state participates in
checkpoint/revert; republishing a restored scene updates the native window.

`gui.bounds`, `graphemeMode` and `overlay` support composition. Overlay inputs
must have matching dimensions/editing modes and unique control IDs; the combined
view is limited to 2048 widgets. Layer focus takes precedence only when present.

The native adapters expose Ctrl/Alt letters, optional Shift and F1..F24 command
names for language 1.9.59 and later. Previous selected languages retain their
native input behavior. Plain Ctrl+C/V/X in a focused text input keeps clipboard
behavior; unfocused commands can reach menus. AltGr and level-three text input
are not captured as commands. Win32 system-key fallback preserves native
Alt+Tab/Space/F4 handling; handled Alt-letter character messages are suppressed.
No scene or value/wire schema is added. Input remains recorded and replay does
not access a native display.

The shipped native `gui-menu` example toggles a checked item, publishes it,
restores the menu model, accepts a new native command and closes on Ctrl+O. This
is a popup composed in the native window scene. OS menu bars, file selection,
IME composition and accessibility integration remain separate work.

Acceptance requires independent navigation/disabled/scroll behavior, bounds and
ID/error atomicity, unchanged input views, actual native shortcut dispatch and
old-mode behavior, source-free fixture/native debug and compact replay, API
compatibility, standard-library contracts and both extracted SDKs. This increment
does not declare v2 complete.

Local acceptance: three native key tests and five CLI scenarios passed; the
changed navigation/error scenarios were checked again after the setting API
and selection repair. Optimized Linux source-free native menu recording and
disconnected replay passed in both trace modes (2.620 seconds for the complete
smoke run), as did the existing clipboard example (1.111 seconds). These are
whole smoke-test durations, not input-latency guarantees. GUI dependency API
snapshots include the additions; every previously published contract was checked
unchanged. Both-platform release and extracted-SDK checks remain publication gates.

The signed SDK inventory now admits at most 768 files (previously 512), while
retaining the 256 MiB aggregate byte cap, 1024-directory/depth-64 limits and
manifest/signature limits. The previous SDK contained 511 regular files; the
new module, documentation and example crossed the old file count. Directory
listings are bounded before sorting/hashing, and paths are limited to 4096
platform string units. Signature, reproducibility and tamper checks remain required.
