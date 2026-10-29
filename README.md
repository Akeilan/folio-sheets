# Folio Sheet — source (called Folio Sheets before 0.19)

| Part | What it is |
|---|---|
| `app/folio_sheets/` | the GTK3 Python UI |
| `engine/` | the calculation and file engine, Rust (pyo3 only; zip/deflate, XML, formulas, number formats, .xlsx, drawing via cairo/pango — all written here) |
| `packaging/` | control file, maintainer scripts, launcher, desktop file, icons, man page, Blank.xlsx |
| `prebuilt/folio_sheets_core.so` | the old 0.2 engine binary — kept only as the oracle for differential tests |

Build:

    engine/build.sh          # Rust tests + release build -> engine/out/folio_sheets_core.so
    ./build-deb.sh [outdir]  # the .deb (uses engine/out unless FOLIO_SHEETS_ENGINE is set)

## 0.17.0 — Backlog 2: nothing left "not available yet"

- Move or Copy > Create a copy (and Ctrl+drag a sheet tab): everything on the sheet comes along;
  its tables get new names, formulas naming the sheet name the copy.
- Objects: Selection Pane (names, show/hide, order), Select Objects, Align and Distribute,
  Snap to Grid / Shape, Group / Ungroup / Regroup (saved as Excel's grpSp), Rotate and Flip,
  Alt Text.
- Data: Advanced Filter (in place or copy, unique records, computed criteria) and Consolidate
  (by position or labels, links to the sources); Relationships and PivotTables from related
  tables (the Data Model, joined into a very hidden sheet).
- Charts: Waterfall, Funnel, Histogram, Pareto, Box and Whisker, Treemap, Sunburst (chartEx
  parts, as Excel writes them), Radar, High-Low-Close and Contour.
- View > Page Layout with rulers; the header and footer typed on the page.
- Review: Allow Edit Ranges, Check Accessibility, Workbook Statistics, Select Comments;
  Data > Workbook Links (values of other workbooks kept, Refresh, Change Source, Break Link).
- Insert: SmartArt (seven layouts, edited again from their text), Signature Line, Object
  (a file embedded in its icon, or linked), Copy as Picture.
- View extras: Custom Views (Excel's own format), Background, Hide Ink, the Clipboard pane.
- File > Share > Email: the workbook or a PDF of it attached in the mail app.
- LibreOffice is only suggested now (old .xls/.ods need it); the app says so when it's missing.
- Dark and coloured desktop themes no longer show through (X2-01); `tests/theme_run.sh`.

Tests: `tests/backlog2_audit.py`, `tests/ui_theme.py`, Rust `tests_backlog2.rs`;
`tests/run_all.sh` runs every UI audit and the command walk.

## 0.9.0 — pictures, and copy and paste with the other apps

- Pictures on the sheet: Insert > Pictures (PNG, JPEG, GIF, BMP as they are; other types
  converted to PNG), Paste of a picture another program copied (Draw, Gable's Copy View,
  a browser), Paste > Picture; select, move, size, delete, Copy / Cut (other programs get
  the picture). Drawn by the engine with its own PNG/JPEG/GIF/BMP decoders; saved as
  pictures in the .xlsx (Excel and LibreOffice read them), and pictures in opened files are
  drawn now instead of a grey frame.
- A copied range also goes to other programs as an HTML table (with the text as before).

Tests: `pictures_audit`, and a Rust test of insert / save / reopen.

## 0.8.0 — the rest of the P2 list

- View > Window: New Window on the same workbook (Book1:2), Arrange All, View Side by Side
  with Synchronous Scrolling, Reset Window Position, Hide / Unhide, Split (panes that scroll
  on their own; drag or double-click the bars). `windows_ui.py`; engine `pane_top/pane_left`.
- Draw Border, Draw Border Grid, Erase Border (a pen over the cells; Esc puts it down); the
  Borders button repeats the last border; Fill / Font Color show the colour they will use.
- Protect Sheet and Protect Workbook: SHA-512 hashed passwords (old 16-bit hashes still
  open), the 15 permissions, Lock Cell and Hidden take effect, structure locked. Every
  command is gated, and a safety net takes back any step that reached a locked cell.
  `protect_ui.py`, engine `protect.rs` (kept out of undo).
- Text to Columns and the Text Import Wizard: delimited or fixed width, a format per column
  (General / Text / Date order / skip), destination, separators. .txt opens through it.
- Group and Outline (`outline.rs`, `outline_ui.py`): Group / Ungroup (Alt+Shift+Right /
  Left), outline bars with level and +/- buttons, Show / Hide Detail, Auto Outline, Clear
  Outline, Subtotal with Replace and Remove All.
- Spelling (F7, Hunspell) and Thesaurus (Shift+F7, MyThes), offline; the package recommends
  hunspell-en-us and mythes-en-us.
- Fixed: text from scrolled-away columns drawn inside frozen panes.

Tests: `window_audit`, `protect_audit` (walks every command on a protected sheet),
`text_audit`, `outline_audit`, `proofing_audit`, and more checks in `format_audit`.

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
