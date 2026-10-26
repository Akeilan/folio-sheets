# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): SH-21 sparklines — Insert › Sparklines, drawing in
# cells, the Sparkline tab (type, show points, colours, group, clear), undo, saving.
import os
import sys
import tempfile
import time
import traceback

import cairo
from gi.repository import Gdk, Gtk

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


from folio_sheets.core import Book  # noqa: E402
from folio_sheets import sparkline_ui as SP  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
win.resize(1280, 800)
pump(0.4)
g = win.grid
area = g.area
b = win.book
S = g.sheet


def grab():
    pump(0.2)
    w = area.get_window()
    img = cairo.ImageSurface(cairo.FORMAT_ARGB32, w.get_width(), w.get_height())
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, w, 0, 0)
    cr.paint()
    img.flush()
    return img


def colored_in(img, r, c, pred):
    x, y, w, h = b.cell_rect(S, r, c, g.params())
    d = img.get_data()
    st = img.get_stride()
    n = 0
    for yy in range(int(y) + 2, int(y + h) - 2):
        for xx in range(int(x) + 2, int(x + w) - 2):
            o = yy * st + xx * 4
            if pred(d[o + 2], d[o + 1], d[o]):
                n += 1
    return n


blue = lambda r, gg, bb: bb > 100 and r < 110 and bb - r > 40  # noqa: E731
red = lambda r, gg, bb: r > 170 and gg < 80 and bb < 80  # noqa: E731

rows = [[3, 5, 2, 8, 6], [-2, 4, -1, 3, 5], [1, 1, 1, 1, 1], [7, 2, 9, 4, 10]]
for r, vals in enumerate(rows):
    for c, v in enumerate(vals):
        b.set_input(S, r, c, str(v))
g.refresh()

# ---- the dialog, filled from the selection
g.sel.ranges = [(0, 0, 3, 4)]
g.sel.active = (0, 0)
seen = {}


def fake_run(self):
    seen["data"] = self.data.get_text()
    seen["loc"] = self.loc.get_text()
    seen["title"] = self.get_title()
    self.loc.set_text(seen.get("set_loc", self.loc.get_text()))
    if "set_data" in seen:
        self.data.set_text(seen["set_data"])
    return Gtk.ResponseType.OK


SP.CreateDialog.run = fake_run
seen["set_loc"] = "F1:F4"
ok = SP.create(win, "line")
check("Create Sparklines: the selected numbers are the data range", seen.get("data") == "A1:E4", seen)
check("Insert › Line puts one sparkline per row in the location", ok and len(b.sparklines(S)) == 1
      and len(b.sparklines(S)[0]["items"]) == 4, b.sparklines(S))
check("each sparkline reads its own row", [f for f, _ in b.sparklines(S)[0]["items"]][1].endswith("A2:E2"),
      b.sparklines(S)[0]["items"])
img = grab()
check("line sparklines are drawn in their cells", all(colored_in(img, r, 5, blue) > 8 for r in range(4)),
      [colored_in(img, r, 5, blue) for r in range(4)])

# a location that's not a row or column is refused, with Excel's message
seen.clear()
seen["set_loc"] = "G1:H4"
calls = {"n": 0}


def run_twice(self):
    calls["n"] += 1
    if calls["n"] > 1:
        return Gtk.ResponseType.CANCEL
    return fake_run(self)


SP.CreateDialog.run = run_twice
g.sel.ranges = [(0, 0, 3, 4)]
SP.create(win, "column")
check("a block of cells for the location is refused", INFO and "single row or column" in INFO[-1][1], INFO[-1:])
SP.CreateDialog.run = fake_run

# ---- an empty selection is where they go
seen.clear()
g.sel.ranges = [(5, 0, 5, 3)]
g.sel.active = (5, 0)
seen["set_data"] = "A1:D4"
SP.create(win, "column")
check("an empty selection fills the Location Range", seen.get("loc") == "A6:D6" and seen.get("data") == "", seen)
check("column sparklines: one per column of the data", len(b.sparklines(S)) == 2
      and b.sparklines(S)[1]["kind"] == "column" and len(b.sparklines(S)[1]["items"]) == 4)

# ---- the Sparkline tab
g.sel.ranges = [(1, 5, 1, 5)]
g.sel.active = (1, 5)
win.on_selection_changed()
pump()
tab = win.ribbon.tab_by_name("Sparkline")
check("selecting a sparkline shows the Sparkline tab", tab is not None and tab.button.get_visible())
win.run("spark-type:column")
check("Type › Column changes the whole group", b.sparklines(S)[0]["kind"] == "column")
img = grab()
check("and they're drawn as columns", colored_in(img, 0, 5, blue) > 20, colored_in(img, 0, 5, blue))
win.run("spark-show:negative:1")
check("Show › Negative Points", b.sparklines(S)[0]["negative"] and win.spark_checks["negative"].get_active())
img = grab()
check("negative columns are drawn red", colored_in(img, 1, 5, red) > 4, colored_in(img, 1, 5, red))
win.spark_checks["high"].set_active(True)
pump()
check("ticking High Point in the ribbon shows it", b.sparklines(S)[0]["high"])
win.run("spark-color:00B050")
check("Sparkline Color", b.sparklines(S)[0]["color"] == 0x00B050)
win.run("spark-weight:2.25")
check("Sparkline Color › Weight", abs(b.sparklines(S)[0]["weight"] - 2.25) < 1e-9)
win.run("spark-marker:low_color:7030A0")
check("Marker Color › Low Point sets the colour and shows the low point",
      b.sparklines(S)[0]["low_color"] == 0x7030A0 and b.sparklines(S)[0]["low"])
win.run("spark-type:winloss")
img = grab()
check("Win/Loss: wins up, losses down in red", b.sparklines(S)[0]["kind"] == "winloss"
      and colored_in(img, 1, 5, red) > 4, colored_in(img, 1, 5, red))
win.run("undo")
check("undo goes back a step", b.sparklines(S)[0]["kind"] == "column")
win.run("redo")

# ungroup / group
g.sel.ranges = [(0, 5, 3, 5)]
g.sel.active = (0, 5)
win.run("spark-ungroup")
check("Ungroup: a group per sparkline", len(b.sparklines(S)) == 5, len(b.sparklines(S)))
win.run("spark-group")
check("Group: one group again, with the first one's look", len(b.sparklines(S)) == 2
      and any(x["kind"] == "winloss" and len(x["items"]) == 4 for x in b.sparklines(S)))

# edit data
gi = b.sparkline_at(S, 0, 5)[0]
seen.clear()
seen["set_data"] = "A1:C4"
seen["set_loc"] = "F1:F4"
SP.edit_data(win)
info = b.sparklines(S)[b.sparkline_at(S, 0, 5)[0]]
check("Edit Data offers the group's data and cells", seen.get("data", "").endswith("A1:E4") and seen.get("loc") == "F1:F4", seen)
check("and remakes the group over the new data, keeping its look",
      info["items"][0][0].endswith("A1:C1") and info["kind"] == "winloss" and info["color"] == 0x00B050, info)

# moving with the cells
win.book.insert_rows(S, 0, 1)
check("inserting rows moves the sparklines and their data", b.sparkline_at(S, 1, 5) is not None
      and b.sparklines(S)[b.sparkline_at(S, 1, 5)[0]]["items"][0][0].endswith("A2:C2"))
win.run("undo")

# the axis button says it isn't available
INFO.clear()
win.run("spark-axis")
check("Axis says it isn't available", INFO and "Axis" in INFO[-1][0])

# saved and read back
path = os.path.join(tempfile.mkdtemp(), "spark.xlsx")
win.save_to(path)
again = Book.open(path)
sp = again.sparklines(0)
check("saved: the groups come back", len(sp) == 2 and sorted(x["kind"] for x in sp) == ["column", "winloss"], sp)
check("with their options", any(x["high"] and x["negative"] and x["color"] == 0x00B050 for x in sp))

# clear
g.sel.ranges = [(2, 5, 2, 5)]
g.sel.active = (2, 5)
win.on_selection_changed()
win.run("spark-clear")
check("Clear Selected Sparklines takes one", b.sparkline_at(S, 2, 5) is None and b.sparkline_at(S, 1, 5) is not None)
g.sel.active = (1, 5)
g.sel.ranges = [(1, 5, 1, 5)]
win.run("spark-clear-group")
check("Clear Selected Sparkline Groups takes the group", b.sparkline_at(S, 0, 5) is None)
win.on_selection_changed()
check("the Sparkline tab goes with them", not tab.button.get_visible())

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
