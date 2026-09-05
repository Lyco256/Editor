# Git process boundary

Git integration invokes the installed executable through structured argument vectors and an explicit
working directory. The UI emits typed Git intents; the root dispatches asynchronous operations and
returns status, diff, progress, stderr, and failure events. Workspace Trust is checked before
dispatch, and credentials are never stored by Editor.

