# 001 — formal K004/K005/K006 ruling

## K004

Document `abcde\n\nabcde`, initial offset 5, tab width 4, operations Down, Down.

Checkpoint 1: offset 6, line 1, character 0, display column 0, preferred display column 5.
Checkpoint 2: offset 12, line 2, character 5, display column 5, preferred display column 5.

Old frozen expected offset 8 is invalid.

## K005

Document `12345\nx\n12345`, initial offset 5, tab width 4, operations Down, Down.

Checkpoint 1: offset 7, line 1, character 1, display column 1, preferred display column 5.
Checkpoint 2: offset 13, line 2, character 5, display column 5, preferred display column 5.

Old frozen expected offset 10 is invalid.

## K006

K006 proves that the preferred column is a terminal display-cell column computed with the supplied effective tab width. The exact escaped document exists only in `vectors/preferred-column.json` to avoid prose/test escaping drift.

Initial character offset 3; operation Down once.

Variant tab width 4: final offset 11, line 1, character 6, display column 6, preferred display column 6.
Variant tab width 8: final offset 15, line 1, character 10, display column 10, preferred display column 10.

Both variants are mandatory for K006.

For K004–K006 only, this ruling plus the canonical vector supersedes any conflicting frozen 001 fixture/expected-value code. The product requirement itself is unchanged.
