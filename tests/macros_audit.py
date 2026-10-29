# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): SH-24 macro-enabled workbooks. The macros stay in
# the file and are never run: the SECURITY WARNING bar, the Macro dialog (Alt+F8), saving as .xlsm
# keeps them, saving as .xlsx asks first and leaves them out, Save As offers the macro types.
import os
import sys
import tempfile
import time
import traceback
import zipfile

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


from folio_sheets import macros_ui  # noqa: E402

HERE = os.path.dirname(os.path.abspath(os.environ.get("FOLIO_TEST_SCRIPT", "tests/macros_audit.py")))
SRC = os.path.join(HERE, "data", "macros.xlsm")
tmp = tempfile.mkdtemp()
INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
ANSWER = {"macro_free": Gtk.ResponseType.YES}
ASKED = []
_md_run = Gtk.MessageDialog.run


def md_run(d):
    ASKED.append(d.get_property("text"))
    return ANSWER["macro_free"]


Gtk.MessageDialog.run = md_run
win.resize(1280, 800)
pump(0.3)
bar = win.macro_bar
check("no warning on a workbook without macros", not bar.get_visible())

win.load(SRC)
pump(0.3)
check("a macro workbook opens", win.book is not None and win.book.has_macros())
check("the SECURITY WARNING bar shows", bar.get_visible() and "Macros have been disabled" in bar.text.get_text()
      and bar.text.get_mapped() and bar.enable.get_mapped())
names = [m[0] for m in win.book.macros()]
check("its macros are read from the VBA project", names == ["Macro1"], names)

# Enable Content: explained, not run
bar.enable_content()
pump()
check("Enable Content explains macros aren't run", INFO and "doesn't run macros" in INFO[-1][1], INFO[-1:])
check("and the bar goes away", not bar.get_visible())

# the Macro dialog
d = macros_ui.MacroDialog(win)
pump()
rows = [r[0] for r in d.store]
check("View Macros lists the macros", rows == ["Macro1"], rows)
check("with the first one chosen", d.name.get_text() == "Macro1")
check("Run, Step Into, Edit and Create are off", all(not d.buttons[k].get_sensitive() for k in ("Run", "Step Into", "Edit", "Create")))
check("and it says why", "doesn't run macros" in d.note.get_text())
d.destroy()
opened = []
_orig_show = macros_ui.show_macros
macros_ui.show_macros = lambda w: opened.append(w)
win.run("macros")
check("View tab › Macros › View Macros opens it", opened == [win])
macros_ui.show_macros = _orig_show
for cmd in ("record-macro", "relative-refs", "visual-basic"):
    n = len(INFO)
    win.run(cmd)
    check("%s says it isn't available" % cmd, len(INFO) == n + 1 and "isn't available" in INFO[-1][1], INFO[-1:])

# saving as .xlsm keeps the project, byte for byte
orig_vba = zipfile.ZipFile(SRC).read("xl/vbaProject.bin")
win.book.set_input(0, 4, 1, "kept")
xm = os.path.join(tmp, "kept.xlsm")
ASKED.clear()
check("save as .xlsm", win.save_to(xm) and not ASKED)
z = zipfile.ZipFile(xm)
check("the macros are in the saved file, unchanged", z.read("xl/vbaProject.bin") == orig_vba)
check("as a macro-enabled workbook", b"sheet.macroEnabled.main+xml" in z.read("[Content_Types].xml"))
check("the open workbook still has them", win.book.has_macros())

# Save As offers the macro-enabled types, and starts from .xlsm for an .xlsm
seen = {}


class FakeChooser:
    def __init__(self, **k):
        self.filters = []
        self.cur = None
        self.name = None

    def set_do_overwrite_confirmation(self, v):
        pass

    def add_filter(self, f):
        self.filters.append(f)

    def set_filter(self, f):
        self.cur = f

    def set_current_name(self, n):
        self.name = n

    def set_current_folder(self, f):
        pass

    def run(self):
        seen["filters"] = [f.get_name() for f in self.filters]
        seen["current"] = self.cur.get_name()
        seen["name"] = self.name
        return Gtk.ResponseType.CANCEL

    def get_filename(self):
        return None

    def get_filter(self):
        return self.cur

    def destroy(self):
        pass


_fcn = Gtk.FileChooserNative
Gtk.FileChooserNative = FakeChooser
win.act_save_as()
Gtk.FileChooserNative = _fcn
check("Save As lists Excel Macro-Enabled Workbook (*.xlsm)", "Excel Macro-Enabled Workbook (*.xlsm)" in seen.get("filters", []), seen)
check("and the templates", "Excel Template (*.xltx)" in seen.get("filters", []) and "Excel Macro-Enabled Template (*.xltm)" in seen.get("filters", []))
check("an .xlsm starts as .xlsm", seen.get("current") == "Excel Macro-Enabled Workbook (*.xlsm)" and seen.get("name", "").endswith(".xlsm"), seen)

# saving as .xlsx: Excel's question; No goes back to choosing a macro-enabled type
xx = os.path.join(tmp, "free.xlsx")
ANSWER["macro_free"] = Gtk.ResponseType.NO
again = []
_sa = win.act_save_as
win.act_save_as = lambda fmt=None: again.append(fmt) or False
ASKED.clear()
win.save_to(xx)
win.act_save_as = _sa
check("saving it as .xlsx asks first", ASKED and "cannot be saved in macro-free workbooks" in ASKED[0], ASKED)
check("No: back to Save As with the macro-enabled type", again == ["xlsm"] and not os.path.exists(xx), again)
ANSWER["macro_free"] = Gtk.ResponseType.YES
ASKED.clear()
check("Yes: saved macro-free", win.save_to(xx) and ASKED)
z = zipfile.ZipFile(xx)
check("without the VBA project", "xl/vbaProject.bin" not in z.namelist() and b"macroEnabled" not in z.read("[Content_Types].xml"))
check("the open workbook no longer has macros (as in Excel)", not win.book.has_macros() and not bar.get_visible())
ASKED.clear()
win.save_to(os.path.join(tmp, "again.xlsx"))
check("so saving again doesn't ask", not ASKED)

# the kept copy opens with its macros and the edit
win.load(xm)
pump(0.2)
check("the .xlsm opens again with its macros and the edit", win.book.has_macros() and win.book.display(0, 4, 1) == "kept"
      and bar.get_visible())

Gtk.MessageDialog.run = _md_run
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
