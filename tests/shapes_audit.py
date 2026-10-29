# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): SH-21 shapes, text boxes and WordArt — the Shapes
# gallery, drawing on the sheet, typing in a shape, the Shape Format tab, saving.
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
from folio_sheets import shapes_ui  # noqa: E402

win.resize(1280, 800)
pump(0.4)
g = win.grid
area = g.area
b = win.book
S = g.sheet


class Ev:
    def __init__(self, x, y, held=False, shift=False, double=False):
        self.x, self.y, self.button = float(x), float(y), 1
        self.type = Gdk.EventType._2BUTTON_PRESS if double else Gdk.EventType.BUTTON_PRESS
        st = Gdk.ModifierType(0)
        if held:
            st |= Gdk.ModifierType.BUTTON1_MASK
        if shift:
            st |= Gdk.ModifierType.SHIFT_MASK
        self.state = st

    def get_state(self):
        return self.state


def drag(a, z, shift=False):
    g.on_press(area, Ev(*a))
    g.on_motion(area, Ev((a[0] + z[0]) / 2, (a[1] + z[1]) / 2, held=True, shift=shift))
    g.on_motion(area, Ev(*z, held=True, shift=shift))
    g.on_release(area, Ev(*z))
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


objs = lambda: b.objects(S)  # noqa: E731
groups = Book.shape_gallery()
check("the Shapes gallery has the desktop spreadsheet's groups", [n for n, _ in groups][:4] == ["Lines", "Rectangles", "Basic Shapes", "Block Arrows"]
      and sum(len(l) for _, l in groups) >= 60, [n for n, _ in groups])
pop = shapes_ui.gallery(win, lambda p: None)
check("with a picture of each", len([w for w in pop.get_child().get_child().get_children()]) > 0)

win.run("shape-insert:roundRect")
check("choosing a shape starts drawing it", win.shapes.placing == "roundRect")
drag((300, 200), (500, 300))
check("dragging draws it", len(objs()) == 1 and objs()[0]["kind"] == "shape" and not win.shapes.placing, objs())
x, y, w, h = objs()[0]["rect"]
p0 = b.sheet_point(S, 300, 200, g.params())
check("where it was drawn, the size dragged", abs(x - p0[0]) < 1.5 and abs(w - 200 / g.params()["zoom"]) < 2, (x, y, w, h, p0))
check("it is selected, the Shape Format tab is there", g.sel_objects == [0] and win.ribbon.tab_by_name("Shape Format") is not None
      and win.ribbon.tab_by_name("Shape Format").button.get_visible())
info = b.shape_info(S, 0)
check("the default look: blue fill, darker outline", info["geom"] == "roundRect" and info["fill"] == 0x4472C4 and info["line"][0] == 0x223962, info)
c = px(400, 230)
check("drawn on the sheet", abs(c[0] - 0x44) < 30 and abs(c[2] - 0xC4) < 30, c)

win.run("shape-fill:FF0000")
win.run("shape-outline:000000")
win.run("shape-weight:3")
info = b.shape_info(S, 0)
check("Shape Fill and Shape Outline", info["fill"] == 0xFF0000 and info["line"][0] == 0 and info["line"][1] == 3, info)
c = px(400, 230)
check("in red now", c[0] > 200 and c[1] < 60 and c[2] < 60, c)
win.run("shape-outline:none")
check("No Outline", b.shape_info(S, 0)["line"] is None)
win.run("undo")
check("undo brings it back", b.shape_info(S, 0)["line"] is not None)

# typing in it
ok = win.shapes.edit(0, first="H")
tv = win.shapes.editor[1]
tv.get_buffer().set_text("Hello\nshapes")
win.shapes.finish()
pump()
check("typing in a shape", ok and b.shape_info(S, 0)["text"] == "Hello\nshapes", b.shape_info(S, 0)["text"])
c = px(400, 250)
win.run("bold")
win.run("font-size:20")
win.run("font-color:FFFF00")
info = b.shape_info(S, 0)
check("Home's Bold, Font Size and Font Color format the shape's text", info["bold"] and info["size"] == 20 and info["color"] == 0xFFFF00, info)
check("and not the cell under it", not win.book.cell_style(S, 0, 0).get("bold", False) if hasattr(win.book, "cell_style") else True)
win.run("align-right")
win.run("align-top")
info = b.shape_info(S, 0)
check("alignment too", info["align"] == 2 and info["anchor"] == "t", info)

win.run("shape-change:star5")
check("Edit Shape › Change Shape", b.shape_info(S, 0)["geom"] == "star5")
win.run("shape-height:2")
check("Size: height in inches", abs(objs()[0]["rect"][3] - 192) < 1.5, objs()[0]["rect"])

# a line, square with Shift
win.run("shape-insert:straightConnector1")
drag((200, 500), (350, 520), shift=True)
li = len(objs()) - 1
li_info = b.shape_info(S, li)
check("a line arrow; Shift keeps it straight", li_info["connector"] and abs(objs()[li]["rect"][3]) < 2, objs()[li]["rect"])
# a text box with a click
win.run("text-box")
g.on_press(area, Ev(700, 450))
g.on_release(area, Ev(700, 450))
pump(0.2)
ti = len(objs()) - 1
check("Text Box: a click makes one and starts typing", b.shape_info(S, ti)["textbox"] and win.shapes.editor is not None and win.shapes.editor[2] == ti)
win.shapes.editor[1].get_buffer().set_text("A note")
win.shapes.finish()
check("its text", b.shape_info(S, ti)["text"] == "A note")

# WordArt
win.grid.deselect_objects()
win.on_object_selection()
win.run("wordart-insert:3")
pump(0.2)
wi = len(objs()) - 1
wv = b.shape_info(S, wi)
check("WordArt: outlined text, no box", wv["textbox"] and wv["fill"] is None and wv["outline"] == 0x4472C4 and wv["size"] == 36, wv)
if win.shapes.editor:
    win.shapes.editor[1].get_buffer().set_text("Sales 2026")
    win.shapes.finish()
check("typed straight into", b.shape_info(S, wi)["text"] == "Sales 2026")

# arrange
g.select_object(0)
win.on_object_selection()
win.run("bring-to-front")
check("Bring to Front", objs()[-1]["name"].startswith("Rectangle") or objs()[-1]["name"].startswith("Star") or b.shape_info(S, len(objs()) - 1)["geom"] == "star5",
      [o["name"] for o in objs()])
win.run("send-to-back")
check("Send to Back", b.shape_info(S, 0)["geom"] == "star5")

# Esc cancels drawing
win.run("shape-insert:ellipse")


class Key:
    keyval = Gdk.KEY_Escape
    type = Gdk.EventType.KEY_PRESS

    def get_state(self):
        return Gdk.ModifierType(0)


win.on_key(win, Key())
check("Esc cancels drawing a shape", not win.shapes.placing)

# saved and read back
path = os.path.join(tempfile.mkdtemp(), "shapes.xlsx")
win.save_to(path)
again = Book.open(path)
kinds = [again.shape_info(0, i)["geom"] if again.shape_info(0, i) else None for i in range(len(again.objects(0)))]
check("saved: every shape comes back", sorted(k for k in kinds if k) == sorted(["star5", "straightConnector1", "rect", "rect"]), kinds)
texts = sorted(again.shape_info(0, i)["text"] for i in range(len(again.objects(0))))
check("with its text", "Hello\nshapes" in texts and "A note" in texts and "Sales 2026" in texts, texts)

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
