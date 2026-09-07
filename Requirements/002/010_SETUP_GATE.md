# 010 — setup gate

The normal setup run stops only when current production work is preserved, safety branch exists, canonical checker passes, K004–K006 read canonical vectors, both pre-freeze reviewers report zero conflicts, the other 75 acceptance cases remain passing, regular repository checks pass, the new 002 baseline and baseline-ref commits exist, `tools/verify-002` pre-goal mode passes, and worktree is clean.

Then stop. The user starts Goal Mode separately. Do not continue product modifications in this setup run.
