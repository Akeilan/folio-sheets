"""Files written by the new engine, read back by the old engine (and by the new one).

    python3 tests/roundtrip.py OLD_DIR NEW_DIR BLANK.xlsx OUT_DIR
"""
import importlib.util
import os
import sys
import xml.etree.ElementTree as ET
import zipfile


def load(d):
    spec = importlib.util.spec_from_file_location("folio_sheets_core", d + "/folio_sheets_core.so")
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


old, new = load(sys.argv[1]), load(sys.argv[2])
BLANK, OUT = sys.argv[3], sys.argv[4]
os.makedirs(OUT, exist_ok=True)
fails = []


def check(name, cond, detail=""):
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  " + repr(detail)[:300]))
    if not cond:
        fails.append(name)


# ---- build a workbook with many features in the new engine
b = new.Book.open(BLANK)
data = [["Item", "Qty", "Price", "Total"], ["a", 1, 2.5, "=B2*C2"], ["b", 3, 1.25, "=B3*C3"],
        ["Sum", "=SUM(B2:B3)", "", "=SUM(D2:D3)"], ["1/2/2026", "10%", "$1,234.5", "TRUE"],
        ["'007", "=\"x\"&A2", "=1/0", "=NA()"], ["line1", "12:30", "1e5", "-3"]]
for r, row in enumerate(data):
    for c, v in enumerate(row):
        if v != "":
            b.set_input(0, r, c, str(v))
b.restyle(0, [(0, 0, 0, 3)], [{"font": {"bold": True, "color": 0xFFFFFF}, "fill": {"pattern": "solid", "fg": 0x4472C4}}], "Head")
b.restyle(0, [(3, 0, 3, 3)], [("preset", "top-double-bottom", "thin", None)], "Borders")
b.restyle(0, [(1, 2, 3, 3)], [{"numfmt": "#,##0.00"}], "Num")
b.restyle(0, [(6, 0, 6, 0)], [{"align": {"wrap": True, "vertical": "top", "horizontal": "center", "indent": 0}}], "Wrap")
b.restyle(0, [(0, 5, 1048575, 5)], [{"fill": {"pattern": "solid", "fg": 0xFFFF00}}], "Col fill")
b.restyle(0, [(20, 0, 20, 16383)], [{"font": {"italic": True}}], "Row italic")
b.set_merges(0, [(10, 0, 11, 2)], "Merge")
b.set_input(0, 10, 0, "Merged")
b.set_col_width(0, 0, 0, 14.0)
b.set_row_height(0, 6, 6, 32.0)
b.set_rows_hidden(0, 8, 8, True)
b.set_cols_hidden(0, 7, 7, True)
b.insert_sheet(1, "Two")
b.set_input(1, 0, 0, "=SEQUENCE(3,2)")
b.set_input(1, 0, 3, "=SUM(A1#)")
b.set_input(1, 5, 0, "=Sheet1!B2+Sheet1!B3")
b.set_array_formula(1, (8, 0, 9, 1), "{1,2;3,4}*10")
b.set_tab_color(1, 0xC00000)
b.define_name("Rate", "=Sheet1!$C$2", None, "the rate")
b.define_name("Local", "=Two!$A$1", 1)
b.set_input(1, 6, 0, "=Rate*2")
b.set_view(0, {"freeze_rows": 1, "freeze_cols": 1, "top_row": 1, "left_col": 1, "active": (2, 2), "sel": [(2, 2, 3, 3)], "zoom": 1.25})
b.set_iterate(True, 50, 0.01)
path = os.path.join(OUT, "rt1.xlsx")
b.save(path)

# ---- the package itself is well formed
with zipfile.ZipFile(path) as z:
    names = z.namelist()
    check("content types first", names[0] == "[Content_Types].xml", names[:3])
    for n in names:
        if n.endswith(".xml") or n.endswith(".rels"):
            try:
                ET.fromstring(z.read(n))
            except ET.ParseError as e:
                check("well-formed " + n, False, str(e))
    ct = z.read("[Content_Types].xml").decode()
    for n in names:
        if n.startswith("xl/worksheets/sheet"):
            check("content type for " + n, "/" + n in ct)
    check("shared strings written", "xl/sharedStrings.xml" in names)
    check("metadata for dynamic arrays", any("metadata" in n for n in names), names)
    s1 = z.read("xl/worksheets/sheet1.xml").decode()
    check("worksheet children in schema order", s1.index("<sheetViews") < s1.index("<sheetFormatPr") < s1.index("<cols") < s1.index("<sheetData") < s1.index("<mergeCells") < s1.index("<pageMargins"))

# ---- the old engine reads it the same
o = old.Book.open(path)
n = new.Book.open(path)
for s in range(2):
    for r in range(0, 22):
        for c in range(0, 8):
            if s == 1 and (r, c) in [(1, 0), (2, 0), (0, 1), (1, 1), (2, 1)]:
                continue  # spilled cells: the old engine shows the stored values only
            vo, vn, vb = o.value(s, r, c), n.value(s, r, c), b.value(s, r, c)
            if not (vo == vn == vb or (isinstance(vb, float) and abs(vb - (vo or 0)) < 1e-9 and abs(vb - (vn or 0)) < 1e-9)):
                check("value %d!%d,%d" % (s, r, c), False, (vb, vo, vn))
            io, inn, ib = o.input(s, r, c), n.input(s, r, c), b.input(s, r, c)
            if io.startswith("{=") and n.array_range(s, r, c) is None:
                io = io[1:-1]  # the old engine shows dynamic arrays as array formulas
            if not (io == inn == ib):
                check("input %d!%d,%d" % (s, r, c), False, (ib, io, inn))
            do, dn, db = o.display(s, r, c), n.display(s, r, c), b.display(s, r, c)
            if not (dn == db):
                check("display %d!%d,%d" % (s, r, c), False, (db, do, dn))
            so, sn = o.cell_style(s, r, c), n.cell_style(s, r, c)
            so.pop("style_index"), sn.pop("style_index")
            if so != sn and o.has_content(s, r, c):
                check("style %d!%d,%d" % (s, r, c), False, (so, sn))
check("values, inputs and styles agree", not [f for f in fails if f.startswith(("value", "input", "style", "display"))])
check("spill read back", n.value(1, 2, 1) == 6.0 and n.value(1, 0, 3) == 21.0, (n.value(1, 2, 1), n.value(1, 0, 3)))
check("old engine sees the spilled values", o.value(1, 2, 1) == 6.0, o.value(1, 2, 1))
check("array formula", n.array_range(1, 9, 1) == (8, 0, 9, 1) and n.value(1, 9, 1) == 40.0 and o.value(1, 9, 1) == 40.0)
check("merges", o.merges(0) == n.merges(0) == [(10, 0, 11, 2)], (o.merges(0), n.merges(0)))
check("column width", o.col_width(0, 0) == n.col_width(0, 0) == 14.0, (o.col_width(0, 0), n.col_width(0, 0)))
check("row height", o.row_height(0, 6) == n.row_height(0, 6) == 32.25, (o.row_height(0, 6), n.row_height(0, 6)))
check("hidden row/col", o.row_px(0, 8) == n.row_px(0, 8) == 0 and o.col_px(0, 7) == n.col_px(0, 7) == 0)
check("names", sorted(o.names()) == sorted(n.names()) == sorted(b.names()), (o.names(), n.names()))
check("name comment", n.name_comment("Rate") == "the rate")
for k in ("name", "state", "tab_color", "freeze_rows", "freeze_cols", "top_row", "left_col", "active", "sel", "zoom"):
    for s in range(2):
        a, c2 = o.sheet_info(s)[k], n.sheet_info(s)[k]
        check("sheet_info %d %s" % (s, k), a == c2, (a, c2))
check("iteration setting", n.iterate == (True, 50, 0.01) and o.iterate == (True, 50, 0.01), (n.iterate, o.iterate))
check("column style far down", n.cell_style(0, 900000, 5)["fill"] == 0xFFFF00)
check("row style far right", n.cell_style(0, 20, 9000)["italic"])

# ---- an old-engine file read by the new engine and written again keeps everything
ref = os.path.join(os.path.dirname(BLANK), "ref.xlsx")
if os.path.exists(ref):
    a = old.Book.open(ref)
    c = new.Book.open(ref)
    c.save(os.path.join(OUT, "rt2.xlsx"))
    d = old.Book.open(os.path.join(OUT, "rt2.xlsx"))
    bad = 0
    for r in range(15):
        for cc in range(6):
            sa, sd = a.cell_style(0, r, cc), d.cell_style(0, r, cc)
            sa.pop("style_index"), sd.pop("style_index")
            if a.input(0, r, cc) != d.input(0, r, cc) or sa != sd or a.display(0, r, cc) != d.display(0, r, cc):
                bad += 1
                print("  ", r, cc, a.input(0, r, cc), d.input(0, r, cc), sa, sd)
    check("ref.xlsx survives a round trip", bad == 0, bad)
    check("ref.xlsx merges", a.merges(0) == d.merges(0))

print("%d failures" % len(fails))
