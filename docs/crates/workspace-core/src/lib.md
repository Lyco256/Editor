# Workspace core boundary

Role: establishes canonical trusted/untrusted policy state for workspace services. The root remains
responsible for enforcing process authorization. Wave 1 adds safe filesystem, search, and document I/O
adapters; UI types never enter this crate.

