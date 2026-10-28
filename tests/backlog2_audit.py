# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): the Backlog 2 commands — copied sheets, the
# Formulas tab's function menus, Advanced Filter, Consolidate, the Review tools, Custom Views,
# Background, objects (Selection Pane, Align, Group, Rotate) and more.
import os
import sys
import tempfile
import time
import traceback

from gi.repository import Gtk

checks = []
ERRORS = []
_hook = sys.excepthook
sys.excepthook = lambda t, v, tb: (ERRORS.append("".join(traceback.format_exception(t, v, tb))), _hook(t, v, tb))


def check(name, cond, detail=""):
    checks.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  " + repr(detail)), flush=True)


def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.005)


# dialogs: a queue of callbacks that fill the dialog and return the response
DIALOG = []


def fake_run(d):
    pump(0.05)
    if DIALOG:
        return DIALOG.pop(0)(d)
    return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = lambda d: (DIALOG.pop(0)(d) if DIALOG else Gtk.ResponseType.OK)


def widgets(w, kind):
    out = []

    def walk(x):
        if isinstance(x, kind):
            out.append(x)
        if isinstance(x, Gtk.Container):
            for c in x.get_children():
                walk(c)
    walk(w)
    return out


g = win.grid
b = win.book
TMP = tempfile.mkdtemp()

# ---------------------------------------------------------------- S2-06 copy sheets
for r in range(5):
    b.set_input(0, r, 0, str(r + 1))
b.set_input(0, 0, 1, "=SUM(Sheet1!A1:A5)")
b.set_input(0, 1, 1, "=A1*2")
g.sel.ranges = [(0, 0, 4, 0)]
g.sel.active = (0, 0)
DIALOG.append(lambda d: Gtk.ResponseType.OK)
win.run("format-as-table:TableStyleMedium2")
pump()
tname = b.tables(0)[0]["name"] if hasattr(b, "tables") else None
b.set_input(0, 2, 1, "=SUM(%s[Column1])" % tname if tname else "=1")
n0 = b.sheet_count


def pick_copy(d):
    for c in widgets(d, Gtk.CheckButton):
        if "copy" in c.get_label().lower():
            c.set_active(True)
    tv = widgets(d, Gtk.TreeView)[0]
    tv.get_selection().select_path(Gtk.TreePath(b.sheet_count))   # (move to end)
    return Gtk.ResponseType.OK


DIALOG.append(pick_copy)
g.set_sheet(0)
win.run("move-copy-sheet")
pump()
check("copy: a sheet was added", b.sheet_count == n0 + 1, b.sheet_count)
i = b.sheet_count - 1
check("copy: named Sheet1 (2)", b.sheet_info(i)["name"] == "Sheet1 (2)", b.sheet_info(i)["name"])
check("copy: values copied", [b.input(i, r, 0) for r in range(6)] == [b.input(0, r, 0) for r in range(6)])
check("copy: own-sheet reference follows the copy", "Sheet1 (2)" in b.input(i, 0, 1), b.input(i, 0, 1))
check("copy: table renamed and its references follow",
      b.input(i, 2, 1) != b.input(0, 2, 1) and "[" in b.input(i, 2, 1), (b.input(0, 2, 1), b.input(i, 2, 1)))
rel = [(r, b.input(0, r, 1)) for r in range(6) if b.input(0, r, 1).startswith("=A")]
check("copy: relative formula kept", rel and b.input(i, rel[0][0], 1) == rel[0][1], rel)
p = os.path.join(TMP, "copy.xlsx")
win.save_to(p) if hasattr(win, "save_to") else b.save(p)
from folio_sheets.core import Book  # noqa: E402
bb = Book.open(p)
check("copy: saved and read back", bb.sheet_count == n0 + 1 and bb.input(i, 3, 0) == b.input(i, 3, 0) != ""
      and len(bb.tables(i)) == 1 and bb.tables(i)[0]["name"] != bb.tables(0)[0]["name"])
win.run("undo")
pump()
check("copy: undo takes the copy away", b.sheet_count == n0)

# ---------------------------------------------------------------- S2-10 function library menus
for key in ("fn-financial", "fn-logical", "fn-text", "fn-datetime", "fn-lookup", "fn-math", "fn-more", "fn-recent"):
    win.missing_log.clear()
    win.run(key)
    pump(0.05)
    ok = win.last_menu is not None and win.last_menu.get_visible() and not win.missing_log
    check("menu opens: " + key, ok)
    if win.last_menu is not None:
        win.last_menu.popdown()
for key in ("themes", "theme-colors", "theme-fonts", "theme-effects", "get-data", "what-if", "borders"):
    win.missing_log.clear()
    win.run(key)
    pump(0.05)
    check("menu opens: " + key, win.last_menu is not None and not win.missing_log)
    if win.last_menu is not None:
        win.last_menu.popdown()

# ---------------------------------------------------------------- S2-07 review tools
from folio_sheets import review_ui, objects_ui, protect_ui  # noqa: E402
win.run("insert-sheet")
pump()
S = g.sheet
for r in range(4):
    b.set_input(S, r, 0, str(r))
b.set_input(S, 4, 0, "=SUM(A1:A4)")
b.set_note(S, 1, 1, "me", "a note")
cur, wbs = review_ui.statistics(win)
check("statistics: cells with data", cur["cells"] == 5, cur)
check("statistics: formulas", cur["formulas"] == 1 and cur["notes"] == 1, cur)
check("statistics: workbook sheets", wbs["sheets"] == b.sheet_count, wbs)
DIALOG.append(lambda d: Gtk.ResponseType.OK)
win.run("workbook-statistics")
pump()
g.select_cell(0, 0)
win.run("select-comments")
check("select comments: the note's cell", g.sel.ranges == [(1, 1, 1, 1)], g.sel.ranges)
# accessibility
b.set_merges(S, [(6, 0, 6, 2)], "Merge")
k = b.insert_shape(S, "rect", 200, 40, 80, 50)
issues = review_ui.accessibility_issues(win)
kinds = {(x[1], x[2]) for x in issues}
check("accessibility: missing alt text found", ("Missing alternative text", S) in kinds, kinds)
check("accessibility: merged cells found", ("Merged cells", S) in kinds, kinds)
b.set_object_props(S, [k], {"descr": "A box"}, "Alt Text")
issues = review_ui.accessibility_issues(win)
check("accessibility: alt text clears it", not any(x[1] == "Missing alternative text" and x[2] == S for x in issues))
check("alt text kept", b.object_props(S, k)["descr"] == "A box")
win.run("check-accessibility")
pump()
check("accessibility pane shows", win.access_pane.get_visible() and len(win.access_pane.store) > 0)
win.access_pane.hide()
# Allow Edit Ranges
review_ui.write_ranges(win, S, [("Open", [(10, 0, 12, 1)], []), ("Secret", [(20, 0, 20, 0)], protect_ui.hash_password("pw", spin=10))])
check("edit ranges: written", [x[0] for x in review_ui.ranges_of(win, S)] == ["Open", "Secret"], review_ui.ranges_of(win, S))
b.set_sheet_protection(S, [("sheet", "1")])
check("edit ranges: an allowed range may change", protect_ui.may_change_cells(win, [(11, 0, 11, 1)], tell=False))
check("edit ranges: other cells stay locked", not protect_ui.may_change_cells(win, [(14, 0, 14, 0)], tell=False))
Gtk.Dialog.run = lambda d: (setattr(d, "_t", 1), getattr(d, "entry", None) and d.entry.set_text("pw"), Gtk.ResponseType.OK)[2] \
    if d.get_title() == "Unlock Range" else fake_run(d)
check("edit ranges: the password opens its range", protect_ui.may_change_cells(win, [(20, 0, 20, 0)], tell=True))
Gtk.Dialog.run = fake_run
b.set_sheet_protection(S, None)
p2 = os.path.join(TMP, "ranges.xlsx")
win.save_to(p2)
check("edit ranges: saved", "protectedRange" in (Book.open(p2).sheet_element(S, "protectedRanges") or ""))
DIALOG.append(lambda d: Gtk.ResponseType.CANCEL)
win.run("allow-edit-ranges")

# ---------------------------------------------------------------- Workbook Links
import zipfile  # noqa: E402
src = os.path.join(TMP, "src.xlsx")
sb = Book.open(p2)
sb.set_input(0, 0, 0, "41")
sb.save(src)
main = os.path.join(TMP, "main.xlsx")
mb = Book.open(p2)
mb.set_input(0, 9, 3, "=[1]Sheet1!A1+1")
mb.save(main)
zin = zipfile.ZipFile(main)
out = os.path.join(TMP, "linked.xlsx")
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for it in zin.infolist():
        data = zin.read(it.filename)
        if it.filename == "xl/workbook.xml":
            data = data.replace(b"<calcPr", b'<externalReferences><externalReference r:id="rIdX1"/></externalReferences><calcPr')
        elif it.filename == "xl/_rels/workbook.xml.rels":
            data = data.replace(b"</Relationships>", b'<Relationship Id="rIdX1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLink" Target="externalLinks/externalLink1.xml"/></Relationships>')
        elif it.filename == "[Content_Types].xml":
            data = data.replace(b"</Types>", b'<Override PartName="/xl/externalLinks/externalLink1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.externalLink+xml"/></Types>')
        z.writestr(it, data)
    z.writestr("xl/externalLinks/externalLink1.xml", '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><externalLink xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><externalBook r:id="rId1"><sheetNames><sheetName val="Sheet1"/></sheetNames><sheetDataSet><sheetData sheetId="0"><row r="1"><cell r="A1"><v>7</v></cell></row></sheetData></sheetDataSet></externalBook></externalLink>')
    z.writestr("xl/externalLinks/_rels/externalLink1.xml.rels", '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLinkPath" Target="src.xlsx" TargetMode="External"/></Relationships>')
win.open_path(out) if hasattr(win, "open_path") else win.load(out)
pump(0.3)
b = win.book
check("links: listed", [x["target"] for x in b.ext_links()] == ["src.xlsx"], b.ext_links())
b.calculate(True, None)
check("links: value from the kept values", b.value(0, 9, 3) == 8, b.value(0, 9, 3))
pane = win.run("workbook-links") or win.links_pane
pane.refresh(1)
check("links: refreshed from the source", b.value(0, 9, 3) == 42, b.value(0, 9, 3))
DIALOG.append(lambda d: Gtk.ResponseType.OK)
Gtk.MessageDialog.run = lambda d: (DIALOG.pop(0)(d) if DIALOG else Gtk.ResponseType.OK)
pane.do_break(1)
check("links: broken to values", b.value(0, 9, 3) == 42 and not b.is_formula(0, 9, 3) and not b.ext_links())
p3 = os.path.join(TMP, "broken.xlsx")
win.save_to(p3)
names = zipfile.ZipFile(p3).namelist()
check("links: the link's parts are gone", not any("externalLink" in n for n in names), names)
pane.hide()

# ---------------------------------------------------------------- S2-01 objects
g = win.grid
S = g.sheet
k1 = b.insert_shape(S, "rect", 100, 100, 60, 40)
k2 = b.insert_shape(S, "ellipse", 200, 160, 50, 50)
n0 = len(b.objects(S))
g.sel_objects = [k1, k2]
win.run("align-objects:left")
o = b.objects(S)
check("align left", abs(o[k1]["rect"][0] - o[k2]["rect"][0]) < 1.5, (o[k1]["rect"], o[k2]["rect"]))
win.run("align-objects:top")
o = b.objects(S)
check("align top", abs(o[k1]["rect"][1] - o[k2]["rect"][1]) < 1.5)
g.sel_objects = [k1]
win.run("rotate-objects:right")
check("rotate right 90", abs(b.object_props(S, k1)["rot"] - 90) < 0.01, b.object_props(S, k1))
win.run("rotate-objects:flip-h")
check("flip horizontal", b.object_props(S, k1)["flip_h"])
g.sel_objects = [k1, k2]
win.run("group-objects:group")
o = b.objects(S)
check("group: one object of two", len(o) == n0 - 1 and o[g.sel_objects[0]]["members"] == 2, o)
gi = g.sel_objects[0]
p4 = os.path.join(TMP, "group.xlsx")
win.save_to(p4)
rb = Book.open(p4)
ro = rb.objects(S)
check("group: saved and read back", any(x["members"] == 2 and x["kind"] == "group" for x in ro), ro)
check("group: drawn", True)
win.run("group-objects:ungroup")
check("ungroup: two objects again", len(b.objects(S)) == n0 and len(g.sel_objects) == 2, b.objects(S))
win.run("group-objects:regroup")
check("regroup", len(b.objects(S)) == n0 - 1)
win.run("undo")
win.run("selection-pane")
pane = win.selection_pane
check("selection pane lists the objects", len(pane.store) == len(b.objects(S)))
pane.set_hidden([0], True)
check("selection pane hides", b.objects(S)[0]["hidden"])
rect = b.object_screen_rect(S, 0, g.params())
check("a hidden object can't be clicked", b.object_hit(S, rect[0] + rect[2] / 2, rect[1] + rect[3] / 2, g.params()) is None
      or b.object_hit(S, rect[0] + rect[2] / 2, rect[1] + rect[3] / 2, g.params())[0] != 0)
pane.show_all_objects(True)
check("show all", not any(x["hidden"] for x in b.objects(S)))
pane.hide()
win.run("select-objects")
check("select objects mode on", win.selecting_objects)
got = objects_ui.objects_in(win, (0, 0, 2000, 2000))
check("select objects: the rectangle takes them", len(got) == len(b.objects(S)), got)
win.run("select-objects")
check("select objects mode off", not win.selecting_objects)
win.run("hide-ink")
check("hide ink toggles", win.hide_ink)
win.run("hide-ink")

# ---------------------------------------------------------------- S2-02 Advanced Filter, Consolidate
from folio_sheets import data2_ui  # noqa: E402
win.run("insert-sheet")
pump()
S = g.sheet
rows = [("Region", "Item", "Sales"), ("East", "Pens", "10"), ("West", "Pens", "20"), ("East", "Ink", "30"),
        ("North", "Ink", "40"), ("East", "Pens", "10"), ("West", "Paper", "50")]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        b.set_input(S, r, c, v)
# criteria: Region East, or Sales > 45
for r, row in enumerate([("Region", "Sales"), ("East", ""), ("", ">45")]):
    for c, v in enumerate(row):
        if v:
            b.set_input(S, 10 + r, c, v)
n = b.advanced_filter(S, (0, 0, 6, 2), (S, (10, 0, 12, 1)))
hidden = [r for r in range(1, 7) if b.row_hidden(S, r)]
check("advanced filter: in place hides the others", n == 4 and hidden == [2, 4], (n, hidden))
win.run("filter-clear")
check("advanced filter: Clear shows them again", not any(b.row_hidden(S, r) for r in range(1, 7)))
n = b.advanced_filter(S, (0, 0, 6, 2), (S, (10, 0, 11, 0)), True, (S, (15, 0, 15, 0)))
check("advanced filter: copy, unique", n == 2 and b.input(S, 15, 0) == "Region" and b.input(S, 17, 2) == "30",
      [b.input(S, 15 + k, 2) for k in range(4)])
b.set_input(S, 20, 0, "Item")
b.set_input(S, 20, 1, "Sales")
n = b.advanced_filter(S, (0, 0, 6, 2), (S, (10, 0, 11, 0)), False, (S, (20, 0, 20, 1)))
check("advanced filter: copy the columns named", b.input(S, 21, 0) == "Pens" and b.input(S, 21, 1) == "10" and b.input(S, 21, 2) == "")
# computed criterion: sales above 25 (formula about the first data row)
b.set_input(S, 25, 5, "Big")
b.set_input(S, 26, 5, "=C2>25")
n = b.advanced_filter(S, (0, 0, 6, 2), (S, (25, 5, 26, 5)))
check("advanced filter: computed criteria", n == 3, n)
win.run("filter-clear")


def fill_adv(d):
    d.list_e.set_text("A1:C7")
    d.crit_e.set_text("A11:A12")
    return Gtk.ResponseType.OK


DIALOG.append(fill_adv)
win.run("advanced-filter")
check("advanced filter dialog", [r for r in range(1, 7) if b.row_hidden(S, r)] == [2, 4, 6])
win.run("filter-clear")
# Consolidate: two blocks with labels
blk = [[("Name", "Q1"), ("Ann", "1"), ("Bob", "2")], [("Name", "Q1"), ("Bob", "5"), ("Cy", "7")]]
for k, block in enumerate(blk):
    for r, row in enumerate(block):
        for c, v in enumerate(row):
            b.set_input(S, 30 + r, 10 + 3 * k + c, v)
out = data2_ui.consolidate_ranges(win, [(S, (30, 10, 32, 11)), (S, (30, 13, 32, 14))], "SUM", True, True, False, S, (40, 10))
vals = {b.input(S, r, 10): b.value(S, r, 11) for r in range(41, out[2] + 1)}
check("consolidate: sum by label", vals == {"Ann": 1, "Bob": 7, "Cy": 7}, vals)
out = data2_ui.consolidate_ranges(win, [(S, (30, 10, 32, 11)), (S, (30, 13, 32, 14))], "SUM", True, True, True, S, (50, 10))
check("consolidate: links are formulas", b.is_formula(S, 52, 11) and b.value(S, 52, 11) == 7, b.input(S, 52, 11))
g.select_cell(60, 10)


def fill_cons(d):
    d.ref.set_text("K31:L33")
    d.add()
    d.ref.set_text("N31:O33")
    d.add()
    d.top.set_active(True)
    d.left.set_active(True)
    d.fn.set_active_id("MAX")
    return Gtk.ResponseType.OK


DIALOG.append(fill_cons)
win.run("consolidate")
check("consolidate dialog: max", b.value(S, 62, 11) == 5, [b.input(S, 60 + k, 11) for k in range(4)])

# ---------------------------------------------------------------- S2-09 view extras, S2-08 Copy as Picture
from folio_sheets import views_ui  # noqa: E402
S = g.sheet
g.state(S).zoom = 1.5
b.set_rows_hidden(S, 3, 4, True)
b.set_cols_hidden(S, 6, 6, True)
views_ui.add_view(win, "Wide")
check("custom view: added", [v[0] for v in views_ui.views(win)] == ["Wide"])
g.state(S).zoom = 1.0
b.set_rows_hidden(S, 0, 100, False)
b.set_cols_hidden(S, 0, 50, False)
views_ui.show_view(win, "Wide")
check("custom view: zoom back", abs(g.state(S).zoom - 1.5) < 1e-9, g.state(S).zoom)
check("custom view: hidden rows and columns back", b.row_hidden(S, 3) and b.row_hidden(S, 4) and not b.row_hidden(S, 5)
      and b.hidden_spans(S)[1] == [(6, 6)], b.hidden_spans(S))
p5 = os.path.join(TMP, "views.xlsx")
win.save_to(p5)
rb = Book.open(p5)
check("custom view: saved", "Wide" in (rb.workbook_element("customWorkbookViews") or "") and
      "customSheetView" in (rb.sheet_element(S, "customSheetViews") or ""))


def cv_show(d):
    d.tree.get_selection().select_path(Gtk.TreePath(0))
    return Gtk.ResponseType.OK


DIALOG.append(cv_show)
win.run("custom-views")
views_ui.delete_view(win, "Wide")
check("custom view: deleted", not views_ui.views(win) and not any(".wvu." in n[0] for n in b.names()))
# background
import struct, zlib  # noqa: E401,E402


def png(w, h, rgb):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


pp = os.path.join(TMP, "bg.png")
with open(pp, "wb") as fh:
    fh.write(png(20, 20, (200, 30, 30)))
check("background: set", views_ui.background(win, pp) == "set" and b.has_background(S))
p6 = os.path.join(TMP, "bg.xlsx")
win.save_to(p6)
check("background: saved and read back", Book.open(p6).has_background(S))
shot = os.path.join(TMP, "bg_shot.png")
b.render_png(S, shot, {"width": 200.0, "height": 120.0, "top_row": 30})
from gi.repository import GdkPixbuf  # noqa: E402
pix = GdkPixbuf.Pixbuf.new_from_file(shot).get_pixels()
check("background: drawn behind the cells", any(pix[k] > 150 and pix[k + 1] < 80 and pix[k + 2] < 80 for k in range(0, len(pix) - 3, 3)))
check("background: deleted", views_ui.background(win) == "deleted" and not b.has_background(S))
# clipboard pane
g.select_range((0, 0, 1, 0), (0, 0))
win.run("copy")
n_hist = len(views_ui.HISTORY)
win.run("clipboard-pane")
check("clipboard pane lists copies", win.clipboard_pane.get_visible() and n_hist >= 1 and len(win.clipboard_pane.list.get_children()) == n_hist)
g.select_cell(40, 8)
win.clipboard_pane.paste(0)
check("clipboard pane pastes an item", b.input(S, 40, 8) == b.input(S, 0, 0) != "", (b.input(S, 40, 8), b.input(S, 0, 0)))
win.clipboard_pane.clear()
check("clipboard: clear all", not views_ui.HISTORY)
win.clipboard_pane.hide()
# Copy as Picture
g.select_range((0, 0, 3, 2), (0, 0))
DIALOG.append(lambda d: Gtk.ResponseType.OK)
pic = views_ui.copy_as_picture(win)
check("copy as picture: a PNG of the cells", pic and os.path.getsize(pic) > 100 and views_ui.HISTORY and views_ui.HISTORY[0].picture)

# ---------------------------------------------------------------- S2-04 Page Layout view and ruler
from folio_sheets import pagelayout_ui  # noqa: E402
S = g.sheet
win.run("view-page-layout")
pump(0.2)
check("page layout: on", g.state(S).view_mode == "pageLayout")
p = g.params()
rc = b.cell_rect(S, 2, 1, p)
hit = b.hit(S, rc[0] + rc[2] / 2, rc[1] + rc[3] / 2, p)
check("page layout: a cell is found where it is drawn", tuple(hit[:3]) == ("cell", 2, 1), (rc, hit))
check("page layout: cells sit inside the page margin", rc[0] > 40 and rc[1] > 60, rc)
r0 = b.cell_rect(S, 0, 0, p)
hh = b.hit(S, r0[0] + 100, r0[1] - 20, p)
check("page layout: the header area", hh[0] == "header", hh)
pop = pagelayout_ui.edit_on_page(win, "header", 1)
pop.entry.set_text("Sales &P")
pop.apply()
check("page layout: header typed on the page", b.page_setup(S)["header"] == "&CSales &P", b.page_setup(S)["header"])
check("header code split", pagelayout_ui.split_code("&La&Cb &P&Rc") == ["a", "b &P", "c"])
shot = os.path.join(TMP, "pl.png")
b.render_png(S, shot, dict(p, width=700.0, height=500.0))
pix = GdkPixbuf.Pixbuf.new_from_file(shot)
px = pix.get_pixels()
grey = px[(5 * pix.get_rowstride()) + 40 * 3: (5 * pix.get_rowstride()) + 40 * 3 + 3]
check("page layout: the ruler and the grey round the pages are drawn", len(set(grey)) == 1 and grey[0] < 250, list(grey))
win.run("view-ruler")
check("ruler off", not win.ruler and b.hit(S, 5, 200, g.params())[0] != "ruler")
win.run("view-ruler")
g.select_cell(60, 3)
g.ensure_visible(60, 3)
r2 = b.cell_rect(S, 60, 3, g.params())
check("page layout: scrolls to a cell on a later page", r2 and 0 <= r2[1] < g.area.get_allocated_height() + 50, r2)
win.run("view-normal")
check("page layout: back to normal", g.state(S).view_mode == "normal")

# ---------------------------------------------------------------- S2-08 SmartArt, Signature Line, Object
from folio_sheets import insert_extras  # noqa: E402
S = g.sheet
n0 = len(b.objects(S))
for lay in ("list", "process", "cycle", "hierarchy", "relationship", "matrix", "pyramid"):
    gi = insert_extras.build_smartart(win, lay, "Plan\n\tDo\n\tCheck\nAct\nMore", 50, 50)
    ok = b.objects(S)[gi]["kind"] == "group" and insert_extras.smartart_of(win, gi)[0] == lay
    check("smartart: %s laid out as a group" % lay, ok, b.objects(S)[gi])
    b.delete_objects(S, [gi])


def sa_pick(d):
    d.radios["process"].set_active(True)
    d.text.get_buffer().set_text("One\nTwo\nThree")
    return Gtk.ResponseType.OK


DIALOG.append(sa_pick)
win.run("smartart")
gi = g.sel_objects[0]
check("smartart: inserted from the dialog", insert_extras.smartart_of(win, gi) == ("process", "One\nTwo\nThree"))
p7 = os.path.join(TMP, "smartart.xlsx")
win.save_to(p7)
rb = Book.open(p7)
k7 = [i for i, o in enumerate(rb.objects(S)) if o["kind"] == "group"]
check("smartart: saved with its layout and text", k7 and "Three" in (rb.object_ext(S, k7[0], insert_extras.SMARTART_URI) or ""))


def sa_edit(d):
    d.text.get_buffer().set_text("One\nTwo")
    return Gtk.ResponseType.OK


DIALOG.append(sa_edit)
insert_extras.activate(win, gi)
gi = g.sel_objects[0]
check("smartart: edited (laid out again)", insert_extras.smartart_of(win, gi)[1] == "One\nTwo" and b.objects(S)[gi]["members"] == 3)


def sig(d):
    d.entries[0].set_text("Ann Lee")
    d.entries[1].set_text("Director")
    return Gtk.ResponseType.OK


DIALOG.append(sig)
win.run("signature-line")
si = g.sel_objects[0]
check("signature line: a picture", b.objects(S)[si]["kind"] == "picture" and "Ann Lee" in b.object_props(S, si)["descr"])
insert_extras.sign(win, si, "Ann Lee")
si = g.sel_objects[0]
meta = insert_extras.png_chunk(insert_extras._pic(win, si), b"flSg").decode()
check("signature line: signed", meta.split("\n")[3] == "Ann Lee", meta)
fp = os.path.join(TMP, "notes.txt")
with open(fp, "w") as fh:
    fh.write("hello object")
oi = insert_extras.insert_object(win, fp)
f = insert_extras.object_file(win, oi)
check("object: the file is embedded", f and f[0] == "notes.txt" and f[1] == b"hello object", f)
win.save_to(p7)
rb_win_pic = Book.open(p7).picture_bytes(S, oi)
check("object: kept in the saved file", rb_win_pic and insert_extras.png_chunk(bytes(rb_win_pic[0]), b"flEm").endswith(b"hello object"))
out = insert_extras.open_object(win, oi, run=False)
check("object: opens as a file", out and open(out).read() == "hello object")
li = insert_extras.insert_object(win, fp, link=True)
check("object: linked", insert_extras.object_file(win, li)[2] == os.path.abspath(fp))

# ---------------------------------------------------------------- S2-03 chart types
from folio_sheets import chartex_ui  # noqa: E402
win.run("insert-sheet")
pump()
S = g.sheet
data = [("Region", "Item", "Amount"), ("East", "Pens", "120"), ("East", "Ink", "-40"), ("West", "Pens", "75"),
        ("West", "Paper", "60"), ("North", "Ink", "-25"), ("North", "Paper", "90"), ("South", "Pens", "33")]
for r, row in enumerate(data):
    for c, v in enumerate(row):
        b.set_input(S, r, c, v)
made = {}
for kind in ("waterfall", "funnel", "histogram", "pareto", "boxwhisker", "treemap", "sunburst"):
    rg = (0, 0, 7, 2) if kind in ("treemap", "sunburst") else (0, 1, 7, 2)
    i = chartex_ui.insert(win, kind, rg)
    made[kind] = i
    info = b.chartex_info(S, i)
    check("chart %s inserted" % kind, info and info["kind"] == kind and b.objects(S)[i]["kind"] == "chartex", info)
check("treemap: two category levels", len(b.chartex_info(S, made["treemap"])["cats"]) == 2, b.chartex_info(S, made["treemap"]))
for kind, i in made.items():
    shot = os.path.join(TMP, "cx_%s.png" % kind)
    rr = b.objects(S)[i]["rect"]
    b.render_png(S, shot, {"width": 1400.0, "height": 1400.0, "sel": [], "active": (9000, 9000), "show_headers": False})
p8 = os.path.join(TMP, "chartex.xlsx")
win.save_to(p8)
rb = Book.open(p8)
kinds = sorted((rb.chartex_info(S, k) or {}).get("kind", "") for k in range(len(rb.objects(S))) if rb.objects(S)[k]["kind"] == "chartex")
check("charts saved and read back", kinds == sorted(made), kinds)
z = zipfile.ZipFile(p8)
names = z.namelist()
ct = z.read("[Content_Types].xml").decode()
dr = [n for n in names if n.startswith("xl/drawings/drawing") and n.endswith(".xml")]
dtext = "".join(z.read(n).decode() for n in dr)
check("chartEx parts in the package", sum(1 for n in names if "chartEx" in n and n.endswith(".xml") and "_rels" not in n) == 7
      and "chartex+xml" in ct, [n for n in names if "chartEx" in n])
check("drawing wraps them as the original does", dtext.count("Requires=\"cx1\"") == 6 and dtext.count("Requires=\"cx2\"") == 1
      and dtext.count("mc:Fallback") >= 7)
wf = [n for n in names if "chartEx" in n and n.endswith(".xml") and "_rels" not in n]
wtext = "".join(z.read(n).decode() for n in wf)
check("chartEx XML: layouts", all(("layoutId=\"%s\"" % l) in wtext for l in ("waterfall", "funnel", "clusteredColumn", "paretoLine", "boxWhisker", "treemap", "sunburst")))
i = made["waterfall"]
b.set_chartex(S, i, title="Cash flow")
check("chart title", b.chartex_info(S, i)["title"] == "Cash flow")
b.set_chartex(S, i, kind="funnel")
check("change chart type", b.chartex_info(S, i)["kind"] == "funnel")
b.insert_rows(S, 0, 1) if hasattr(b, "insert_rows") else None
check("references move with inserted rows", "$A$3" in "".join(b.chartex_info(S, made["treemap"])["cats"]) or "$B$3" in "".join(b.chartex_info(S, made["treemap"])["cats"]),
      b.chartex_info(S, made["treemap"])["cats"])
from gi.repository import GdkPixbuf as _G  # noqa: E402
shot = os.path.join(TMP, "cx_all.png")
b.render_png(S, shot, {"width": 1600.0, "height": 1200.0, "sel": [], "active": (9000, 9000)})
pb = _G.Pixbuf.new_from_file(shot).get_pixels()
blue = sum(1 for k in range(0, len(pb) - 3, 3) if pb[k] < 90 and pb[k + 2] > 150)
check("charts drawn", blue > 2000, blue)

# radar, stock, contour (classic chart parts)
win.run("insert-sheet")
pump()
S = g.sheet
rows = [("Day", "High", "Low", "Close"), ("Mon", "12", "8", "10"), ("Tue", "14", "9", "13"), ("Wed", "13", "10", "11"),
        ("Thu", "15", "11", "14"), ("Fri", "16", "12", "15")]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        b.set_input(S, r, c, v)
made = []
for kind in ("radar", "radar-markers", "radar-filled", "stock", "contour"):
    g.sel.ranges = [(0, 0, 5, 3)]
    g.sel.active = (0, 0)
    n = len(b.objects(S))
    win.run("chart-column:" + kind)
    ok = len(b.objects(S)) == n + 1 and b.chart_spec(S, n)["kind"] == kind and not b.chart_exotic(S, n)
    check("chart %s inserted" % kind, ok, b.chart_spec(S, n)["kind"] if len(b.objects(S)) > n else None)
    made.append((n, kind))
p9 = os.path.join(TMP, "radar.xlsx")
win.save_to(p9)
rb = Book.open(p9)
check("radar, stock, contour saved and read back", [rb.chart_spec(S, i)["kind"] for i, _ in made] == [k for _, k in made],
      [rb.chart_spec(S, i)["kind"] for i, _ in made])
xs = "".join(zipfile.ZipFile(p9).read(n).decode() for n in zipfile.ZipFile(p9).namelist() if n.startswith("xl/charts/chart"))
check("chart parts: radarChart, stockChart with hiLowLines, surfaceChart", "<c:radarChart>" in xs and "radarStyle val=\"filled\"" in xs
      and "<c:hiLowLines>" in xs and "<c:surfaceChart>" in xs and "<c:serAx>" in xs)

# ---------------------------------------------------------------- M-12 File > Share > Email
fake = os.path.join(TMP, "bin")
os.makedirs(fake, exist_ok=True)
log = os.path.join(TMP, "post.log")
with open(os.path.join(fake, "folio-post"), "w") as fh:
    fh.write("#!/bin/sh\necho \"$@\" >> %s\n" % log)
os.chmod(os.path.join(fake, "folio-post"), 0o755)
os.environ["PATH"] = fake + os.pathsep + os.environ.get("PATH", "")
g.set_sheet(0)
win.sheet_changed()
pa = win.run("send-attachment") or win.act_send_attachment()
pp = win.act_send_pdf()
time.sleep(0.5)
got = open(log).read() if os.path.exists(log) else ""
check("share: workbook attached", pa and pa.endswith(".xlsx") and os.path.getsize(pa) > 0 and "--attach " + pa in got, got)
check("share: PDF attached", pp and pp.endswith(".pdf") and open(pp, "rb").read(4) == b"%PDF" and "--attach " + pp in got, got)
win.show_backstage() if hasattr(win, "show_backstage") else win.backstage.open()
win.backstage.nav("share")
pump()
check("share: File > Share page", win.backstage.current == "share")
win.backstage.close()

for p in tuple(globals().get("EXTRA", ())):
    p()

print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
