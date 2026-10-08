# Native graph adjacency

Run `rewind run main.rw` with a v1.9.49 SDK. Output is 5 followed by -1.
`fromEdges` accepts rank-one IntArray inputs; BFS distances are an IntArray.
Changing an input array later preserves the graph's input storage version.
Use `create` and `add` when incrementally building a graph. Bulk construction
avoids per-edge VM calls for a known edge set.
