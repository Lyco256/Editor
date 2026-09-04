# Application runtime

Role: drives action/state/effect/event/render flow through injected ports. It renders once on startup,
dispatches effects outside state transitions, and restores the terminal after both success and normal
runtime errors. Error variants preserve both runtime and restoration failures. The headless test
covers enter, frame production, quit, and cleanup.

