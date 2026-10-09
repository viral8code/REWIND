# Native input during materialized reverse-mode work

Run `rewind run main.rw --allow-effects gui`. Click the window to cancel a
cooperative nonlinear reverse pass over 262144 materialized nonzero cells.
`ready` means forward tape construction has finished and backward work is pending.
If it finishes before the click, the program reports `completed` and closes.

The SDK test requires native pointer input while work is still pending, checks
cancellation and replays with the display disconnected. Its two samples cover
injection-to-cancellation after `ready`; they exclude synchronous forward setup
and do not constitute sustained GUI latency percentiles.
