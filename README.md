# Folio Sheets — source

| Part | What it is |
|---|---|
| `app/folio_sheets/` | the GTK3 Python UI |
| `engine/` | the calculation and file engine, Rust (pyo3 only; zip/deflate, XML, formulas, number formats, .xlsx, drawing via cairo/pango — all written here) |
| `packaging/` | control file, maintainer scripts, launcher, desktop file, icons, man page, Blank.xlsx |
| `prebuilt/folio_sheets_core.so` | the old 0.2 engine binary — kept only as the oracle for differential tests |

Build:

    engine/build.sh          # Rust tests + release build -> engine/out/folio_sheets_core.so
    ./build-deb.sh [outdir]  # the .deb (uses engine/out unless FOLIO_SHEETS_ENGINE is set)

## 0.7.0 — data validation and hyperlinks

- Data > Data Validation (Settings, Input Message, Error Alert), the Stop / Warning /
  Information check on entry, the in-cell list arrow and Alt+Down, the input message
  beside the cell, Circle Invalid Data. Paste carries rules; Paste Special has Validation.
  Engine: `validation.rs`.
- Hyperlinks: Insert / Edit Hyperlink (Ctrl+K) to a web page or file, a place in the
  workbook or an e-mail address; a click follows (click and hold selects), `=HYPERLINK()`
  too; Clear / Remove Hyperlinks. Outside addresses are relationships in the file.
  Engine: `links.rs`.
- Deleting the cells of a validation rule, link or other range-held element now removes it.

Tests: `tests/validation_audit.py`, `tests/links_audit.py`.

## 0.4.0 — the engine rewritten

The engine is now source in `engine/`. It keeps the old API (`ENGINE_API_FULL.txt`)
and adds formatting (`restyle`, `style_spec`, `xf_grid`, `set_merges`), undo
groups (`begin_group`/`end_group`), array formulas and dynamic-array spills. All
undo history is the engine's own; `history.py` is a thin wrapper.

Tests:

    (cd engine && ./b.sh test --no-default-features --lib)          # Rust unit tests
    python3 tests/diff_formulas.py OLD_DIR NEW_DIR BLANK.xlsx         # 529 formulas vs the old engine
    python3 tests/roundtrip.py OLD_DIR NEW_DIR BLANK.xlsx OUT_DIR     # files read back by both engines
    FOLIO_TEST_SCRIPT=tests/format_audit.py PYTHONPATH=app python3 -m folio_sheets
    FOLIO_TEST_SCRIPT=tests/command_walk.py PYTHONPATH=app python3 -m folio_sheets

## 0.3.0 — cell formatting (superseded by 0.4.0)

The engine draws every kind of formatting it reads from a file but only exposes
number formats for changing. Formatting commands therefore edit the workbook file
itself (`restyle.py`): the engine saves, the styles and the touched sheet rows are
edited, and the engine loads the result. `history.py` keeps the previous engine
Book alive so undo/redo runs straight across engine edits and formatting commands.
Cost is about one save + load per command (instant on normal sheets, ~1.5 s at
200,000 filled cells). A rewritten engine with style setters would remove that.

Tests (run under Xvfb from the source tree):

    FOLIO_TEST_SCRIPT=tests/format_audit.py PYTHONPATH=app python3 -m folio_sheets
    FOLIO_TEST_SCRIPT=tests/command_walk.py PYTHONPATH=app python3 -m folio_sheets
