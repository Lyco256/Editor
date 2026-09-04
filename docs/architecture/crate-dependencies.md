# Crate dependency diagram

`editor-types` is the protocol-neutral base. Editing, terminal, and configuration depend on it.
Workspace builds on configuration; syntax and LSP build on the editor snapshot boundary; Git builds
on workspace; compatibility builds on configuration. `app-ui` reads all domain models, and the root
package wires all adapters into the runtime. Cycles are forbidden.

