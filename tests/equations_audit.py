# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): SH-21 sparklines — Insert › Sparklines, drawing in
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
from folio_sheets import equation_ui as E  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
win.resize(1280, 800)
pump(0.4)
g = win.grid
area = g.area
b = win.book
S = g.sheet


def dark_pixels(i):
    pump(0.2)
    w = area.get_window()
    img = cairo.ImageSurface(cairo.FORMAT_ARGB32, w.get_width(), w.get_height())
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, w, 0, 0)
    cr.paint()
    img.flush()
    x, y, ww, hh = b.object_screen_rect(S, i, g.params())
    d = img.get_data()
    st = img.get_stride()
    n = 0
    for yy in range(int(y) + 2, int(y + hh) - 2):
        for xx in range(int(x) + 2, int(x + ww) - 2):
            o = yy * st + xx * 4
            if d[o] < 90 and d[o + 1] < 90 and d[o + 2] < 90:
                n += 1
    return n


# ---- the linear format
for name, lin in Book.equation_built_ins():
    check("built-in %s reads back the same" % name, Book.equation_normalize(lin) == Book.equation_normalize(Book.equation_normalize(lin)))
om = Book.equation_omml("\\frac{a}{b}+x^2")
check("LaTeX commands become Office Math", "<m:f>" in om and "<m:sSup>" in om, om)

# ---- Insert › Equation
win.run("equation")
pump(0.3)
ed = win.equation_editor
check("Insert › Equation adds an equation box and opens it for typing", ed is not None and len(b.objects(S)) == 1)
check("the Equation tab shows", win.ribbon.tab_by_name("Equation").button.get_visible())
check("an empty equation says Type equation here.", ed.entry.get_placeholder_text() == "Type equation here.")
ed.entry.set_text("x=")
ed.entry.set_position(-1)
win.run("equation-put:⬚/⬚")
check("Structures › Fraction goes in at the cursor, its first box selected for typing",
      ed.entry.get_text() == "x=⬚/⬚" and ed.entry.get_selection_bounds() == (2, 3), (ed.entry.get_text(), ed.entry.get_selection_bounds()))
ed.entry.set_text("x=(−b±√(b^2−4ac))/2a")
ed.entry.activate()
pump(0.2)
i = 0
check("Enter puts the equation in its box", b.equation_linear(S, i) == "x=(−b±√(b^2−4ac))/2a", b.equation_linear(S, i))
check("the editor closes", win.equation_editor is None)
check("the equation is drawn built up", dark_pixels(i) > 60, dark_pixels(i))
r1 = b.object_screen_rect(S, i, g.params())
check("its box fits the equation (a fraction is taller than a line of text)", r1[3] > 40 and r1[2] > 100, r1)

# editing it again: typing goes to the equation editor, not the text box editor
g.select_object(i)
win.on_object_selection()
win.shapes.edit(i, first="+1")
pump(0.1)
ed = win.equation_editor
check("typing on a selected equation edits it in the linear format", ed is not None and ed.entry.get_text().endswith("2a+1"))
ed.entry.set_text("a^2+b^2=c^2")
ed.finish()
check("a new equation replaces the old one", b.equation_linear(S, i) == "a^2+b^2=c^2")
win.run("undo")
check("and undo brings the old one back", b.equation_linear(S, i) == "x=(−b±√(b^2−4ac))/2a")
win.shapes.edit(i)
win.equation_editor.entry.set_text("zzz")
Ev = type("Ev", (), {"keyval": Gdk.KEY_Escape})
win.equation_editor.key(None, Ev())
check("Esc leaves it as it was", b.equation_linear(S, i) == "x=(−b±√(b^2−4ac))/2a")

# the built-in gallery
pop = E.equation_menu(win)
kids = pop.flow.get_children()
check("Equation ▾ offers the nine built-in equations", len(kids) == 9)
kids[1].get_child().clicked()
pump(0.2)
check("picking one inserts it", len(b.objects(S)) == 2 and "∑" in (b.equation_linear(S, 1) or ""), b.equation_linear(S, 1))
check("built-in equations are drawn", dark_pixels(1) > 60)

# symbols and structures with an equation selected but not open
g.select_object(1)
win.on_object_selection()
win.run("equation-put:≠")
pump(0.1)
check("a symbol with an equation selected opens it and adds the symbol", win.equation_editor is not None
      and win.equation_editor.entry.get_text().endswith("≠"))
win.equation_editor.finish()
_orig_run = Gtk.Dialog.run
_seen_ink = []
Gtk.Dialog.run = lambda d: (_seen_ink.append(d.get_title()), Gtk.ResponseType.CANCEL)[1]
win.run("equation-ink")
Gtk.Dialog.run = _orig_run
check("Ink Equation opens the Math Input Control (S3-12)", _seen_ink == ["Math Input Control"], _seen_ink)

# saved as Office keeps it, and read back
path = os.path.join(tempfile.mkdtemp(), "eq.xlsx")
win.save_to(path)
import zipfile  # noqa: E402
d = zipfile.ZipFile(path).read("xl/drawings/drawing1.xml").decode()
check("saved as Office Math with a plain-text fallback", d.count("Requires=\"a14\"") == 2 and d.count("<mc:Fallback>") == 2)
again = Book.open(path)
check("and read back", again.equation_linear(0, 0) == "x=(−b±√(b^2−4ac))/2a")

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
