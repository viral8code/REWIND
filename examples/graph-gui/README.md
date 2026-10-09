# Cancel graph work from a native window

Run `rewind run main.rw --allow-effects gui --native-work 10000000000
--history-memory 256MiB --steps 100000000 --task-steps 100000000` on one line.
Click the window while the graph task is pending to cancel it. The program prints
`ready` and then `cancelled`; if the calculation finishes first it prints
`completed` and closes the window. The window uses the native Windows/X11 backend.

The example uses a million repeated edges held in virtual-zero native pages.
It demonstrates bounded task handoff and GUI input, not a fully materialised
model workload or a universal latency bound. Published GUI/output and delivered
OS input follow the existing external-boundary contract.
