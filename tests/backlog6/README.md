# Backlog 6 probes (Sheet 0.22.0)

The scripts behind Backlog 6. They are probes, not gate tests: each prints what differs from Excel.

- Engine, no display: `formulas.py`, `formulas2.py` (1,219 formulas), `fmt.py`, `structure.py`, `rt.py`, `rt3.py`
  (round trips; need python3-openpyxl), `fuzz.py`, `fuzz2.py`, `fuzz3.py`, `p9.py` (panics, hangs, deep nesting).
  Run inside the 22.04 root: `python3 formulas.py <dir with folio_sheets_core.so> <Blank.xlsx> excel_fns.txt`.
- The window, real X input through xdotool: `uiharn.py` is the harness, `ui_*.py` the probes, `runui.sh <probe> [display]`
  runs one from /srv/jammy/tmp/b6 against the source tree at /tmp/fs.
- `validate.py`: well-formedness, relationships, content types, openpyxl and LibreOffice for any .xlsx.
- `FINDINGS.md`: the working notes the backlog was written from.
