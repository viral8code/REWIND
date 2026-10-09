# Native file selection

Compile and run with `--allow-effects gui,external`, followed by `-- TITLE ABSOLUTE_PATH open`
(or `save`). Windows uses its common dialog; Linux needs the system GTK 3 library
and a display. Select the initial path, or Cancel. The example prints `ready`,
`progress` while the chooser is still waiting,
then `selected`/`cancelled`, then `cached` after reverting and reusing the recorded
operation. It does not open a second chooser or change the selected file.

`std.guiDialog.openFile` and `saveFile` return a task. Create it in an `external`
region in the application task, then await it outside that region. User Cancel
returns `Ok(None)` inside the successful task result; task cancellation is a
separate task error. Selecting a save destination grants no filesystem access,
creates no file and does not publish a write. Use the existing explicitly
permitted file APIs to perform that operation.

Complete traces replay without a display. `external fresh` explicitly requests a
new chooser; `external live` retains the result only while its task/checkpoints
own it and cannot be recorded for complete replay.
