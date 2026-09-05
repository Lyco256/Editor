# Recovery and session format

Recovery data is versioned, serialized under the application data directory, and replaced
atomically. A session records canonical workspace roots, open tabs/splits, active editor, cursor and
selection state, and unsaved text. Corrupt or incompatible records are quarantined with a warning;
the last valid recoverable contents remain available and no workspace file is overwritten implicitly.

