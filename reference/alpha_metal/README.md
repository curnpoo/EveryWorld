# Alpha Metal renderer (reference only)

The alpha's native Metal renderer (`ed_metal`) and its GPU experiments (the alpha `ed_gpu`:
cluster, pool, graph, cull). Not part of the workspace and not built by CI.

Use: look reference and shader-maths source for the new renderer (P1-D18 in
`docs/DECISIONS.md`). Do not depend on it and do not extend it. The runnable alpha is the
`alpha-early` branch.
