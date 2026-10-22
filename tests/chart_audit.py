# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): inserting, moving, sizing, changing,
# deleting and saving charts, through the ribbon commands, the dialogs and the mouse.
import os
import sys
import time
import traceback

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


SCRIPT = []


def fake_run(d):
    pump(0.05)
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    return fn(d)


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run


def find(w, name):
    if w.get_name() == name:
        return w
    if isinstance(w, Gtk.Container):
        for c in w.get_children():
            r = find(c, name)
            if r is not None:
                return r
    return None


g = win.grid
b = win.book
S = 0
data = [["", "North", "South", "East"], ["Q1", "10", "20", "15"], ["Q2", "15", "25", "18"],
        ["Q3", "12", "30", "22"], ["Q4", "18", "28", "26"]]
for r, row in enumerate(data):
    for c, v in enumerate(row):
        if v:
            b.set_input(S, r, c, v)
g.sel.ranges = [(1, 1, 1, 1)]
g.sel.active = (1, 1)
g.refresh()
pump(0.1)

# ---- insert from the ribbon menu: a single cell charts its whole region
win.run("chart-column:col")
pump(0.1)
objs = b.objects(S)
check("a chart was inserted", len(objs) == 1 and objs[0]["kind"] == "chart", objs)
check("the region around the cell is charted", b.chart_range_text(S, 0) == "Sheet1!$A$1:$D$5", b.chart_range_text(S, 0))
check("series down the columns, named by the header", [s["name"] for s in b.chart_spec(S, 0)["series"]] ==
      ["Sheet1!$B$1", "Sheet1!$C$1", "Sheet1!$D$1"], b.chart_spec(S, 0)["series"])
check("the new chart is selected", g.sel_objects == [0])
dt = win.ribbon.tab_by_name("Chart Design")
check("Chart Design tab shows and is current", dt.button.get_visible() and win.ribbon.current is dt)
check("one undo step", b.undo_labels()[0] == "Insert Column or Bar Chart" and b.undo_labels()[1] == "Typing", b.undo_labels()[:2])

# ---- the mouse: move by dragging the body
p = g.params()
x, y, w, h = b.object_screen_rect(S, 0, p)
check("the chart is on screen", w > 300 and h > 150, (x, y, w, h))
check("clicking its middle hits it", b.object_hit(S, x + w / 2, y + h / 2, p) == (0, "body"))
check("its corner is a handle", b.object_hit(S, x + w, y + h, p) == (0, "se"))
before = b.objects(S)[0]["rect"]


class Ev:
    def __init__(self, x, y, button=1, kind=Gdk.EventType.BUTTON_PRESS, state=0):
        self.x, self.y, self.button, self.type, self.state = x, y, button, kind, state

    def get_state(self):
        return self.state


g.on_press(g.area, Ev(x + w / 2, y + h / 2))
g._drag_to(x + w / 2 + 40, y + h / 2 + 60)
check("dragging shows the outline", g.obj_preview is not None and abs(g.obj_preview[0] - (x + 40)) < 1, g.obj_preview)
g.on_release(g.area, Ev(x + w / 2 + 40, y + h / 2 + 60))
pump(0.05)
after = b.objects(S)[0]["rect"]
check("dropping moves it", abs(after[0] - before[0] - 40) < 1.5 and abs(after[1] - before[1] - 60) < 1.5, (before, after))
check("moved as one step", b.undo_labels()[0] == "Move Chart")
# size from the bottom-right handle
x, y, w, h = b.object_screen_rect(S, 0, g.params())
g.on_press(g.area, Ev(x + w, y + h))
g._drag_to(x + w - 100, y + h - 50)
g.on_release(g.area, Ev(x + w - 100, y + h - 50))
r2 = b.objects(S)[0]["rect"]
check("the handle sizes it", abs(r2[2] - (after[2] - 100)) < 1.5 and abs(r2[3] - (after[3] - 50)) < 1.5, (after, r2))
check("sized as one step", b.undo_labels()[0] == "Size Chart")
win.run("undo")
check("undo puts the size back", abs(b.objects(S)[0]["rect"][2] - after[2]) < 1.5)

# ---- clicking a cell deselects, the tab goes
x, y, w, h = b.object_screen_rect(S, 0, g.params())
cx, cy, cw, ch = b.cell_rect(S, 0, 0, g.params())
g.on_press(g.area, Ev(cx + 5, cy + 5))
g.on_release(g.area, Ev(cx + 5, cy + 5))
pump(0.05)
check("a cell click deselects the chart", g.sel_objects == [] and not dt.button.get_visible())
g.on_press(g.area, Ev(x + w / 2, y + h / 2))
g.on_release(g.area, Ev(x + w / 2, y + h / 2))
check("a click selects it again", g.sel_objects == [0] and dt.button.get_visible())

# ---- Chart Design commands
win.run("chart-switch-row-col")
check("Switch Row/Column: series by quarter", len(b.chart_spec(S, 0)["series"]) == 4 and
      b.chart_spec(S, 0)["series"][0]["name"] == "Sheet1!$A$2", b.chart_spec(S, 0)["series"])
win.run("chart-add-element:legend:r")
check("legend to the right", b.chart_spec(S, 0)["legend"] == "r")
win.run("chart-add-element:axis-title-y:on")
check("vertical axis title", b.chart_spec(S, 0)["y_title"] == "Axis Title")
win.run("chart-add-element:labels:on")
check("data labels", b.chart_spec(S, 0)["labels"])
win.run("chart-colors:2")
check("monochrome colours", b.chart_spec(S, 0)["palette"] == 2)


# Change Chart Type dialog: Line page, Line with Markers
def pick_line(d):
    pages = find(d, "pages")
    for row in pages.get_children():
        if row.page == "Line":
            pages.select_row(row)
    pump(0.05)
    for rb in find(d, "subtypes").get_children():
        btn = rb.get_child() if isinstance(rb, Gtk.FlowBoxChild) else rb
        if getattr(btn, "kind", None) == "line-markers":
            btn.set_active(True)
    pump(0.05)
    return Gtk.ResponseType.OK


SCRIPT.append(pick_line)
win.run("chart-change-type")
check("Change Chart Type to line with markers", b.chart_spec(S, 0)["kind"] == "line-markers", b.chart_spec(S, 0)["kind"])
check("settings survive a type change", b.chart_spec(S, 0)["legend"] == "r" and b.chart_spec(S, 0)["labels"])


# Format Chart dialog: custom title, no gridlines
def fmt(d):
    find(d, "title-mode").set_active_id("text")
    find(d, "title").set_text("Sales by region")
    find(d, "gridlines").set_active(False)
    find(d, "legend").set_active_id("b")
    return Gtk.ResponseType.OK


SCRIPT.append(fmt)
win.run("chart-format")
sp = b.chart_spec(S, 0)
check("Format Chart: title, gridlines, legend", sp["title"] == "Sales by region" and not sp["gridlines"] and sp["legend"] == "b", sp)
check("the title shows", b.chart_title(S, 0) == "Sales by region")


# Select Data: a smaller range
def data_range(d):
    find(d, "range").set_text("=Sheet1!$A$1:$C$5")
    return Gtk.ResponseType.OK


SCRIPT.append(data_range)
win.run("chart-select-data")
check("Select Data: two series", len(b.chart_spec(S, 0)["series"]) == 2, b.chart_spec(S, 0)["series"])
check("the type is kept", b.chart_spec(S, 0)["kind"] == "line-markers")


# Insert Chart dialog (combo, last series on the secondary axis)
def combo(d):
    pages = find(d, "pages")
    for row in pages.get_children():
        if row.page == "Combo":
            pages.select_row(row)
    pump(0.05)
    find(d, "secondary-2").set_active(True)
    pump(0.05)
    return Gtk.ResponseType.OK


g.deselect_objects()
g.sel.ranges = [(0, 0, 4, 3)]
g.sel.active = (0, 0)
SCRIPT.append(combo)
win.run("charts-dialog")
check("Insert Chart dialog: a combo chart", len(b.objects(S)) == 2 and b.chart_spec(S, 1)["kind"] == "combo", b.chart_spec(S, 1))
sp = b.chart_spec(S, 1)
check("combo: columns then a line on the secondary axis", [x["kind"] for x in sp["series"]] == ["col", "col", "line"]
      and sp["series"][2]["secondary"], sp["series"])

# ---- inserting rows moves the chart and its data with it
r0 = b.objects(S)[0]["rect"]
g.deselect_objects()
b.insert_rows(S, 0, 2)
check("charts move down with inserted rows", b.objects(S)[0]["rect"][1] > r0[1])
check("their data follows", b.chart_range_text(S, 0) == "Sheet1!$A$3:$C$7", b.chart_range_text(S, 0))
win.run("undo")

# ---- delete with the key, undo brings it back
g.select_object(1)
win.on_object_selection()
g.area.grab_focus()
pump(0.05)
class Key:
    keyval = Gdk.KEY_Delete

    def get_state(self):
        return Gdk.ModifierType(0)


win.set_focus(g.area)
win.on_key(win, Key())
check("Delete removes the selected chart", len(b.objects(S)) == 1)
win.run("undo")
check("undo brings it back", len(b.objects(S)) == 2)

# ---- save and open again
path = "/tmp/chart_audit.xlsx"
win.save_to(path)
from folio_sheets import core
b2 = core.Book.open(path)
check("saved charts open again", [o["kind"] for o in b2.objects(0)] == ["chart", "chart"])
check("their settings were saved", b2.chart_spec(0, 0)["title"] == "Sales by region" and b2.chart_spec(0, 1)["kind"] == "combo",
      (b2.chart_spec(0, 0), b2.chart_spec(0, 1)["kind"]))

# a picture of the sheet with the charts
rr = b.objects(S)[1]["rect"]
b.set_object_rect(S, 1, rr[0] + 520, 20.0, rr[2], rr[3])
g.select_object(0)
pump(0.2)
shot = os.environ.get("FOLIO_SHOT", "/tmp/chart_audit.png")
b.render_png(S, shot, g.params({"width": 1400.0, "height": 800.0}))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
