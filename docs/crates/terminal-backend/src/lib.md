# Terminal backend

Role: owns terminal lifecycle and virtual framebuffer contracts. Cells carry semantic roles and bounds
checks return typed errors. The adapter lifecycle is enter, render, restore; the root always attempts
restore after an entered runtime. Unit tests cover cell replacement, with differential rendering and
capability behavior added by the terminal Wave 1 implementation.

