### Changed

- **Follow photo relationships back to the batch.** Mosaic groups now link to
  their files, and the Web and CLI summaries report checked files, unique
  related files and relationship groups. The CLI prints group indexes as
  readable sequences; the value-free JSON report keeps its existing version
  and shape. In a browser batch, selecting a relationship group selects only
  its cleanable photos for verified copies; the rest remain excluded until
  selected in the batch list. Core exposes the evidence type for each group,
  and the CLI names and numbers groups consistently.
