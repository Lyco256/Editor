# Fixture strategy

Fixtures are small, deterministic, and checked into feature-owned paths. Text fixtures cover Unicode,
line endings, encodings, and every built-in syntax language. Process fixtures implement local fake
servers or temporary repositories; VSIX archives are generated once and include traversal rejection
cases. Snapshot names describe the visible state and do not contain terminal escape bytes.

