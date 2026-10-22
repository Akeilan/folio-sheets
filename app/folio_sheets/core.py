"""The native engine (folio_sheets_core.so)."""
import os
import sys

_here = os.path.dirname(os.path.abspath(__file__))
# the copy next to the package (development tree) wins over the installed one
for p in (os.path.dirname(_here), "/usr/lib/folio-sheets"):
    if os.path.exists(os.path.join(p, "folio_sheets_core.so")):
        if p not in sys.path:
            sys.path.insert(0, p)
        break

import folio_sheets_core as _c  # noqa: E402

Book = _c.Book
parse_ref = _c.parse_ref
col_name = _c.col_name
format_number = _c.format_number
ref_spans = _c.ref_spans
function_names = _c.function_names
check_formula = _c.check_formula
MAX_ROWS = _c.MAX_ROWS
MAX_COLS = _c.MAX_COLS


def a1(r, c):
    return "%s%d" % (col_name(c), r + 1)


def range_a1(rg):
    r1, c1, r2, c2 = rg
    if r1 == 0 and r2 == MAX_ROWS - 1 and not (c1 == 0 and c2 == MAX_COLS - 1):
        return "%s:%s" % (col_name(c1), col_name(c2))
    if c1 == 0 and c2 == MAX_COLS - 1:
        return "%d:%d" % (r1 + 1, r2 + 1)
    if (r1, c1) == (r2, c2):
        return a1(r1, c1)
    return "%s:%s" % (a1(r1, c1), a1(r2, c2))
