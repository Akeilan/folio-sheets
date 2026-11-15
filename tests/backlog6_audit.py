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

# ------------------------------------------------------------------ END
print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
sys.stdout.flush()
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
