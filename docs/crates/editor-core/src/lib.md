# Editor core boundary

Role: establishes the document descriptor consumed by syntax and LSP while the Wave 1 implementation
owns the private text model. Document versions and large-file suppression are explicit. It performs no
filesystem, terminal, process, or Git I/O.

