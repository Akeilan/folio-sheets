# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): SH-26 the Draw tab — pen, pencil and highlighter
# ink over the sheet, the eraser, Lasso Select; kept in the file as Office ink.
import os
import sys
import tempfile
import time
import traceback
import zipfile

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

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.resize(1280, 800)
pump(0.4)
g = win.grid
area = g.area
ink = win.ink


class Ev:
    def __init__(self, x, y, button=1, held=False):
        self.x, self.y, self.button = float(x), float(y), button
        self.type = Gdk.EventType.BUTTON_PRESS
        self.state = Gdk.ModifierType.BUTTON1_MASK if held else Gdk.ModifierType(0)

    def get_state(self):
        return self.state


def drag(points):
    g.on_press(area, Ev(*points[0]))
    for p in points[1:]:
        g.on_motion(area, Ev(*p, held=True))
    g.on_release(area, Ev(*points[-1]))
    pump(0.05)


def px(x, y):
    pump(0.2)
    w = area.get_window()
    img = cairo.ImageSurface(cairo.FORMAT_ARGB32, w.get_width(), w.get_height())
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, w, 0, 0)
    cr.paint()
    img.flush()
    d = img.get_data()
    o = int(y) * img.get_stride() + int(x) * 4
    return d[o + 2], d[o + 1], d[o]


b = win.book
S = g.sheet
inks = lambda: [i for i, o in enumerate(b.objects(S)) if o["kind"] == "ink"]  # noqa: E731

check("drawing is off to begin with", not ink.active())
win.run("draw-pen")
check("Pen turns drawing on", ink.tool == "pen")
r0 = g.sel.active
drag([(300, 200), (340, 230), (380, 210), (420, 240)])
check("dragging draws a stroke, not a selection", len(inks()) == 1 and g.sel.active == r0, (inks(), g.sel.active))
st = b.ink_strokes(S, inks()[0])
p0 = b.sheet_point(S, 300, 200, g.params())
check("where the pointer went, in sheet coordinates", st and abs(st[0][0][0][0] - p0[0]) < 1 and abs(st[0][0][0][1] - p0[1]) < 1, (st and st[0][0][:2], p0))
check("a black pen 0.5 mm wide", st[0][1] == 0x000000 and abs(st[0][2] - 0.5 / 25.4 * 96) < 0.01 and st[0][3] == 0, st[0][1:])
c = px(340, 230)
check("the ink is on screen", max(c) < 90, c)
win.run("ink-color:pen:E71224")
win.run("ink-width:pen:2")
drag([(300, 320), (500, 320)])
st2 = b.ink_strokes(S, inks()[-1])
check("the pen's colour and thickness can be changed", st2[0][1] == 0xE71224 and abs(st2[0][2] - 2 / 25.4 * 96) < 0.01, st2[0][1:])
c = px(400, 320)
check("drawn in red", c[0] > 180 and c[1] < 90, c)
win.run("draw-highlighter")
drag([(300, 420), (520, 420)])
st3 = b.ink_strokes(S, inks()[-1])
check("the highlighter: yellow, wide, see-through", st3[0][3] == 2 and st3[0][1] == 0xFFFF00 and st3[0][2] > 20, st3[0][1:])
c = px(400, 420)
check("cells show through the highlighter", c[2] < 200 and c[0] > 200 and c[1] > 200, c)
win.run("draw-pencil")
drag([(600, 200), (650, 260)])
check("the pencil", b.ink_strokes(S, inks()[-1])[0][3] == 1)
check("four strokes", len(inks()) == 4)

# the eraser takes a whole stroke away
win.run("draw-eraser")
drag([(398, 315), (402, 325)])
check("the eraser removes the stroke it touches", len(inks()) == 3 and all(b.ink_strokes(S, i)[0][1] != 0xE71224 for i in inks()))
win.run("undo")
pump()
check("undo brings it back", len(inks()) == 4)

# lasso select, then delete
win.run("draw-lasso")
drag([(280, 400), (540, 400), (540, 440), (280, 440), (280, 400)])
sel = list(g.sel_objects)
hl = [i for i in inks() if b.ink_strokes(S, i)[0][3] == 2]
check("Lasso Select picks up the ink inside", sel == hl, (sel, hl))
win.run("chart-delete")
pump()
check("and Delete deletes it", len(inks()) == 3)

# Esc and Select stop drawing
win.run("draw-pen")
class Key:
    keyval = Gdk.KEY_Escape
    type = Gdk.EventType.KEY_PRESS

    def get_state(self):
        return Gdk.ModifierType(0)


win.on_key(win, Key())
check("Esc stops drawing", not ink.active())
win.run("draw-pen")
win.run("draw-select")
check("so does Select", not ink.active())
g.on_press(area, Ev(500, 500))
g.on_release(area, Ev(500, 500))
pump()
check("and clicks select cells again", g.sel.active != r0)

# kept in the file as Office ink
path = os.path.join(tempfile.mkdtemp(), "ink.xlsx")
win.save_to(path)
z = zipfile.ZipFile(path)
inkparts = [n for n in z.namelist() if n.startswith("xl/ink/")]
check("saved as InkML parts", len(inkparts) == 3, z.namelist())
check("in the drawing as content parts", b"xdr14:contentPart" in z.read("xl/drawings/drawing1.xml"))
again = Book.open(path)
kinds = sorted(o["kind"] for o in again.objects(0))
check("opened again: the ink is there", kinds == ["ink", "ink", "ink"], kinds)
red = [i for i, o in enumerate(again.objects(0)) if again.ink_strokes(0, i)[0][1] == 0xE71224]
check("with its colours", len(red) == 1)

# what is out
for cmd in ("draw-ruler", "ink-to-shape", "ink-to-math", "ink-replay"):
    k = len(INFO)
    win.run(cmd)
    check("%s says it isn't available" % cmd, len(INFO) == k + 1 and "isn't available" in INFO[-1][1], INFO[-1:])

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
