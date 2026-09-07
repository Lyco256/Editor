# 006 — K004/K005/K006 Rust adapter contract

Load `Requirements/002/vectors/preferred-column.json` from a repository/manifest-derived path, not the shell working directory.

Mapping:
- K004 selects case `K004_PREFERRED_COLUMN_EMPTY`;
- K005 selects `K005_PREFERRED_COLUMN_SHORT`;
- K006 executes both `K006_PREFERRED_COLUMN_TABS` variants.

For each vector:
1. construct a real `TextBuffer` from `document`;
2. place primary caret at `initial_char_offset`;
3. clear pre-existing preferred vertical goal;
4. invoke the real production Down operation for each operation using vector `tab_width`;
5. after each operation read actual offset, logical line/character, display column, and stored preferred display column;
6. compare all fields to the vector checkpoint.

The Rust source may name fields but may not contain the K004–K006 numeric expected values. Failure output includes case, variant, operation index, expected checkpoint, actual checkpoint.

Production branching on case ID, fixture text, test environment, or test process is forbidden.
