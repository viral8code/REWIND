# Menus in a native REWIND window

Run `rewind run main.rw --allow-effects gui`. Use Ctrl+T twice (the first toggle
is undone), then Ctrl+O. The menu also supports pointer selection, Up/Down,
Home/End, Enter/Space and Escape; F10 opens it again. Output: `ready`, `toggle`,
`undo`, `toggle`, `open`. Disabled commands cannot activate.

The popup model is VM state, while native input is recorded and the scene is
applied by publish. Undo explicitly presents the restored model and skips
already delivered input before waiting for another real event. `gui.overlay`
keeps the underlying view's focus/scroll and rejects conflicting IDs, dimensions
or text-editing modes. This example composes a popup in the native window scene;
OS menu bars, accessibility integration and IME composition are separate work.
