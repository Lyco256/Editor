# Application module

Role: registers the action, effect, event, state, runtime, registry, and bootstrap boundaries. Domain
crates cannot depend on this root module, preserving acyclic dependencies and one-way state flow.

