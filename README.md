# Folio Sheets 0.2.2 — recovered source

Rebuilt from the installed package `folio-sheets_0.2.2_amd64.deb`.

| Part | Status |
|---|---|
| `app/folio_sheets/` — the whole GTK3 Python UI (21 modules, ~7,600 lines), `sheets.css`, all ribbon icons | **Full source** (Python ships as source) |
| `packaging/` — control file, postinst/prerm/postrm, launcher, desktop file, app icons, man page, fontconfig aliases, Blank.xlsx template, SHORTCUTS.txt | **Full** |
| `prebuilt/folio_sheets_core.so` — the Rust engine (pyo3 0.22.6, quick-xml 0.36.2, zip 2.4.2, chrono 0.4.45) | **Binary only** — the Rust source is not in the package and cannot be recovered from it |

`./build-deb.sh [outdir]` reassembles the package. Its contents are identical
to the original 0.2.2 .deb (all 320 files match by checksum), and the rebuilt
package installs and runs.

You can keep developing the UI as normal. The engine can't be changed until its
source is rewritten. Two files record what it has to match:

- `ENGINE_API.txt` — every class, method and signature the Python UI calls
  (introspected from the compiled module, with its docstrings).
- `ENGINE_MODULES.txt` — the original Rust file layout
  (`engine/src/calc/fns/*.rs`, `formula/parse.rs`, `model/*.rs`, `numfmt/*.rs`, …).

To use a rebuilt engine: `FOLIO_SHEETS_ENGINE=path/to/folio_sheets_core.so ./build-deb.sh`.

## 0.3.0 — cell formatting

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
    python3 tests/restyle_fuzz.py
