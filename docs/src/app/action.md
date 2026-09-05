# Application actions

Role: normalized user intent entering the state machine. Actions contain data only and never perform
I/O. Input adapters create them; `AppState` consumes them. Trust changes and effect requests are
explicit so policy tests can observe authorization decisions.
`OpenPath` and `AddWorkspaceRoot` make file/workspace navigation explicit root actions; the
transition layer performs the corresponding model update and reports errors as output messages.
`Language` and `Git` carry typed app-ui panel actions into root transitions without embedding I/O.
`ReopenWithEncoding` uses an explicit BOM-aware codec after refusing to replace dirty text, while
`SetEncoding` changes the next-save codec and marks the buffer dirty so conversion cannot be lost.
`SplitPane`, `CloseSplit`, and `SetSplitRatio` make horizontal/vertical editor layout changes
explicit and keep them in the same state transition path as tab changes.
`SaveAs` and `CloseTab` are explicit root actions; close refuses dirty buffers and Save As does not
change the active path until the asynchronous atomic write reports success.
`QuickOpen`, `StartSearch`, and `CancelSearch` route workspace UI intent without allowing views to
perform filesystem I/O.
`RequestFileOperation` creates a typed create/rename/move/delete confirmation prompt;
`ConfirmFileOperation` is the only action that emits the corresponding background filesystem
effect, while `CancelFileOperation` drops the pending plan.
`RequestFileOperation` creates a typed create/rename/move/delete confirmation prompt;
`ConfirmFileOperation` is the only action that emits the corresponding background filesystem
effect, while `CancelFileOperation` drops the pending plan.
