# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): View > Window — New Window on the same
# workbook, Arrange All, View Side by Side with Synchronous Scrolling, Hide and Unhide.
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


SCRIPT = []
SEEN = []


def fake_run(d):
    pump(0.03)
    text = d.get_property("text") if isinstance(d, Gtk.MessageDialog) else None
    SEEN.append((d.get_title(), text))
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

from folio_sheets import windows_ui  # noqa: E402

w1 = win
w1.unmaximize()
pump(0.4)
b = w1.book
b.set_input(0, 0, 0, "hello")
w1.grid.select_range((5, 2, 5, 2), (5, 2))
w1.grid.state().top_row = 3
pump(0.1)

# ---- New Window
w2 = w1.run("new-window") or app.windows[-1]
pump(0.5)
check("New Window opens a second window", len(app.windows) == 2 and w2 is not w1)
check("on the same workbook", w2.book is w1.book)
check("numbered in the title", w1.doc_title() == "Book1:1" and w2.doc_title() == "Book1:2",
      (w1.doc_title(), w2.doc_title()))
check("showing the same place", tuple(w2.grid.sel.active) == (5, 2) and w2.grid.state().top_row == 3)
w1.grid.select_range((1, 0, 1, 0), (1, 0))
w1.grid.start_edit(initial="typed in one")
w1.grid.commit_edit()
pump(0.5)
check("what is typed in one window is in the other", w2.book.display(0, 1, 0) == "typed in one")
w2.grid.select_range((1, 0, 1, 0), (1, 0))
pump(0.05)
check("and its formula bar shows it", w2.formula_bar.text() == "typed in one", w2.formula_bar.text())
n = w1.book.sheet_count
w1.run("insert-sheet")
pump(0.5)
check("a sheet added in one appears in the other's tabs", w2.book.sheet_count == n + 1 and
      len(w2.tabs.tabs if hasattr(w2.tabs, "tabs") else range(w2.book.sheet_count)) == n + 1)
w1.grid.set_sheet(0)
w1.sheet_changed()
w2.grid.set_sheet(0)
w2.sheet_changed()
pump(0.05)
check("each window keeps its own sheet and scroll", w1.grid.state() is not w2.grid.state())

# ---- Arrange All
SCRIPT.append(lambda d: ([rb.set_active(True) for rb, k in d.radios if k == "vertical"], Gtk.ResponseType.OK)[1])
placed = w1.run("arrange-all")
pump(0.3)
x1, y1, wd1, h1 = w1.arranged
x2, y2, wd2, h2 = w2.arranged
check("Arrange All > Vertical puts them side by side", y1 == y2 and x2 == x1 + wd1 and wd1 == wd2, (w1.arranged, w2.arranged))
check("the active window first", w1.arranged[0] < w2.arranged[0])
windows_ui.arrange(w1, "horizontal")
check("Horizontal stacks them", w1.arranged[0] == w2.arranged[0] and w2.arranged[1] == w1.arranged[1] + w1.arranged[3])
windows_ui.arrange(w1, "cascade")
check("Cascade staggers them", w2.arranged[0] > w1.arranged[0] and w2.arranged[1] > w1.arranged[1])
windows_ui.arrange(w1, "tiled")
check("Tiled shares the screen", w1.arranged[2] == w2.arranged[2] and w1.arranged[1] == w2.arranged[1])

# ---- View Side by Side, Synchronous Scrolling
w1.run("side-by-side")
pump(0.3)
check("View Side by Side pairs the windows", w1.side_partner is w2 and w2.side_partner is w1)
check("one above the other", w1.arranged[0] == w2.arranged[0] and w2.arranged[1] > w1.arranged[1])
check("scrolling together", w1.sync_scroll and w2.sync_scroll)


def checked(w, key):
    for c in w.ribbon.controls:
        if getattr(c, "state_key", None) == key:
            return c.get_style_context().has_class("checked")
    return None


check("both buttons show as on", checked(w1, "side-by-side") and checked(w1, "sync-scroll"))
w1.grid.area.queue_draw()
pump(0.1)
t2 = w2.grid.state().top_row
w1.grid.scroll_by(drows=10)
pump(0.2)
check("scrolling one scrolls the other as far", w2.grid.state().top_row == t2 + 10, (t2, w2.grid.state().top_row))
w2.grid.scroll_by(dcols=3)
pump(0.2)
check("both ways, across too", w1.grid.state().left_col == 3, w1.grid.state().left_col)
w1.run("sync-scroll")
pump(0.05)
t2 = w2.grid.state().top_row
w1.grid.scroll_by(drows=5)
pump(0.2)
check("Synchronous Scrolling off lets them go their own way", w2.grid.state().top_row == t2 and
      not checked(w1, "sync-scroll"))
w1.run("sync-scroll")
place = (w1.arranged, w2.arranged)
w1.resize(400, 300)
w1.run("reset-window")
pump(0.1)
check("Reset Window Position shares the screen again", (w1.arranged, w2.arranged) == place)
w1.run("side-by-side")
check("pressing View Side by Side again ends it", w1.side_partner is None and w2.side_partner is None and
      not checked(w1, "side-by-side"))

# ---- Hide and Unhide
w2.run("hide-window")
pump(0.1)
check("Hide hides the window", not w2.get_visible() and w1.get_visible())
SEEN.clear()
w1.run("hide-window")
check("the only window showing can't be hidden", w1.get_visible() and "only window" in (SEEN[-1][1] or ""), SEEN)
SCRIPT.append(lambda d: Gtk.ResponseType.OK)
shown = w1.run("unhide-window")
pump(0.1)
check("Unhide lists hidden windows and shows the one picked", SEEN[-1][0] == "Unhide" and w2.get_visible(), SEEN[-1:])

# ---- saving and closing
path = os.path.join(tempfile.mkdtemp(), "shared.xlsx")
w1.save_to(path)
check("both windows carry the saved name", w2.doc_title() == "shared:2" and w1.doc_title() == "shared:1",
      (w1.doc_title(), w2.doc_title()))
w1.book.set_input(0, 9, 9, "unsaved")
SEEN.clear()
w2.act_close()
pump(0.2)
check("closing one of two windows doesn't ask to save", not SEEN and len(app.windows) == 1, SEEN)
pump(0.2)
check("and the other goes back to its plain name", w1.doc_title() == "shared", w1.doc_title())
check("with the workbook still there", w1.book.display(0, 9, 9) == "unsaved")

# ---- Split
from gi.repository import Gdk  # noqa: E402
g = w1.grid
g.set_sheet(0)
w1.sheet_changed()
st = g.state()
st.top_row, st.left_col = 0, 0
g.select_range((10, 3, 10, 3), (10, 3))
pump(0.1)
w1.run("split")
pump(0.1)
check("Split splits at the active cell", st.split and st.freeze_rows == 10 and st.freeze_cols == 3 and
      st.pane_top == 0, (st.split, st.freeze_rows, st.freeze_cols))
geo = g.geometry()
check("into four panes", len(geo["row_spans"]) == 2 and len(geo["col_spans"]) == 2)


class _Scroll:
    def __init__(self, x, y, dy):
        self.x, self.y, self.direction = x, y, Gdk.ScrollDirection.DOWN if dy > 0 else Gdk.ScrollDirection.UP

    def get_state(self):
        return 0

    def get_scroll_deltas(self):
        return (False, 0, 0)


sy, sx = g.split_at()
g.on_scroll(g.area, _Scroll(sx + 40, sy + 40, 1))
pump(0.05)
check("the bottom pane scrolls on its own", st.top_row > 10 and st.pane_top == 0, (st.top_row, st.pane_top))
bottom = st.top_row
g.on_scroll(g.area, _Scroll(sx + 40, sy - 30, 1))
pump(0.05)
check("so does the top pane", st.pane_top > 0 and st.freeze_rows - st.pane_top == 10 and st.top_row >= st.freeze_rows,
      (st.pane_top, st.freeze_rows, st.top_row))
rows = [r[0] for r in g.geometry()["rows"]]
check("the top pane shows its own rows", rows[0] == st.pane_top and st.freeze_rows - 1 in rows, rows[:3])
g.select_cell(0, 0)
pump(0.05)
check("going above the top pane brings it back", st.pane_top == 0, st.pane_top)
check("a cell in the top pane has a place on screen", b.cell_rect(0, 2, 1, g.params()) is not None)


class _Ev:
    def __init__(self, x, y, kind=Gdk.EventType.BUTTON_PRESS):
        self.x, self.y, self.button, self.type, self.state = x, y, 1, kind, 0

    def get_state(self):
        return 0


sy, sx = g.split_at()
r5 = b.cell_rect(0, 5, 0, g.params())
g.on_press(g.area, _Ev(sx + 50, sy))
g._drag_to(sx + 50, r5[1] + r5[3] / 2 + 3)
g.on_release(g.area, _Ev(sx + 50, r5[1] + r5[3] + 1))
pump(0.05)
check("dragging the bar moves the split", st.split and st.freeze_rows - st.pane_top == 6, (st.pane_top, st.freeze_rows))
w1.store_view_state()
check("a split isn't saved as frozen panes", b.sheet_info(0)["freeze_rows"] == 0, b.sheet_info(0).get("freeze_rows"))
w1.run("freeze-panes")
check("Freeze Panes turns the split into frozen panes there", not st.split and st.freeze_rows == 6 and st.freeze_cols == 3,
      (st.split, st.freeze_rows, st.freeze_cols))
w1.run("freeze-panes")
w1.run("split")
w1.run("split")
check("Split again takes it away", not st.split and st.freeze_rows == 0 and st.freeze_cols == 0)
g.select_range((0, 0, 0, 0), (0, 0))
w1.run("split")
check("from the top-left cell it splits in the middle", st.split and st.freeze_rows > 3 and st.freeze_cols > 2,
      (st.freeze_rows, st.freeze_cols))
sy, sx = g.split_at()
g.on_press(g.area, _Ev(sx + 50, sy, Gdk.EventType._2BUTTON_PRESS))
pump(0.05)
check("double-clicking a bar removes the split", not st.split)

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
