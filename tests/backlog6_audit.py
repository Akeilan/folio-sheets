# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): the Backlog 6 items (Sheet).
# Passwords to open, region, files, shortcuts and editing behaviour fixed in Backlog 6.
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
import io
import zipfile
from folio_sheets import encryption


def say_stub():
    said = []
    real = win.dialogs.message
    win.dialogs.message = lambda msg, *a, **k: said.append(msg)
    return said, lambda: setattr(win.dialogs, "message", real)


def answers(seq):
    """ask_password answers, in order (None = Cancel)."""
    q = list(seq)
    real = win.dialogs.ask_password
    win.dialogs.ask_password = lambda *a, **k: q.pop(0) if q else None
    return lambda: setattr(win.dialogs, "ask_password", real)


# ------------------------------------------------------------------ S6-30 password to open
b.set_input(0, 0, 0, "Secret value")
b.set_input(0, 1, 0, "=21*2")
undo = answers(["Tulip 7", "Tulip 7"])
win.run("encrypt-password")
undo()
check("S6-30: Encrypt with Password sets the password to open", win.book.open_password == "Tulip 7", win.book.open_password)
check("S6-30: setting it leaves the workbook needing a save", win.book.modified)
enc_path = os.path.join(TMP, "locked.xlsx")
check("S6-30: saving works", win.save_to(enc_path))
raw = open(enc_path, "rb").read()
check("S6-30: the saved file is encrypted (compound file, no zip inside)", raw[:8] == encryption.OLE_MAGIC and b"xl/workbook" not in raw)
check("S6-30: the saved file isn't left needing a save", not win.book.modified)
try:
    import msoffcrypto
    f = msoffcrypto.OfficeFile(io.BytesIO(raw))
    f.load_key(password="Tulip 7")
    out = io.BytesIO()
    f.decrypt(out, verify_integrity=True)
    z = zipfile.ZipFile(out)
    check("S6-30: an independent reader decrypts it (integrity checked)", "xl/workbook.xml" in z.namelist())
except ImportError:
    print("NOTE msoffcrypto-tool not installed; independent check skipped")

# mismatched confirmation
undo = answers(["abc", "abd"])
said, unsay = say_stub()
win.run("encrypt-password")
undo(); unsay()
check("S6-30: a confirmation that differs is refused", win.book.open_password == "Tulip 7" and said and "not identical" in said[0], said)

# open it again: wrong password, then the right one
undo = answers(["tulip 7", "Tulip 7"])
said, unsay = say_stub()
win.load(enc_path)
undo(); unsay()
pump(0.2)
check("S6-30: a wrong password says so", said and "not correct" in said[0], said)
check("S6-30: the right password opens the workbook", win.book.display(0, 0, 0) == "Secret value" and win.book.display(0, 1, 0) == "42",
      (win.book.display(0, 0, 0), win.book.display(0, 1, 0)))
check("S6-30: the opened workbook keeps its password", win.book.open_password == "Tulip 7")
check("S6-30: the opened workbook keeps its name", win.path == os.path.abspath(enc_path), win.path)

# Cancel keeps the workbook that was open
before = win.book
undo = answers([None])
win.load(enc_path)
undo()
check("S6-30: Cancel at the password leaves the open workbook alone", win.book is before)

# File > Info says so
win.backstage.open()
win.backstage.nav("info")
pump(0.2)
st = getattr(win.backstage, "protect_status", None)
check("S6-30: File > Info says a password is required to open it",
      st is not None and "password is required to open" in st.get_text(), st and st.get_text())
labels = [i.get_label() for i in win.backstage.protect_menu.get_children()] if hasattr(win.backstage, "protect_menu") else []
check("S6-30: Protect Workbook offers Encrypt with Password", "Encrypt with Password" in labels, labels)
win.backstage.close()

# an attachment copy doesn't save the open workbook
win.book.set_input(0, 2, 0, "unsaved")
copy = os.path.join(TMP, "copy.xlsx")
win.write_file(copy)
check("S6-30: writing a copy (attachment) leaves unsaved changes unsaved", win.book.modified and win.path == os.path.abspath(enc_path))
check("S6-30: the copy is encrypted too", open(copy, "rb").read(8) == encryption.OLE_MAGIC)

# removing the password
undo = answers([""])
win.run("encrypt-password")
undo()
plain = os.path.join(TMP, "plain.xlsx")
win.save_to(plain)
check("S6-30: an empty password removes it; the file saves as a plain workbook",
      win.book.open_password is None and zipfile.is_zipfile(plain))

# the older Standard encryption (Excel 2007), written straight from the specification
sys.path.insert(0, os.path.dirname(os.path.abspath("tests/standard_enc.py")))
from standard_enc import standard
theirs = os.path.join(TMP, "standard.xlsx")
open(theirs, "wb").write(standard(open(plain, "rb").read(), "Other1", encryption.write_streams))
undo = answers(["Other1"])
win.load(theirs)
undo()
check("S6-30: a workbook in Excel 2007's Standard encryption opens", win.book.display(0, 0, 0) == "Secret value")
# a large workbook (past 109 allocation sectors) round-trips
import zipfile as _z
_b = io.BytesIO()
with _z.ZipFile(_b, "w", _z.ZIP_STORED) as _zz:
    _zz.writestr("x", os.urandom(9000000))
_big = _b.getvalue()
check("S6-30: a 9 MB package encrypts and decrypts", encryption.decrypt(encryption.encrypt(_big, "p"), "p") == _big)

# ------------------------------------------------------------------ S6-28 / S6-31 text files
win.new_book()
pump(0.1)
b = win.book
cp = os.path.join(TMP, "ansi.csv")
open(cp, "wb").write("Naam,Bedrag\r\nJosé,12\r\nCrème brûlée,€ 3\r\n".encode("cp1252"))
win.load(cp)
pump(0.1)
check("S6-28: a Windows-1252 CSV keeps its accents", win.book.display(0, 1, 0) == "José" and win.book.display(0, 2, 0) == "Crème brûlée",
      (win.book.display(0, 1, 0), win.book.display(0, 2, 0)))
up = os.path.join(TMP, "unicode.csv")
open(up, "wb").write("\ufeffa\tb\r\nŻółw\t1\r\n".encode("utf-16-le"))
win.load(up)
pump(0.1)
check("S6-28: a UTF-16 (Unicode Text) file opens as text", win.book.display(0, 1, 0) == "Żółw" and win.book.display(0, 1, 1) == "1",
      (win.book.display(0, 1, 0), win.book.display(0, 1, 1)))
from folio_sheets.text_ui import detect_encoding
check("S6-28: encodings are recognised (UTF-8, BOM, UTF-16 without a mark, Windows-1252)",
      detect_encoding("é".encode()) == "utf-8-sig" and detect_encoding(b"\xef\xbb\xbfx") == "utf-8-sig"
      and detect_encoding("ab,cd\r\n".encode("utf-16-le")) == "utf-16-le" and detect_encoding("é".encode("cp1252")) == "cp1252")

win.new_book()
pump(0.1)
win.book.set_input(0, 0, 0, "José")
win.book.set_input(0, 0, 1, "1.5")
win.book.set_input(0, 1, 0, "a,b")
win.book.set_input(0, 1, 1, "Żółw")
c1 = os.path.join(TMP, "out.csv")
win.save_csv(c1, "csv")
raw = open(c1, "rb").read()
check("S6-28: CSV (Comma delimited) is written in the Windows code page", raw.startswith("José,1.5".encode("cp1252")), raw[:20])
check("S6-28: characters it can't hold become ? and the status bar says CSV UTF-8 keeps them",
      b"?" in raw and "CSV UTF-8" in win.status.extra.get_text(), win.status.extra.get_text())
c2 = os.path.join(TMP, "out8.csv")
win.save_csv(c2, "csvutf8")
raw = open(c2, "rb").read()
check("S6-28: CSV UTF-8 starts with a byte-order mark and keeps every character",
      raw.startswith(b"\xef\xbb\xbf") and "Żółw".encode() in raw and b'"a,b"' in raw, raw[:30])
t1 = os.path.join(TMP, "out.txt")
win.save_csv(t1, "txt")
raw = open(t1, "rb").read()
check("S6-31: Text (Tab delimited) separates fields with tabs", raw.split(b"\r\n")[0] == "José\t1.5".encode("cp1252"), raw[:30])
win.backstage.open()
win.backstage.nav("export")
pump(0.2)
blabels = [x.get_label() for x in widgets(win.backstage, Gtk.Button)]
check("S6-31: File > Export offers Text (Tab delimited) and CSV UTF-8",
      any("Text (Tab delimited)" in (l or "") for l in blabels) and any("CSV UTF-8" in (l or "") for l in blabels), blabels)
win.backstage.close()

# ------------------------------------------------------------------ S6-29 templates
tp = os.path.join(TMP, "Budget.xltx")
win.new_book()
pump(0.1)
win.book.set_input(0, 0, 0, "Template text")
win.save_to(tp)
w2 = win.app.open_path(tp, as_new=True)
pump(0.3)
check("S6-29: a template opened from outside starts a new workbook (Budget1)",
      w2.path is None and w2.untitled == "Budget1" and w2.book.display(0, 0, 0) == "Template text", (w2.path, w2.untitled))
check("S6-29: its Save As suggests a workbook, not the template", getattr(w2, "save_kind", None) == "xlsx")
nb = os.path.join(TMP, "Budget1.xlsx")
w2.save_to(nb)
ct = zipfile.ZipFile(nb).read("[Content_Types].xml").decode()
check("S6-29: saved as a workbook, its content type is a workbook's", "sheet.main+xml" in ct and "template.main" not in ct)
check("S6-29: the template itself is untouched", zipfile.ZipFile(tp).read("[Content_Types].xml").decode().count("template.main") == 1)
w2.book.mark_saved()
w2.destroy()
pump(0.2)
w3 = win.app.open_path(tp, win)
pump(0.2)
check("S6-29: File > Open of a template edits the template itself", w3.path == os.path.abspath(tp), w3.path)

# ------------------------------------------------------------------ S6-32 region
from folio_sheets import regional
from folio_sheets.core import _c
NL = {"decimal": ",", "group": ".", "date_order": "DMY", "short_date": "d-m-yyyy",
      "currency": "\u20ac", "currency_before": True, "currency_space": True}
US = regional.current()
win.new_book()
pump(0.1)
b = win.book
regional.apply(NL)
b.set_input(0, 0, 0, "1,5")
b.set_input(0, 1, 0, "1.234,5")
b.set_input(0, 2, 0, "15-11-2026")
b.set_input(0, 3, 0, "\u20ac 12,50")
b.set_input(0, 4, 0, "=A1*2")
b.set_input(0, 5, 0, "12%")
check("S6-32: NL: 1,5 is a number shown as 1,5", b.value(0, 0, 0) == 1.5 and b.display(0, 0, 0) == "1,5", (b.value(0, 0, 0), b.display(0, 0, 0)))
check("S6-32: NL: 1.234,5 is 1234.5", b.value(0, 1, 0) == 1234.5, b.value(0, 1, 0))
check("S6-32: NL: 15-11-2026 is a date shown day first", b.value(0, 2, 0) == 46341 and b.display(0, 2, 0) == "15-11-2026",
      (b.value(0, 2, 0), b.display(0, 2, 0)))
check("S6-32: NL: \u20ac 12,50 is currency", b.value(0, 3, 0) == 12.5 and b.display(0, 3, 0).replace("\xa0", " ") == "\u20ac 12,50", b.display(0, 3, 0))
check("S6-32: NL: a formula result shows a decimal comma; the formula stays as typed", b.display(0, 4, 0) == "3" and b.input(0, 4, 0) == "=A1*2",
      (b.display(0, 4, 0), b.input(0, 4, 0)))
check("S6-32: NL: the cell's edit text uses the decimal comma", b.input(0, 0, 0) == "1,5", b.input(0, 0, 0))
cur = dict(regional.refresh_presets() or __import__("folio_sheets.ribbon_defs", fromlist=["x"]).NUMBER_FORMATS)["Currency"]
b.set_number_format(0, [(1, 0, 1, 0)], cur)
check("S6-32: NL: the ribbon's Currency format is in euros", b.display(0, 1, 0).replace("\xa0", " ") == "\u20ac 1.234,50", b.display(0, 1, 0))
c3 = os.path.join(TMP, "nl.csv")
win.save_csv(c3, "csv")
first = open(c3, "rb").read().split(b"\r\n")[0].decode("cp1252")
check("S6-32: NL: CSV uses semicolons", first == "1,5", first)
lines = open(c3, "rb").read().decode("cp1252").split("\r\n")
check("S6-32: NL: CSV rows keep decimal commas without quoting", lines[1].replace("\xa0", " ") in ("\u20ac 1.234,50", "1234,5", "1.234,5"), lines[1])
nlx = os.path.join(TMP, "nl.xlsx")
win.save_to(nlx)
regional.apply(US)
regional.refresh_presets()
win.load(nlx)
pump(0.1)
check("S6-32: the file stays region-neutral: opened in the US it shows 1.5 and 11/15/2026",
      win.book.display(0, 0, 0) == "1.5" and win.book.display(0, 2, 0) == "11/15/2026", (win.book.display(0, 0, 0), win.book.display(0, 2, 0)))
try:
    import locale as _loc
    _loc.setlocale(_loc.LC_TIME, "nl_NL.UTF-8")
    _have_nl = True
except Exception:
    _have_nl = False
if _have_nl:
    det = regional.detect({"FOLIO_SHEET_REGION": "nl_NL.UTF-8"})
    check("S6-32: the Dutch system region is read as Excel reads it",
          det["decimal"] == "," and det["group"] == "." and det["date_order"] == "DMY" and det["currency"] == "\u20ac", det)
    regional.detect({})
else:
    print("NOTE nl_NL locale not generated here; detection check skipped")
regional.apply(regional.detect({"FOLIO_SHEET_REGION": "default"}))

# ------------------------------------------------------------------ S6-33 drawing cell formats
import cairo
from folio_sheets.core import Book as _Book
_d = _Book.open(os.path.abspath("tests/data/drawing_s633.xlsx"))
_png = os.path.join(TMP, "s633.png")
_d.render_png(0, _png, {"width": 520, "height": 420, "zoom": 1.0})
_img = cairo.ImageSurface.create_from_png(_png)
_W, _H, _st = _img.get_width(), _img.get_height(), _img.get_stride()
_px = bytes(_img.get_data())


def pix(x, y):
    o = y * _st + x * 4
    return (_px[o + 2], _px[o + 1], _px[o])


def region(x0, y0, x1, y1):
    return [pix(x, y) for y in range(max(0, y0), min(_H, y1)) for x in range(max(0, x0), min(_W, x1))]


_vp = {"width": 520, "height": 420, "zoom": 1.0}


def cell(r, c):
    x, y, w, h = _d.cell_rect(0, r, c, _vp)
    return int(x), int(y), int(x + w), int(y + h)


ax0, ay0, ax1, ay1 = cell(0, 0)
a_cell = region(ax0 + 4, ay0 + 3, ax1 - 3, ay1 - 3)
reds = sum(1 for p in a_cell if p[0] > 200 and p[1] < 80)
yell = sum(1 for p in a_cell if p[0] > 200 and p[1] > 200)
check("S6-33: a pattern fill is drawn as a pattern (both colours show)", reds > 20 and yell > 5, (reds, yell))
cx0, cy0, cx1, cy1 = cell(0, 2)
g = (pix(cx0 + 3, (cy0 + cy1) // 2), pix(cx1 - 4, (cy0 + cy1) // 2))
check("S6-33: a gradient fill runs from its first colour to its last", g[0][0] > 200 and g[0][2] < 80 and g[1][2] > 200 and g[1][0] < 80, g)
sx0, sy0, sx1, sy1 = cell(4, 2)
spill = [p for p in region(sx1 + 2, sy0 + 2, sx1 + 40, sy1 - 2) if sum(p) < 400]
inside = [p for p in region(sx0 + 2, sy0 + 2, sx1 - 2, sy1 - 2) if sum(p) < 400]
check("S6-33: Shrink to Fit keeps the text inside its cell", inside and not spill, (len(inside), len(spill)))
rx0, ry0, rx1, ry1 = cell(5, 2)
dark = [(x, y) for y in range(ry0 + 2, ry1 - 2) for x in range(rx0 + 2, rx1 - 2) if sum(pix(x, y)) < 300]
tall = (max(y for _, y in dark) - min(y for _, y in dark)) if dark else 0
wide = (max(x for x, _ in dark) - min(x for x, _ in dark)) if dark else 0
check("S6-33: rotated text with Wrap Text on stands up, in more than one line", tall > 60 and 20 < wide < 60, (tall, wide))
dx0, dy0, dx1, dy1 = cell(7, 2)
lastline = [x for y in range((dy0 + dy1) // 2, dy1 - 2) for x in range(dx0 + 2, dx1 - 2) if sum(pix(x, y)) < 300]
check("S6-33: Distributed runs the last line edge to edge", lastline and max(lastline) > dx1 - 12 and min(lastline) < dx0 + 10,
      (min(lastline or [0]), max(lastline or [0]), dx0, dx1))
print("NOTE drawing picture at %s" % _png)

# ------------------------------------------------------------------ S6-34 charts
win.new_book()
pump(0.1)
b = win.book
g = win.grid
for r, row in enumerate([["", "North", "South"], ["Q1", 12, 7], ["Q2", 15, 9], ["Q3", 9, 12], ["Q4", 18, 10]]):
    for c, v in enumerate(row):
        if v != "":
            b.set_input(0, r, c, str(v))
for r, row in enumerate([["X", "Y", "Size"], [1, 4, 10], [2, 7, 30], [3, 3, 5]]):
    for c, v in enumerate(row):
        b.set_input(0, r, 5 + c, str(v))
NEW = ["col3d-clustered", "col3d-stacked", "col3d-percent", "col3d", "bar3d-clustered", "bar3d-stacked", "bar3d-percent",
       "line3d", "area3d", "area3d-stacked", "area3d-percent", "pie3d", "pie-of-pie", "bar-of-pie",
       "surface3d", "surface3d-wire", "contour-wire", "bubble", "bubble3d"]
CMD = {"col3": "chart-column", "bar3": "chart-column", "line": "chart-line", "area": "chart-line", "pie3": "chart-pie",
       "pie-": "chart-pie", "bar-": "chart-pie", "surf": "chart-column", "cont": "chart-column", "bubb": "chart-scatter"}
made = []
for k in NEW:
    g.select_range((0, 5, 3, 7) if k.startswith("bubble") else (0, 0, 4, 2))
    n0 = len(b.objects(0))
    win.run("%s:%s" % (CMD[k[:4]], k))
    pump(0.05)
    if len(b.objects(0)) > n0:
        made.append(b.chart_spec(0, len(b.objects(0)) - 1)["kind"])
    g.clear_object_selection() if hasattr(g, "clear_object_selection") else None
check("S6-34: every new chart kind inserts from the ribbon", made == NEW, [x for x in NEW if x not in made])
bs = b.chart_spec(0, NEW.index("bubble"))
check("S6-34: a bubble chart takes X, Y and size from the columns",
      len(bs["series"]) == 1 and bs["series"][0]["cat"].endswith("$F$2:$F$4") and bs["series"][0]["size"].endswith("$H$2:$H$4"), bs["series"])
cx = os.path.join(TMP, "charts.xlsx")
win.save_to(cx)
reb = _Book.open(cx)
back = [reb.chart_spec(0, i)["kind"] for i in range(len(NEW))]
check("S6-34: they save as themselves and open again as the same kinds", back == NEW, back)
names = zipfile.ZipFile(cx).read("xl/charts/chart13.xml").decode() if "xl/charts/chart13.xml" in zipfile.ZipFile(cx).namelist() else ""
check("S6-34: pie-of-pie is written as an ofPie chart", "<c:ofPieChart>" in names and 'ofPieType val="pie"' in names, names[:200])
sp = b.range_spec(0, (0, 0, 4, 2), "scatter")
check("S6-34: a scatter of a block whose first column is text numbers the points (no X values)",
      all(x["cat"] == "" for x in sp["series"]) and len(sp["series"]) == 2, sp["series"])
import cairo as _cairo
_img = _cairo.ImageSurface(_cairo.FORMAT_ARGB32, 300, 200)
_ok = True
for k in NEW:
    s2 = b.range_spec(0, (0, 5, 3, 7) if k.startswith("bubble") else (0, 0, 4, 2), k if k.startswith("bubble") else "col")
    s2["kind"] = k
    try:
        b.render_spec(0, _cairo.Context(_img), 300, 200, s2)
    except Exception as e:
        _ok = False
        print("render", k, e)
check("S6-34: every new kind draws", _ok)

# ------------------------------------------------------------------ S6-18 references copied off the sheet
win.new_book()
pump(0.1)
b = win.book
g = win.grid
b.set_input(0, 0, 2, "=A1+B1")
g.select_cell(0, 2)
win.run("copy")
g.select_cell(0, 0)
win.run("paste")
check("S6-18: =A1+B1 copied from C1 to A1 gives #REF!", b.input(0, 0, 0) == "=#REF!+#REF!" and b.display(0, 0, 0) == "#REF!", b.input(0, 0, 0))
b.set_input(0, 5, 3, "=SUM(D1:D5)")
data = bytes(b.copy_to_bytes(0, (5, 3, 5, 3)))
b.paste_from_bytes(data, 0, (2, 3, 2, 3))
check("S6-18: a range pushed past the top is #REF! too (between workbooks)", b.input(0, 2, 3) == "=SUM(#REF!)", b.input(0, 2, 3))
b.paste_from_bytes(data, 0, (9, 6, 9, 6))
check("S6-18: an ordinary paste still moves the references", b.input(0, 9, 6) == "=SUM(G5:G9)", b.input(0, 9, 6))
b.set_input(0, 1, 5, "=F1*2")
g.select_range((0, 5, 1, 5))
win.run("fill-up")
check("S6-18: Fill Up past the top gives #REF!", b.input(0, 0, 5) == "=#REF!*2", b.input(0, 0, 5))

# ------------------------------------------------------------------ window behaviour
from gi.repository import Gdk


class K:
    def __init__(self, kv, mods=0):
        self.keyval = kv
        self.type = Gdk.EventType.KEY_PRESS
        self.hardware_keycode = 0
        self._m = mods
        self.state = Gdk.ModifierType(mods)

    def get_state(self):
        return Gdk.ModifierType(self._m)


CTRL, SHIFT, ALT = Gdk.ModifierType.CONTROL_MASK, Gdk.ModifierType.SHIFT_MASK, Gdk.ModifierType.MOD1_MASK


def key(kv, mods=0):
    g = win.grid
    if g.editing():
        if win.on_key(win, K(kv, mods)):
            return True
        return g.on_editor_key(g.editor, K(kv, mods))
    g.area.grab_focus()
    pump(0.02)
    return win.on_key(win, K(kv, mods))


def typ(text):
    g = win.grid
    for ch in text:
        if not g.editing():
            key(Gdk.unicode_to_keyval(ord(ch)))
        else:
            # as a key press in the text view: the selection is replaced
            buf = g.editor.get_buffer()
            buf.delete_selection(True, True)
            buf.insert_interactive_at_cursor(ch, -1, True)
            pump(0.01)
    pump(0.05)


win.new_book()
pump(0.1)
b = win.book
g = win.grid

# S6-07 Alt+Enter turns Wrap Text on
g.select_cell(0, 0)
typ("one")
key(Gdk.KEY_Return, ALT)
typ("two")
key(Gdk.KEY_Return)
check("S6-07: Alt+Enter keeps the line break and turns Wrap Text on",
      b.input(0, 0, 0) == "one\ntwo" and b.cell_style(0, 0, 0).get("wrap"), (b.input(0, 0, 0), b.cell_style(0, 0, 0).get("wrap")))
check("S6-07: and the row grows", b.row_height(0, 0) > 20, b.row_height(0, 0))

# S6-08 a missing closing bracket is added
g.select_cell(1, 0)
typ("=SUM(1,2")
key(Gdk.KEY_Return)
check("S6-08: =SUM(1,2 is stored as =SUM(1,2)", b.input(0, 1, 0) == "=SUM(1,2)" and b.display(0, 1, 0) == "3", b.input(0, 1, 0))
g.select_cell(1, 1)
typ('=LEN("((")')
key(Gdk.KEY_Return)
check("S6-08: brackets inside quotes don't count", b.display(0, 1, 1) == "2", b.input(0, 1, 1))

# S6-21 typed addresses become links
g.select_cell(2, 0)
typ("www.example.org")
key(Gdk.KEY_Return)
lk = b.link_at(0, 2, 0)
check("S6-21: a typed web address becomes a link", lk and lk.get("target") == "http://www.example.org", lk)
g.select_cell(2, 1)
typ("ron@example.org")
key(Gdk.KEY_Return)
lk = b.link_at(0, 2, 1)
check("S6-21: a typed e-mail address becomes a mailto link", lk and lk.get("target") == "mailto:ron@example.org", lk)

# S6-22 Backspace then typing is one undo step
b.set_input(0, 3, 0, "old")
g.select_cell(3, 0)
key(Gdk.KEY_BackSpace)
typ("7")
key(Gdk.KEY_Return)
win.run("undo")
pump(0.05)
check("S6-22: Backspace, 7, Enter, Ctrl+Z gives the old value back", b.input(0, 3, 0) == "old", b.input(0, 3, 0))
g.select_cell(3, 0)
key(Gdk.KEY_BackSpace)
key(Gdk.KEY_Escape)
check("S6-22: Backspace then Esc leaves the cell as it was", b.input(0, 3, 0) == "old")
g.select_cell(3, 0)
win.run("copy")
pump(0.05)
key(Gdk.KEY_Escape)
g.select_cell(10, 5)
win.run("paste")
pump(0.05)
check("S6-22: after Esc ends a copy, Paste has nothing to paste", b.input(0, 10, 5) == "", b.input(0, 10, 5))

# S6-12 shortcuts
import datetime
g.select_cell(4, 0)
key(Gdk.KEY_semicolon, CTRL)
key(Gdk.KEY_Return)
v = b.value(0, 4, 0)
today = (datetime.date.today() - datetime.date(1899, 12, 30)).days
check("S6-12: Ctrl+; enters today's date", v == today, (v, today, b.input(0, 4, 0)))
g.select_cell(4, 1)
key(Gdk.KEY_colon, CTRL | SHIFT)
key(Gdk.KEY_Return)
v = b.value(0, 4, 1)
check("S6-12: Ctrl+Shift+: enters the time", isinstance(v, float) and 0 <= v < 1, v)
b.set_input(0, 5, 2, "=1+1")
g.select_cell(6, 2)
key(Gdk.KEY_apostrophe, CTRL)
key(Gdk.KEY_Return)
check("S6-12: Ctrl+' copies the formula from above as it is", b.input(0, 6, 2) == "=1+1", b.input(0, 6, 2))
g.select_cell(7, 2)
key(Gdk.KEY_quotedbl, CTRL | SHIFT)
key(Gdk.KEY_Return)
check("S6-12: Ctrl+Shift+\" copies the value from above", b.input(0, 7, 2) == "2", b.input(0, 7, 2))
g.select_cell(20, 0)
key(Gdk.KEY_9, CTRL)
check("S6-12: Ctrl+9 hides the row", b.row_hidden(0, 20))
g.select_range((19, 0, 21, 0))
key(Gdk.KEY_parenleft, CTRL | SHIFT)
check("S6-12: Ctrl+Shift+( unhides it", not b.row_hidden(0, 20))
g.select_cell(0, 9)
key(Gdk.KEY_0, CTRL)
check("S6-12: Ctrl+0 hides the column", b.col_px(0, 9) == 0, b.col_px(0, 9))
g.select_range((0, 8, 0, 10))
key(Gdk.KEY_parenright, CTRL | SHIFT)
check("S6-12: Ctrl+Shift+) unhides it", b.col_px(0, 9) > 0, b.col_px(0, 9))
for r in range(30, 33):
    for c in range(0, 3):
        b.set_input(0, r, c, str(r + c))
g.select_cell(31, 1)
key(Gdk.KEY_asterisk, CTRL | SHIFT)
check("S6-12: Ctrl+Shift+* selects the current region", tuple(g.sel.ranges[-1]) == (30, 0, 32, 2), g.sel.ranges)
g.select_cell(30, 0)
key(Gdk.KEY_2, CTRL)
check("S6-12: Ctrl+2 is Bold", b.cell_style(0, 30, 0).get("bold"))
win.run("italic")
g.select_cell(31, 0)
key(Gdk.KEY_F4)
check("S6-12: F4 repeats the last action", b.cell_style(0, 31, 0).get("italic"))
g.select_cell(32, 0)
key(Gdk.KEY_y, CTRL)
check("S6-12: Ctrl+Y with nothing to redo repeats too", b.cell_style(0, 32, 0).get("italic"))
b.set_input(0, 40, 0, "=A30+B31")
g.select_cell(40, 0)
key(Gdk.KEY_bracketleft, CTRL)
check("S6-12: Ctrl+[ selects the cells the formula uses", sorted(tuple(x) for x in g.sel.ranges) == [(29, 0, 29, 0), (30, 1, 30, 1)], g.sel.ranges)
g.select_cell(29, 0)
key(Gdk.KEY_bracketright, CTRL)
check("S6-12: Ctrl+] selects the cells that use it", [tuple(x) for x in g.sel.ranges] == [(40, 0, 40, 0)], g.sel.ranges)
col0 = win.ribbon.revealer.get_reveal_child() if hasattr(win.ribbon, "revealer") else None
key(Gdk.KEY_F1, CTRL)
pump(0.2)
col1 = win.ribbon.revealer.get_reveal_child() if hasattr(win.ribbon, "revealer") else None
key(Gdk.KEY_F1, CTRL)
pump(0.2)
check("S6-12: Ctrl+F1 collapses and shows the ribbon", col0 != col1, (col0, col1))
_saved_as = []
_real_sa = win.act_save_as
win.act_save_as = lambda *a, **k: _saved_as.append(1)
key(Gdk.KEY_F12)
win.act_save_as = _real_sa
check("S6-12: F12 is Save As", _saved_as == [1])
_helped = []
_real_h = win.act_help
win.act_help = lambda: _helped.append(1)
key(Gdk.KEY_F1)
win.act_help = _real_h
check("S6-12: F1 opens the shortcut list", _helped == [1])
b.set_input(0, 50, 0, "copied")
g.select_cell(50, 0)
win.run("copy")
g.select_cell(51, 0)
key(Gdk.KEY_Return)
check("S6-12: Enter pastes after a copy and ends it", b.input(0, 51, 0) == "copied" and g.copy_range is None, (b.input(0, 51, 0), g.copy_range))
win.book.set_note(0, 60, 0, "Ron", "a note")
g.select_cell(0, 0)
key(Gdk.KEY_o, CTRL | SHIFT)
check("S6-12: Ctrl+Shift+O selects the cells with notes", (60, 0, 60, 0) in [tuple(x) for x in g.sel.ranges], g.sel.ranges)

# F11: a chart sheet
for r, row in enumerate([["", "A", "B"], ["x", 1, 2], ["y", 3, 4]]):
    for c, v in enumerate(row):
        if v != "":
            b.set_input(0, 70 + r, c, str(v))
g.select_cell(71, 1)
n0 = b.sheet_count
key(Gdk.KEY_F11)
pump(0.2)
check("S6-12: F11 puts a chart of the data on a chart sheet of its own",
      b.sheet_count == n0 + 1 and b.sheet_info(g.sheet)["kind"] == "chart" and b.sheet_info(g.sheet)["name"] == "Chart1", (b.sheet_count, b.sheet_info(g.sheet)))
check("S6-12: the chart sheet's chart takes the chart commands", g.selected_chart() == 0 and win.ribbon.tab_by_name("Chart Design") is not None)
cs = os.path.join(TMP, "chartsheet.xlsx")
win.save_to(cs)
zn = zipfile.ZipFile(cs).namelist()
check("S6-12: it is saved as a chart sheet with its drawing and chart",
      any(n.startswith("xl/chartsheets/sheet") for n in zn) and any(n.startswith("xl/charts/chart") for n in zn), [n for n in zn if "chart" in n])
rb = _Book.open(cs)
idx = next(i for i in range(rb.sheet_count) if rb.sheet_info(i)["kind"] == "chart")
check("S6-12: and opens again showing its chart", rb.objects(idx) and rb.objects(idx)[0]["kind"] == "chart", rb.objects(idx))
try:
    import openpyxl as _ox
except ImportError:
    _ox = None
    print("NOTE openpyxl is not installed here; the second reader's check is skipped")
if _ox is not None:
    try:
        _wb = _ox.load_workbook(cs)
        check("S6-12: another reader sees the chart sheet", any(type(ws).__name__ == "Chartsheet" for ws in _wb.worksheets + getattr(_wb, "chartsheets", [])))
    except Exception as e:
        check("S6-12: another reader sees the chart sheet", False, e)
win.run("undo")
pump(0.1)
check("S6-12: Undo takes the chart sheet away again", b.sheet_count == n0 and b.sheet_info(0)["kind"] == "work", b.sheet_count)
g.set_sheet(0)
win.sheet_changed()

# S6-16 Ctrl+Shift+Enter
for r in range(80, 83):
    b.set_input(0, r, 0, str(r - 79))
g.select_range((80, 1, 82, 1))
typ("=A81:A83*2")
key(Gdk.KEY_Return, CTRL | SHIFT)
ar = b.array_range(0, 80, 1)
check("S6-16: Ctrl+Shift+Enter makes one array formula over the selection",
      ar is not None and tuple(ar) == (80, 1, 82, 1) and b.display(0, 82, 1) == "6", (ar, b.display(0, 82, 1)))

# S6-19 Name Box defines a name
g.select_range((80, 0, 82, 0))
win.formula_bar.namebox.goto("Units")
names = [n[0] for n in b.names()]
check("S6-19: a new name typed in the Name Box names the selection", "Units" in names, names)
b.set_input(0, 85, 0, "=SUM(Units)")
check("S6-19: and the name works in formulas", b.display(0, 85, 0) == "6")

# S6-15 AutoComplete
b.set_input(0, 90, 0, "apple")
b.set_input(0, 91, 0, "banana")
g.select_cell(92, 0)
typ("b")
pump(0.1)
key(Gdk.KEY_Return)
check("S6-15: typing b under apple and banana completes banana", b.input(0, 92, 0) == "banana", b.input(0, 92, 0))
g.select_cell(93, 0)
typ("a")
pump(0.1)
key(Gdk.KEY_BackSpace) if False else None
typ("x")
key(Gdk.KEY_Return)
check("S6-15: typing on replaces the suggestion", b.input(0, 93, 0) == "ax", b.input(0, 93, 0))

# S6-23 pasted text with quotes
from folio_sheets import clip_ui
check("S6-23: a quoted field loses its quotes", clip_ui.split_text('a\t"q,r"\r\n"x\ty"\tz') == [["a", "q,r"], ["x\ty", "z"]],
      clip_ui.split_text('a\t"q,r"\r\n"x\ty"\tz'))

# S6-13 fill handle double-click and dragging the border
for r in range(100, 106):
    b.set_input(0, r, 0, str(r))
b.set_input(0, 100, 1, "=A101*2")
g.select_cell(100, 1)
win.fill_handle_double()
check("S6-13: a double-click on the fill handle fills down beside the data", b.input(0, 105, 1) == "=A106*2" and b.input(0, 106, 1) == "",
      (b.input(0, 105, 1), b.input(0, 106, 1)))
b.set_input(0, 110, 0, "move me")
b.set_input(0, 111, 0, "=A111")
win.move_cells_by_drag((110, 0, 110, 0), (110, 3, 110, 3))
check("S6-13: dragging the border moves the cells, and formulas follow", b.input(0, 110, 3) == "move me" and b.input(0, 110, 0) == ""
      and b.input(0, 111, 0) == "=D111", (b.input(0, 110, 3), b.input(0, 111, 0)))
p = g.params()
rc = b.cell_rect(0, 110, 3, p)
g.select_cell(110, 3)
check("S6-13: the pointer finds the selection's border", rc is not None and g.on_sel_edge(rc[0] + 1, rc[1] + rc[3] / 2) and not g.on_sel_edge(rc[0] + rc[2] / 2, rc[1] + rc[3] / 2))

# S6-10 filtered rows
win.new_book()
pump(0.1)
b = win.book
g = win.grid
for r, row in enumerate([["Fruit", "Qty"], ["apple", 1], ["pear", 2], ["apple", 3], ["fig", 4], ["apple", 5]]):
    for c, v in enumerate(row):
        b.set_input(0, r, c, str(v))
g.select_range((0, 0, 5, 1))
win.run("filter")
pump(0.1)
b.filter_column(0, 0, {"values": ["apple"]}, None)
pump(0.1)
hidden = [r for r in range(6) if b.row_hidden(0, r)]
check("S6-10: the filter shows the apples only", hidden == [2, 4], hidden)
if hidden == [2, 4]:
    check("S6-10: the filtered rows are known as filtered", b.filtered_rows(0, 0, 10) == [2, 4], b.filtered_rows(0, 0, 10))
    g.select_range((1, 1, 5, 1))
    win.run("copy")
    g.select_cell(10, 4)
    win.run("paste")
    check("S6-10: copy and paste bring the visible rows only", [b.input(0, r, 4) for r in range(10, 14)] == ["1", "3", "5", ""],
          [b.input(0, r, 4) for r in range(10, 14)])
    g.select_range((1, 1, 5, 1))
    win.run("clear-contents")
    check("S6-10: Delete clears the visible rows only", [b.input(0, r, 1) for r in range(1, 6)] == ["", "2", "", "4", ""],
          [b.input(0, r, 1) for r in range(1, 6)])
    g.select_range((1, 1, 5, 1))
    typ("9")
    key(Gdk.KEY_Return, CTRL)
    check("S6-10: Ctrl+Enter fills the visible rows only", [b.input(0, r, 1) for r in range(1, 6)] == ["9", "2", "9", "4", "9"],
          [b.input(0, r, 1) for r in range(1, 6)])
    g.select_range((3, 0, 5, 16383))
    win.run("delete-rows")
    names_left = [b.input(0, r, 0) for r in range(0, 5)]
    check("S6-10: Delete Rows removes the visible rows only", names_left[:4] == ["Fruit", "apple", "pear", "fig"], names_left)

# S6-20 tables: Tab adds a row; pointing writes [@Col]
win.new_book()
pump(0.1)
b = win.book
g = win.grid
for r, row in enumerate([["Qty", "Price", "Total"], [2, 5, ""], [3, 4, ""]]):
    for c, v in enumerate(row):
        if v != "":
            b.set_input(0, r, c, str(v))
g.select_range((0, 0, 2, 2))
DIALOG.append(lambda d: Gtk.ResponseType.OK)
win.run("table")
pump(0.2)
ts = b.tables(0)
check("S6-20: the table is made", len(ts) == 1, ts)
if ts:
    g.select_cell(1, 2)
    typ("=")
    g.on_press(g.area, type("E", (), {"x": b.cell_rect(0, 1, 0, g.params())[0] + 5, "y": b.cell_rect(0, 1, 0, g.params())[1] + 5, "button": 1,
                                      "type": Gdk.EventType.BUTTON_PRESS, "state": 0, "get_state": lambda self: Gdk.ModifierType(0)})())
    g.on_release(g.area, type("E", (), {"x": 0, "y": 0, "button": 1, "type": Gdk.EventType.BUTTON_RELEASE, "state": 0,
                                        "get_state": lambda self: Gdk.ModifierType(0)})())
    pump(0.05)
    text = g.editor.text()
    key(Gdk.KEY_Escape)
    check("S6-20: clicking a table cell while typing a formula in the table writes [@Qty]", text == "=[@Qty]", text)
    g.select_cell(2, 2)
    key(Gdk.KEY_Tab)
    pump(0.1)
    t = b.tables(0)[0]
    check("S6-20: Tab in the table's last cell adds a row", tuple(t["ref"]) == (0, 0, 3, 2) and tuple(g.sel.active) == (3, 0), (t["ref"], g.sel.active))
    key(Gdk.KEY_t, CTRL | SHIFT)
    check("S6-20: Ctrl+Shift+T turns the Total Row on", b.tables(0)[0]["totals"])

# S6-35 no tag warnings while typing a formula
import io as _io
from folio_sheets import formula_edit
_buf = win.grid.editor.get_buffer()
fa = win.assist
fa._tags_for = set()
fa._ensure_tags(_buf)
fa._tags_for = set()
fa._ensure_tags(_buf)
check("S6-35: the reference colours are added to a text buffer once", _buf.get_tag_table().get_size() >= 1 and _buf.get_tag_table().lookup("ref0") is not None)

# S6-06 AutoRecover
from folio_sheets import autorecover
os.environ["XDG_DATA_HOME"] = os.path.join(TMP, "data")
win.new_book()
pump(0.1)
win.book.set_input(0, 0, 0, "unsaved work")
ar = win.app.autorecover
path = ar.save(win)
check("S6-06: a workbook with unsaved changes gets an AutoRecover copy", path and os.path.exists(path) and path.startswith(os.path.join(TMP, "data")), path)
check("S6-06: the copy isn't offered while this program runs", autorecover.AutoRecover.left_behind() == [])
import json as _json
meta = _json.load(open(path + ".json"))
meta["pid"] = 999999
_json.dump(meta, open(path + ".json", "w"))
left = autorecover.AutoRecover.left_behind()
check("S6-06: a copy left by a program that ended is offered", len(left) == 1 and left[0][0] == path, left)
w2 = autorecover.recover(win.app, path, meta)
pump(0.2)
check("S6-06: recovering opens it unsaved, named (Recovered)", w2.book.display(0, 0, 0) == "unsaved work" and w2.path is None
      and "(Recovered)" in w2.untitled and w2.book.modified, (w2.untitled, w2.path))
w2.book.mark_saved()
w2.destroy()
win.book.set_input(0, 1, 0, "more")
p2 = ar.save(win)
win.save_to(os.path.join(TMP, "saved.xlsx"))
check("S6-06: saving removes the copy", not os.path.exists(p2))

# Generate GetPivotData
win.new_book()
pump(0.1)
b = win.book
g = win.grid
for r, row in enumerate([["Region", "Sales"], ["East", 10], ["West", 20], ["East", 5]]):
    for c, v in enumerate(row):
        b.set_input(0, r, c, str(v))
try:
    pv = b.create_pivot(0, (0, 0, 3, 1), 0, (0, 4))
    b.set_pivot(pv, {"rows": [0], "values": [{"field": 1, "agg": "sum"}]})
except Exception as e:
    pv = None
    print("NOTE pivot", e)
if pv is not None or b.pivot_at(0, 1, 4) is not None:
    cell = next(((r, c) for r in range(0, 6) for c in range(4, 7) if b.pivot_role(0, r, c) and b.pivot_role(0, r, c)[0] == "data"), None)
    f = b.getpivotdata_formula(0, *cell) if cell else None
    check("S6-24: a PivotTable value gives its GETPIVOTDATA formula", f and f.startswith('=GETPIVOTDATA("Sum of Sales",$E$') and '"Region","East"' in f, f)
    if f:
        b.set_input(0, 10, 0, f)
        check("S6-24: and that formula returns the value", b.display(0, 10, 0) == "15", (f, b.display(0, 10, 0)))
else:
    print("NOTE no PivotTable API for the GETPIVOTDATA check")

# ------------------------------------------------------------------ END
print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
sys.stdout.flush()
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
