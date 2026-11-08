# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): the Backlog 4 items.
# Formulas tab's function menus, Advanced Filter, Consolidate, the Review tools, Custom Views,
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
from folio_sheets.core import Book  # noqa: E402
from folio_sheets import data_ui, clip_ui  # noqa: E402

# ---------------------------------------------------------------- S3-14 states and provinces
from folio_sheets import chartex_ui  # noqa: E402

rows = [("State", "Sales"), ("Guangdong", "40"), ("Kerala", "12"), ("Gauteng", "7"), ("Bali", "3"),
        ("Kaliningrad", "9")]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        b.set_input(g.sheet, r, c, v)
pump()
g.select_range((0, 0, len(rows) - 1, 1))
pump()
win.run("chartex:regionmap")
pump(0.2)
mi = g.sel_objects[0] if g.sel_objects else None
info = b.chartex_info(g.sheet, mi) if mi is not None else None
check("S3-14: provinces of five different countries make a map", info and info["kind"] == "regionmap", info)
check("S3-14: all five found, none unknown",
      info and info["map"]["found"] == 5 and not info["map"]["unknown"],
      info and info["map"])

# ---------------------------------------------------------------- S3-15 nothing fetched online
import socket  # noqa: E402

_blocked = []
_real = socket.socket


class NoSocket(socket.socket):
    def connect(self, *a, **k):
        _blocked.append(a)
        raise OSError("the map chart tried to reach the network")


socket.socket = NoSocket
try:
    png = os.path.join(TMP, "map.png")
    b.render_png(g.sheet, png, {"width": 640.0, "height": 420.0})
    made = os.path.exists(png) and os.path.getsize(png) > 2000
finally:
    socket.socket = _real
check("S3-15: a map chart draws with the network refused", made, _blocked)
check("S3-15: and nothing tried to connect", not _blocked, _blocked)

# ---------------------------------------------------------------- S3-17 Insert > Object > Create New
from folio_sheets import insert_extras  # noqa: E402

kinds = insert_extras.new_object_kinds()
check("S3-17: Create New offers the kinds this computer can make", len(kinds) >= 1,
      [k[0] for k in kinds])
if kinds:
    label, template, stem, ext = kinds[0]
    name, body = insert_extras._new_object_body(template, stem, ext)
    oi = insert_extras._place_object(
        win, insert_extras._icon_png(name, name.encode() + b"\x00" + body, b"flEm"), "New " + name)
    f = insert_extras.object_file(win, oi)
    check("S3-17: the new object is embedded whole", f and f[0] == name and f[1] == body, f and f[0])
    check("S3-17: and it is a real document, not an empty file", f and len(f[1]) > 1000, f and len(f[1]))
    out = insert_extras.open_object(win, oi, run=False)
    check("S3-17: it opens back out as a file of its own kind", out and out.endswith(ext) and
          open(out, "rb").read() == body, out)
    # it survives a save and reopen
    p = os.path.join(TMP, "withobject.xlsx")
    win.save_to(p)
    again = Book.open(p).picture_bytes(g.sheet, oi)
    check("S3-17: and it is still there in the saved workbook",
          again and insert_extras.png_chunk(bytes(again[0]), b"flEm") is not None)

print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
sys.stdout.flush()
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
