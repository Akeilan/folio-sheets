# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Page Layout commands, Page Setup, page
# breaks, Page Break Preview, the Backstage Print page, printing and Export to PDF.
import os
import sys
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


SCRIPT = []


def fake_run(d):
    pump(0.05)
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    return fn(d)


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run


def find(w, name):
    if w.get_name() == name:
        return w
    if isinstance(w, Gtk.Container):
        for c in w.get_children():
            r = find(c, name)
            if r is not None:
                return r
    return None


g = win.grid
b = win.book
S = 0
b.set_input(S, 0, 0, "Item")
b.set_input(S, 0, 1, "Amount")
for r in range(1, 150):
    b.set_input(S, r, 0, "Row %d" % r)
    b.set_input(S, r, 1, str(r * 3))
    b.set_input(S, r, 9, str(r))
n0 = len(b.pages(S))
check("the sheet prints on several pages", n0 >= 4, n0)

win.run("page-orientation:landscape")
check("Orientation: Landscape", b.page_setup(S)["landscape"])
win.run("margins:narrow")
check("Margins: Narrow", b.page_setup(S)["margins"][0] == 0.25)
win.run("page-size:a4")
check("Size: A4", b.page_setup(S)["paper"] == 9)
win.run("scale-width:1")
check("Scale to Fit: 1 page wide", b.page_setup(S)["fit"] == (1, 0) and all(p[1] == (0, 9) for p in b.pages(S)), b.pages(S)[:2])
win.run("print-gridlines")
check("Print gridlines", b.page_setup(S)["gridlines"])
win.run("undo")
check("undo", not b.page_setup(S)["gridlines"])

# breaks and print area
g.select_cell(40, 0)
win.run("breaks:insert")
check("Insert Page Break above the cell", 40 in b.page_setup(S)["row_breaks"] and any(p[0][0] == 40 for p in b.pages(S)))
win.run("breaks:reset")
check("Reset All Page Breaks", b.page_setup(S)["row_breaks"] == [])
g.sel.ranges = [(0, 0, 20, 1)]
g.sel.active = (0, 0)
win.run("print-area:set")
check("Set Print Area", b.page_setup(S)["print_area"] == [(0, 0, 20, 1)] and len(b.pages(S)) == 1)
win.run("print-area:clear")
check("Clear Print Area", b.page_setup(S)["print_area"] == [])


# Page Setup: titles, header and footer, fit one page wide
def setup(d):
    find(d, "title-rows").set_text("$1:$1")
    find(d, "footer").set_text("&CPage &P of &N")
    find(d, "header").set_text("&L&A")
    find(d, "center-h").set_active(True)
    return Gtk.ResponseType.OK


SCRIPT.append(setup)
win.run("page-setup-dialog")
ps = b.page_setup(S)
check("Page Setup: rows to repeat, footer, header, centering",
      ps["title_rows"] == (0, 0) and ps["footer"] == "&CPage &P of &N" and ps["header"] == "&L&A" and ps["center_h"], ps)

# Page Break Preview
win.run("view-page-break")
check("Page Break Preview", g.params()["page_breaks"])
pump(0.1)
win.run("view-normal")
check("back to Normal", not g.params()["page_breaks"])

# Export to PDF
pdf = "/tmp/print_audit.pdf"
from folio_sheets import print_ui
n = print_ui.write_pdf(win, pdf)
data = open(pdf, "rb").read()
def pdf_pages(path):
    # cairo 1.18 (Ubuntu 24.04) packs objects into compressed streams, so count with Poppler
    import gi
    gi.require_version("Poppler", "0.18")
    from gi.repository import Poppler
    return Poppler.Document.new_from_file("file://" + path, None).get_n_pages()


check("Export to PDF writes every page", n == len(b.pages(S)) and data.startswith(b"%PDF") and pdf_pages(pdf) == n, (n, len(data)))

# printing, through the print system (exported to a file)
out = "/tmp/print_audit_print.pdf"
if os.path.exists(out):
    os.remove(out)
print_ui.print_sheet(win, export_to=out)
check("printing goes through the print system", os.path.exists(out) and open(out, "rb").read(4) == b"%PDF")

# the Backstage Print page
win.run("print")
pump(0.2)
pv = win.backstage.print_view
check("Ctrl+P's page shows a preview of the pages", pv is not None and pv.label.get_text() == "1 of %d" % len(b.pages(S)), pv.label.get_text())
find(pv, "scaling").set_active_id("sheet")
pump(0.05)
check("Fit Sheet on One Page from the print page", b.page_setup(S)["fit"] == (1, 1) and len(b.pages(S)) == 1, b.page_setup(S)["fit"])
win.backstage.close()

# saved and opened again
path = "/tmp/print_audit.xlsx"
win.save_to(path)
from folio_sheets import core
b2 = core.Book.open(path)
p2 = b2.page_setup(0)
check("page setup is saved", p2["landscape"] and p2["paper"] == 9 and p2["fit"] == (1, 1) and p2["title_rows"] == (0, 0)
      and p2["footer"] == "&CPage &P of &N", p2)

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
