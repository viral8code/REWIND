# REWIND v1.9.60 — Native file selection

`std.guiDialog.openFile(title, initial)` and `saveFile(title, initial)` return
`Task<Result<Option<String>,StdError>>`. Create the operation inside an explicit
`external` region in the application task, leave that region, then await it.
GUI/external/tasks permissions remain required. User cancellation returns None;
task cancellation is a separate task failure and requests native closure.
Selecting a destination does not create/truncate a file or grant filesystem
capabilities. Save selection uses native overwrite confirmation.

The VM owns the request, response and task state; the native chooser and its
physical lifetime are external. Recorded operations reuse their result after
revert, including an existing in-flight request. Different requests at the same
logical position fail; `external fresh` explicitly requests another chooser.
Source-free debug/compact replay consumes the observations without opening a
native display. `external live` uses the existing task/checkpoint lease and
cannot produce complete replay. Cancellation never retries a physical operation.

There is at most one native chooser per process. Requests are bounded to 256
UTF-8 title bytes and 4096 path bytes. Native responses have bounded recording
reservations and malformed/non-UTF-8/oversize paths are typed errors. Invalid
arguments are rejected before copying request strings or contacting the host.
Native UI events are polled without blocking VM work; GTK processes at most 32
pending events per scheduler poll. This bounds iteration count, not OS callback
latency or filesystem responsiveness.

Windows uses the Unicode Win32 common file dialog on one bounded worker. The
worker's slot and reservation remain held through actual completion; cancellation
posts native closure and does not synchronously join a live worker. Linux uses
the system GTK 3 native file chooser (`libgtk-3.so.0`) on its owning UI thread.
Absent display/library or a different owning thread yields a defined error.
GTK's library and native framework caches have process lifetime; the VM does not
snapshot or manage OS/toolkit allocations as VM heap pages. VM-owned host state
reserves 64 KiB, plus the 2 MiB worker stack reservation on Windows. A cancelled
Windows request is reaped on subsequent dialog operations or runtime destruction;
its remaining reservation is retained until its worker terminates.

Acceptance covers native Unicode open/save/user cancellation without modifying
files, completed/in-flight rollback without a second chooser, cancellation,
request/effect/version bounds, concurrent VM progress, recorded and live lifetime,
source-free disconnected replay and both extracted SDKs. Native Linux selection,
rollback and cancellation have been exercised; Windows and final SDK publication
checks must pass before this version is released. IME composition, accessibility
integration and sustained combined-load acceptance remain separate v2 work.

SDK file digests are streamed through a fixed 64 KiB buffer, rather than loading
entire compiler binaries into temporary vectors. Size changes during hashing
are rejected. This preserves SHA-256 inventory/signature formats and existing
reproducibility/tamper verification while bounding the temporary hash buffer.

API comparison recognizes additive exports within dependency snapshots. Every
existing dependency contract must remain identical; removal, parameter/return/
effect or public type changes remain breaking. This avoids treating a compatible
new helper in an imported library as a breaking change in all its consumers.

Local validation: the four CLI scenarios passed, including an actual native
open/save/cancel story in both trace modes. The story completes a VM calculation
while the chooser is still waiting and then reuses the response after revert.
Linux keyboard acceptance runs on a private Xvfb display with real device events;
other GUI fixtures keep their targeted synthetic input. Standard-library
contracts and the actual language-1.9.60 API snapshot passed. Optimized native
acceptance passed as well: the six open/save/cancel debug/compact stories
completed in 16.290 seconds, including compilation, a one-second UI preparation
wait per story and replay. This is a whole smoke-test duration, not an input
latency guarantee. Sixteen recorded-free live cancellations retained each
checkpoint lease and released all results, buffers and pending operations after
the last lease dropped; native acceptance including those cases passed in
3.810 seconds. Final full coverage and both official SDKs remain release gates.

The historical signed-dependency regression checks a compatible added export
and then rejects a changed return type on that already exported function. Its
former expectation that every addition breaks consumers conflicted with the
additive contract above; existing signature/effect/removal checks remain.

The Windows native chooser waits for `CDN_INITDONE` before applying a pending
cancellation or test response. Acceptance clicks the ready enabled native button
rather than posting a bare command during initialization. This correction is
included in the unpublished v1.9.60; actual Windows CI remains the release gate.

Windows supplies the initial parent directory separately from the Unicode
filename. Native diagnostics exposed the chooser reusing its Documents directory
instead of the requested fixture directory. Test drivers select the actual Button
class, avoiding the shell FolderView control that also uses ID 1.
