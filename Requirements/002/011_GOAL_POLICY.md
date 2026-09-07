# 011 — Goal execution policy

Goal starts only from a valid 002 baseline.

If verify-002 already passes product behavior, proceed to final reviews and completion. If a valid canonical case fails, fix the general production behavior.

Goal cannot edit canonical vectors, reference checker, repaired K004–K006 adapter, verifier semantics, baseline reference, or 002 written oracle.

Do not classify a valid product failure as an oracle conflict because it is difficult. A post-freeze oracle conflict is recognized only when an independent final oracle reviewer can derive a concrete contradiction between frozen canonical data and frozen written invariant. The known 001 K004–K006 conflict is already adjudicated and cannot re-block 002.
