# Text coordinate systems

Character offsets, logical line/character positions, screen cells, display widths, UTF-8 byte offsets,
and LSP wire positions are distinct. Shared newtypes prevent accidental substitution. LSP UTF-8 and
UTF-16 conversion remains confined to `lsp-client`; display-cell conversion remains in rendering.

