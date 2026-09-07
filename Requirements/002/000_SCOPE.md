# 000 — scope

The 001 Goal stopped because three frozen tests contradicted their written requirements. The uploaded conflict evidence shows K004–K006 were the only failing cases while the other 75 passed, and that changing production behavior only for those fixtures would be a specification violation.

002 must preserve the current implementation, repair the oracle rather than the product for this conflict, and make future oracle generation data-first.

002 does not authorize resetting `devenv`, reverting correct 001 work, deleting passing cases, weakening preferred-column behavior, or adding case/fixture-specific production branches.
