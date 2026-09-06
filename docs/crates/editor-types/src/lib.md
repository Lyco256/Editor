# Shared Editor types

Role: protocol-neutral identifiers, coordinate newtypes, normalized input, diagnostics, semantic style
roles, capability summaries, service status, and safe output messages. It depends only on serialization
support. Range and identifier unit tests cover basic invariants; subsystem tests cover their use.
StyleRole includes dedicated menu, Explorer, tab, selection, panel, status, and separator roles.
# Normalized mouse clicks

`MouseEvent.click_count` records normalized single, double, and triple clicks; it defaults to one
for serialized input produced by older clients.
