# 047 — UI chrome glyph and width policy

Owner: WORKBENCH worker

Writable paths:
- shell workbench files owned by this lane as needed
- matching docs/tests

## Objective

Remove width-unstable decorative UI symbols and use one grapheme-aware display-width writer.

## Required implementation

1. UI chrome structural characters may be printable ASCII U+0020–U+007E or Box Drawing U+2500–U+257F only.
2. Remove `▸`, `▾`, `›`, `•`, `…`, `▌` from production chrome.
3. Use `...`, ` | `, `[+]`, `[-]`, and Box Drawing where needed.
4. User/source/file/diagnostic/completion text remains Unicode.
5. Shell sizing/truncation never uses raw `.chars().count()` as screen-cell width.
6. Shared writer segments graphemes, uses display width, clips to rect, never splits wide grapheme.

## Required tests

- `U005_NO_ICON_CHROME`
- `Q001_UNICODE_CHROME_WIDTH`
- source-policy scan
- CJK/combining labels near borders

## Done only when

all structural UI is font-portable under the allowed glyph set and Unicode data cannot corrupt geometry.
