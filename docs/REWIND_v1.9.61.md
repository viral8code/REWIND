# REWIND v1.9.61 — Native IME and repeated service acceptance

Selected language 1.9.61 enables native preedit handling for the published
textbox/textarea. No new event or observation format is introduced. Only
committed Unicode text enters the existing VM input journal; the OS composition,
conversion session and candidate UI are external transient state.

Linux prefers an available XIM service's preedit callback style. Native deltas
use scalar offsets, support feedback-only changes and bound retained text to
4096 UTF-8 bytes. UTF-8 input is decoded according to the supplied character
count rather than an unbounded terminator scan; wchar input is validated too.
The surface draws underlined preedit and its cursor inside the focused widget.
Services without callbacks retain the existing committed-input route. A running
input service, fonts and its conversion engine remain OS facilities.

Windows uses a separately owned IMM context per native window, bounded Unicode
composition/result reads, inline preedit and published cursor/candidate placement.
Handled result text does not also pass through the default IME-character route.
Contexts are destroyed on window closure. A changed published focused input identity/content/selection
cancels an active composition and replaces its native context, preventing old
queued feedback from being applied to the restored/other input. Ordinary
publication with inactive composition does not recreate the context per character.
Geometry/style changes move/repaint preedit without replacing the input context.
Updating an already visible Windows surface does not activate it again.
Focus loss cancels preedit. VM revert alone does not recreate an OS session;
publishing a changed input synchronizes the physical surface.

An enabled native host reserves 1 MiB for its bounded event queue, preedit and
conversion/geometry scratch in addition to the earlier clipboard reservation.
Admission occurs before native preparation. The Windows queue is bounded to
512 KiB and 4096 entries, and drained entries release their counted bytes.
Both adapters poll at most 64 native events/messages per nonblocking call.
These bounds cover iteration/storage, not arbitrary OS callback latency or all
OS/toolkit allocations. Candidate anchors are clipped to narrow/scrolled inputs.

The `gui-ime` example is shipped in SDK builds and documentation exports. Native
committed input remains undoable in the View, and debug/compact source-free replay
runs without a display or conversion service. Linux acceptance uses real
IBus/Anthy and device input on a private display/bus, covering preedit, confirmation,
conversion, focus cancellation and queued-feedback lifetime. Windows context
lifetime/queue tests and committed-Unicode SDK input are separate from verifying
a complete Japanese conversion engine on a Windows desktop. That latter
verification, OS accessibility and font-service coverage remain v2 acceptance
work; a metadata type check alone is not Windows native acceptance.

The combined service acceptance repeats eight rollback/save cycles in the same
VM for each real SQLite/PostgreSQL adapter. Each adapter handles 16 HTTP requests,
16 native clicks during query waits, independently verifies eight durable rows,
closes its resources and replays compiled observations without DB/network/display
access. It samples the VM process's RSS/working set and checks explicit resource
cleanup. Samples are observations, not a sustained latency/memory guarantee.
The 256 MiB history limit includes PostgreSQL's conservative 192 MiB opening
admission and other host/VM state; it is not an OS RSS cap. Retained recorded
observations are distinguished from temporary resources.

Longer runs use compact traces because debug step inspection has a fixed 16 MiB
index cap. The earlier short source/debug/compact service tests remain. Full
regression, standard-library contracts and both extracted official SDKs are
required before publication. Remaining work and completion criteria are tracked
centrally in [v2 status](v2-status.md).

Windows native dialog automation explicitly enters the requested absolute path
in the filename Edit control before pressing the real Open/Save Button. The
shell's asynchronous initial selection and hidden-extension display do not
determine the tested file. Production chooser behavior is unchanged by this
fixture repair; actual returned path spelling is still recorded and replayed.
