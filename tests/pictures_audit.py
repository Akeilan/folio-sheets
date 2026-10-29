# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): pictures on the sheet and the clipboard between
# programs — paste a picture another program copied, Insert > Pictures, copy and cut a picture,
# drawing it, saving it; and a copied range offered to other programs as an HTML table.
import os
import sys
import tempfile
import time
import traceback

from gi.repository import Gdk, GdkPixbuf, Gtk

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


MESSAGES = []
Gtk.Dialog.run = lambda d: (MESSAGES.append(d.get_property("text") if isinstance(d, Gtk.MessageDialog) else d.get_title()),
                            Gtk.ResponseType.OK)[1]
Gtk.MessageDialog.run = Gtk.Dialog.run

win.resize(1280, 800)
pump(0.5)
g = win.grid
b = win.book
S = 0
cb = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
tmp = tempfile.mkdtemp()


def solid(w, h, rgb):
    pb = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, False, 8, w, h)
    pb.fill((rgb << 8) | 0xFF)
    return pb


def screen_px(x, y):
    pump(0.15)
    a = g.area.get_allocation()
    shot = Gdk.pixbuf_get_from_window(g.area.get_window(), 0, 0, a.width, a.height)
    px = shot.get_pixels()
    o = int(y) * shot.get_rowstride() + int(x) * shot.get_n_channels()
    return px[o], px[o + 1], px[o + 2]


def objs():
    return b.objects(S)


# ---- a picture another program copied (Draw and Gable put image/png on the clipboard)
g.select_range((2, 1, 2, 1), (2, 1))
cb.set_image(solid(120, 80, 0xE03C28))
pump(0.1)
win.run("paste")
o = objs()
check("Paste puts the copied picture on the sheet", len(o) == 1 and o[0]["kind"] == "picture", o)
x, y, w, h = o[0]["rect"] if o else (0, 0, 0, 0)
check("at its own size", abs(w - 120) < 1 and abs(h - 80) < 1, (w, h))
check("with its corner at the active cell", abs(x - b.col_x(S, 1)) < 1 and abs(y - b.row_y(S, 2)) < 1, (x, y))
check("and it is selected", g.sel_objects == [0])
check("its name", o and o[0]["name"] == "Picture 1", o and o[0]["name"])
r = b.object_screen_rect(S, 0, g.params())
c = screen_px(r[0] + r[2] / 2, r[1] + r[3] / 2) if r else (0, 0, 0)
check("it is drawn", abs(c[0] - 0xE0) < 6 and abs(c[1] - 0x3C) < 6 and abs(c[2] - 0x28) < 6, c)
win.run("undo")
check("undo takes it away", len(objs()) == 0)
win.run("redo")
check("redo brings it back", len(objs()) == 1)

# ---- copy and cut a selected picture: other programs get the picture
g.select_object(0)
win.run("copy")
pump(0.1)
img = cb.wait_for_image()
check("Copy with a picture selected puts the picture on the clipboard",
      img is not None and img.get_width() == 120 and img.get_height() == 80, img and (img.get_width(), img.get_height()))
win.run("cut")
pump(0.1)
check("Cut takes it off the sheet", len(objs()) == 0)
g.select_range((10, 4, 10, 4), (10, 4))
win.run("paste")
o = objs()
check("and Paste puts it back where the active cell is", len(o) == 1 and abs(o[0]["rect"][0] - b.col_x(S, 4)) < 1, o)
win.run("undo")
win.run("undo")
check("undo restores it where it was", len(objs()) == 1 and abs(objs()[0]["rect"][0] - b.col_x(S, 1)) < 1)

# ---- Paste Picture, and its message with no picture to paste
cb.set_image(solid(30, 30, 0x2050D0))
pump(0.1)
g.select_range((0, 8, 0, 8), (0, 8))
win.run("paste-picture")
check("Paste > Picture", len(objs()) == 2 and objs()[1]["kind"] == "picture")
cb.set_text("just text", -1)
pump(0.1)
MESSAGES.clear()
win.run("paste-picture")
check("with no picture it says so", len(objs()) == 2 and any("no picture" in (m or "") for m in MESSAGES), MESSAGES)

# ---- Insert > Pictures from files (a format Sheets can't read is converted)
png = os.path.join(tmp, "a.png")
solid(64, 48, 0x30A040).savev(png, "png", [], [])
tif = os.path.join(tmp, "a.tiff")
solid(50, 25, 0x804020).savev(tif, "tiff", [], [])
from folio_sheets import clip_ui  # noqa: E402
g.select_range((20, 1, 20, 1), (20, 1))
check("insert a PNG file", clip_ui.insert_picture_file(win, png) and objs()[-1]["rect"][2] == 64)
check("insert a TIFF file (converted)", clip_ui.insert_picture_file(win, tif) and abs(objs()[-1]["rect"][2] - 50) < 1)
bad = os.path.join(tmp, "bad.png")
with open(bad, "w") as f:
    f.write("not a picture")
MESSAGES.clear()
check("a file that isn't a picture is refused", not clip_ui.insert_picture_file(win, bad) and MESSAGES, MESSAGES)
big = os.path.join(tmp, "big.png")
solid(4000, 3000, 0x808080).savev(big, "png", [], [])
clip_ui.insert_picture_file(win, big)
bw = objs()[-1]["rect"]
check("a large picture is scaled to fit the view, keeping its shape", bw[2] < 1300 and abs(bw[2] / bw[3] - 4 / 3) < 0.01, bw)
win.run("undo")

# ---- moving, deleting
n = len(objs())
b.set_object_rect(S, 0, 300.0, 200.0, 240.0, 160.0, "Move Picture")
check("a picture can be moved and sized", tuple(round(v) for v in objs()[0]["rect"]) == (300, 200, 240, 160), objs()[0]["rect"])
g.select_object(0)
win.run("chart-delete")
pump(0.05)
check("Delete removes the selected picture", len(objs()) == n - 1, len(objs()))
win.run("undo")

# ---- a range goes to other programs as an HTML table and as text
g.deselect_objects()
b.set_input(S, 30, 0, "Item")
b.set_input(S, 30, 1, "Price")
b.set_input(S, 31, 0, "Tea & cake")
b.set_input(S, 31, 1, "4.5")
g.select_range((30, 0, 31, 1), (30, 0))
win.run("copy")
pump(0.1)
sel = cb.wait_for_contents(Gdk.Atom.intern("text/html", False))
html = sel.get_data().decode("utf-8") if sel is not None else ""
check("other programs get an HTML table", "<table" in html and "<td>Tea &amp; cake</td>" in html and "<td>4.5</td>" in html, html[:200])
check("and tab-separated text", (cb.wait_for_text() or "").replace("\r\n", "\n") == "Item\tPrice\nTea & cake\t4.5\n",
      cb.wait_for_text())
g.select_range((40, 0, 40, 0), (40, 0))
win.run("paste")
check("pasting it inside Sheets still copies the cells themselves", b.input(S, 41, 0) == "Tea & cake" and b.input(S, 41, 1) == "4.5")

# ---- saved and opened again
path = os.path.join(os.environ.get("PICS_OUT", tmp), "pictures.xlsx")
b.save(path)
from folio_sheets.core import Book  # noqa: E402
b2 = Book.open(path)
o2 = b2.objects(S)
check("pictures survive saving", [x["kind"] for x in o2] == [x["kind"] for x in objs()] and len(o2) == 4, [x["kind"] for x in o2])
check("with their pictures and places", b2.picture_bytes(S, 1) == b.picture_bytes(S, 1) and
      all(abs(a - c) < 1 for a, c in zip(o2[0]["rect"], objs()[0]["rect"])))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
