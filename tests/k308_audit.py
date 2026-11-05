# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): K3-08 — a diagram copied in Folio Cork pastes as an
# editable diagram; double-click opens it in Cork and each save there updates the picture.
import os
import shutil
import sys
import tempfile
import time
import traceback
import zipfile

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


MSG = []
win.dialogs.message = lambda t, *a, **k: MSG.append(t)
g = win.grid
b = win.book
TMP = tempfile.mkdtemp()
DATA = os.path.join(os.path.dirname(os.path.abspath(os.environ["FOLIO_TEST_SCRIPT"])), "data")
from folio_sheets import clip_ui, corkembed, insert_extras  # noqa: E402

diag = open(os.path.join(DATA, "diagram.png"), "rb").read()
cfg = os.path.join(TMP, "cfg")
os.makedirs(cfg)
os.environ["FOLIO_CONFIG_DIR"] = cfg
open(os.path.join(cfg, "clipboard.png"), "wb").write(diag)
ld = GdkPixbuf.PixbufLoader.new_with_type("png")
ld.write(diag)
ld.close()
cb = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
cb.set_image(ld.get_pixbuf())
pump(0.3)
g.select_cell(2, 2)
check("Paste Picture", clip_ui.paste_picture(win))
pump(0.2)
i = g.sel_objects[0] if g.sel_objects else None
got = b.picture_bytes(g.sheet, i) if i is not None else None
check("a diagram copied in Cork pastes as an editable diagram", got and corkembed.is_diagram(bytes(got[0])))
rect0 = b.objects(g.sheet)[i]["rect"] if i is not None else None
other = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, True, 8, 928, 720)
other.fill(0x30A050FF)
cb.set_image(other)
pump(0.3)
g.select_cell(40, 2)
clip_ui.paste_picture(win)
pump(0.2)
j = g.sel_objects[0] if g.sel_objects else None
g2 = b.picture_bytes(g.sheet, j) if j is not None else None
check("another picture stays a plain picture", g2 and not corkembed.is_diagram(bytes(g2[0])))
fake = os.path.join(TMP, "fake-cork")
shutil.copy(os.path.join(DATA, "fake_cork.py"), fake)
os.chmod(fake, 0o755)
os.environ["FOLIO_CORK"] = fake
if i is not None:
    check("double-click opens it in Cork", insert_extras.activate(win, i))
    t0 = time.time()
    k = None
    while time.time() - t0 < 8:
        pump(0.3)
        for n, o in enumerate(b.objects(g.sheet)):
            p = b.picture_bytes(g.sheet, n) if o["kind"] == "picture" else None
            if p and corkembed.png_size(bytes(p[0])) == (600, 200):
                k = n
        if k is not None:
            break
    check("saving in Cork updates the diagram", k is not None and corkembed.is_diagram(bytes(b.picture_bytes(g.sheet, k)[0])))
    if k is not None:
        x, y, w, h = b.objects(g.sheet)[k]["rect"]
        check("it keeps its place and takes the new proportions", abs(x - rect0[0]) < 0.5 and abs(y - rect0[1]) < 0.5
              and abs(w / h - 3.0) < 0.05, (x, y, w, h, rect0))
    check("two pictures still on the sheet", len(b.objects(g.sheet)) == 2, len(b.objects(g.sheet)))
out = os.path.join(TMP, "diagram.xlsx")
win.save_to(out)
z = zipfile.ZipFile(out)
check("the workbook carries the diagram", any(corkembed.is_diagram(z.read(n)) for n in z.namelist() if n.startswith("xl/media/")))
del os.environ["FOLIO_CORK"]
old = os.environ.get("PATH", "")
os.environ["PATH"] = "/nonexistent"
MSG.clear()
if k is not None and not os.path.exists("/usr/bin/folio-cork"):
    insert_extras.activate(win, k)
    check("without Cork, double-click says so", any("isn't installed" in m for m in MSG), MSG)
os.environ["PATH"] = old
b.mark_saved()
print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
