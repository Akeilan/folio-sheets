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


from folio_sheets import symbol_ui as SY  # noqa: E402
from folio_sheets import screenshot_ui as SC  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
win.resize(1280, 800)
pump(0.4)
g = win.grid
b = win.book
S = g.sheet
win.app.save_settings = lambda: None

# ---- Insert › Symbol
g.sel.ranges = [(0, 0, 0, 0)]
g.sel.active = (0, 0)
win.run("symbol")
pump(0.3)
d = win._symbol_dialog
check("Insert › Symbol opens the Symbol dialog", d is not None and d.get_title() == "Symbol")
check("with fonts and subsets", d.font.get_active_id() == "(normal text)" and d.subset.get_active_id() is not None)
d.subset.set_active_id("Greek and Coptic")
pump()
cp = d.grid.chars[d.grid.sel]
check("choosing a subset goes to it", 0x370 <= cp <= 0x3FF, hex(cp))
d.code.set_text("03B1")
pump()
check("typing a character code selects it", chr(d.grid.chars[d.grid.sel]) == "α")
check("and shows its Unicode name", d.name_lbl.get_text() == "Greek Small Letter Alpha", d.name_lbl.get_text())
d.insert_btn.clicked()
pump()
check("Insert types it in the cell", g.editing() and g.editor.text() == "α", g.editor.text() if g.editing() else None)
check("Cancel becomes Close", d.close_btn.get_label() == "Close")
check("it goes to the front of Recently used symbols", d.recent()[0] == "α")
d.base.set_active_id("dec")
pump()
check("from: ASCII (decimal) shows the decimal code", d.code.get_text() == "945", d.code.get_text())
d.base.set_active_id("hex")
d.grid.select(d.grid.index_of(ord("β")))
d.insert()
check("a second one goes at the cursor", g.editor.text() == "αβ")
d.nb.set_current_page(1)
pump()
m = d.special.get_model()
names = [row[1] for row in m]
check("Special Characters: em dash, copyright, ellipsis, ...", "Em Dash" in names and "Copyright" in names and "Ellipsis" in names)
d.special.get_selection().select_path(Gtk.TreePath(names.index("Em Dash")))
d.insert()
check("Special Characters insert too", g.editor.text() == "αβ—")
d.response(Gtk.ResponseType.CANCEL)
pump()
g.commit_edit()
check("the cell keeps them", b.input(S, 0, 0) == "αβ—")
win.run("symbol")
pump(0.2)
d = win._symbol_dialog
d.grid.select(d.grid.index_of(0x20AC))
d.grid.on_key(d.grid, type("K", (), {"keyval": Gdk.KEY_Return})())
check("Enter in the grid inserts (into the active cell, editing it)", g.editing() and g.editor.text().endswith("€"))
d.destroy()
g.commit_edit()

# ---- Insert › Screenshot
pop = SC.menu(win)
labels = []


def walk(w):
    if isinstance(w, Gtk.Label):
        labels.append(w.get_text())
    if isinstance(w, Gtk.ModelButton):
        labels.append(w.get_property("text"))
    if isinstance(w, Gtk.Container):
        for c in w.get_children():
            walk(c)


walk(pop)
check("Screenshot ▾ has Available Windows and Screen Clipping", "Available Windows" in labels and "Screen Clipping" in labels, labels)
check("no other windows open: No Available Windows", pop.windows_shown > 0 or "No Available Windows" in labels)
pb = SC.window_picture(win.get_window().get_xid())
check("a window's picture can be taken", pb is not None and pb.get_width() > 100)

n0 = len(b.objects(S))
SC.screen_clipping(win)
pump(0.6)
cw = getattr(win, "_clip_window", None)
check("Screen Clipping hides the workbook and greys the screen", cw is not None and not win.get_visible() and cw.get_visible())


class Ev:
    def __init__(self, x, y):
        self.x, self.y = float(x), float(y)


cw.on_press(cw.area, Ev(100, 120))
cw.on_motion(cw.area, Ev(300, 220))
cw.on_release(cw.area, Ev(300, 220))
pump(0.4)
objs = b.objects(S)
check("dragging out a part puts it on the sheet as a picture", len(objs) == n0 + 1 and objs[-1].get("kind") == "picture", objs[-1:])
check("and the workbook comes back", win.get_visible())
SC.screen_clipping(win)
pump(0.6)
cw = win._clip_window
cw.on_key(cw.area, type("K", (), {"keyval": Gdk.KEY_Escape})())
pump(0.3)
check("Esc cancels the clipping", len(b.objects(S)) == n0 + 1 and win.get_visible())

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
