# Requirements 003 source-issue evidence matrix

The expected values below are fixed from the user-visible invariants in Requirements 003 and
independent fixture contents (`PRIMARY`, `SECONDARY`, `SHARED`, `THIRD`, `SPLIT`). They are not
computed by the production resolver.

## S003-01

- SOURCE_ISSUE_ID: S003-01
- USER_VISIBLE_SYMPTOM: Requirements 002 verifier/reviews passed while the reported pane operation remained broken.
- MINIMAL_REPRODUCTION: Run the 003 product-boundary secondary-pane reproduction at the 002 completion state, then compare the visible target and non-target buffers.
- EXPECTED_RESULT: The visible secondary buffer changes and the primary buffer remains unchanged; completion evidence must include that product observation.
- BASELINE_ACTUAL_RESULT: 002 completion commit `145b10c7e743726ed38fbef02b04c56d530c249a` still produced the S003-02 and S003-03 failures.
- BASELINE_EVIDENCE: `docs/testing/requirements-003-baseline-red.log` (`EXIT_CODE: 101`, S003-02/S003-03 FAILED).
- SUSPECTED_LAYER: Product-boundary acceptance was absent; active compatibility buffer and shared selection state masked pane binding.
- FIX_COMMIT: `bb94d29ac03b3e8b125b15c41afd8122661644ca` (`fix(app): isolate pane-local editor state`).
- POST_FIX_ACTUAL_RESULT: The same command, fixtures, inputs, and observations pass; visible target-only mutation is recorded.
- POST_FIX_EVIDENCE: `docs/testing/requirements-003-postfix-green.log`, S003-02 through S003-06 PASS.
- REGRESSION_TEST_ID: `s003_02_different_buffer_secondary_pointer_and_edit`, `s003_03_same_buffer_panes_restore_independent_cursor_state`.
- FINAL_STATUS: CLOSED

## S003-02

- SOURCE_ISSUE_ID: S003-02
- USER_VISIBLE_SYMPTOM: Secondary-pane pointer/edit was resolved against the wrong buffer and could mutate primary content.
- MINIMAL_REPRODUCTION: Pane A displays `PRIMARY`, pane B displays `SECONDARY`; focus B, click column 9, paste `!`.
- EXPECTED_RESULT: focused/displayed/resolved/pointer owner are B/Y; `SECONDARY!` is the only content change and `PRIMARY` is unchanged.
- BASELINE_ACTUAL_RESULT: Focused pane 1 displayed secondary, but resolved content remained `SECONDARY` and primary became `!PRIMARY`.
- BASELINE_EVIDENCE: `docs/testing/requirements-003-baseline-red.log`, S003-02 step 4.
- SUSPECTED_LAYER: `AppState::buffer` was used as the operation buffer instead of the focused pane's displayed tab buffer.
- FIX_COMMIT: `bb94d29ac03b3e8b125b15c41afd8122661644ca`.
- POST_FIX_ACTUAL_RESULT: The same focus/click/paste sequence yields `SECONDARY!`; primary remains `PRIMARY`.
- POST_FIX_EVIDENCE: `docs/testing/requirements-003-postfix-green.log`, S003-02 PASS; S003-04/05/06 cover focus, switch, and split immediacy.
- REGRESSION_TEST_ID: `s003_02_different_buffer_secondary_pointer_and_edit`, `s003_04_focus_switch_stress_keeps_resolution_current`, `s003_05_buffer_switch_immediately_before_edit`, `s003_06_split_creation_immediate_pointer_and_edit`.
- FINAL_STATUS: CLOSED

## S003-03

- SOURCE_ISSUE_ID: S003-03
- USER_VISIBLE_SYMPTOM: An acceptance oracle could pass while checking the wrong abstraction and shared cursor state erased the other pane's position.
- MINIMAL_REPRODUCTION: Both panes display `SHARED`; click pane A at column 2, focus/click pane B at column 4, then alternate focus and observe both cursors.
- EXPECTED_RESULT: Pane A cursor remains 2 and pane B cursor remains 4; expected state comes from the independent fixture invariant.
- BASELINE_ACTUAL_RESULT: After the same sequence both observed cursors were A=4 and B=4.
- BASELINE_EVIDENCE: `docs/testing/requirements-003-baseline-red.log`, S003-03 step 5.
- SUSPECTED_LAYER: Selection state was stored only in the shared `TextBuffer` projection rather than per pane.
- FIX_COMMIT: `bb94d29ac03b3e8b125b15c41afd8122661644ca`.
- POST_FIX_ACTUAL_RESULT: The same sequence observes independent pane cursors A=2 and B=4.
- POST_FIX_EVIDENCE: `docs/testing/requirements-003-postfix-green.log`, S003-03 PASS.
- REGRESSION_TEST_ID: `s003_03_same_buffer_panes_restore_independent_cursor_state`.
- FINAL_STATUS: CLOSED
