# Frozen oracle conflict evidence

The frozen `K004`–`K006` tests do not use the fixtures or expected semantics stated by
`Requirements/001/001_ACCEPTANCE_CASES.md` and `Requirements/001/020_PREFERRED_DISPLAY_COLUMN_STATE.md`.

| Case | Frozen test fixture / expected result | Requirement-defined behavior | Current implementation evidence |
| --- | --- | --- | --- |
| K004 | `12345\n\nxyz` → `CharacterOffset(8)` | `abcde\n\nabcde`, display column 5 retained | editor-core unit test passes with final offset 12 |
| K005 | `12345\nx\n12345` → `CharacterOffset(10)` | long/short/long retains original goal | current implementation retains display goal and reaches line end |
| K006 | `\tabc\n\tx` → `CharacterOffset(6)` | effective tab width controls display cells | current implementation uses the supplied tab width |

The frozen acceptance suite currently reports these three failures while all other 75 cases pass.
Changing production behavior only for these unrelated fixtures would be a test-specific bypass and
would contradict the written Microsoft Edit oracle. The frozen artifacts cannot be corrected during
this Goal run.
