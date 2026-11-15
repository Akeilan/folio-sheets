exec(open("/tmp/b6/uiharn.py").read())
import subprocess
from folio_sheets import print_ui
def pdf(name, sheets=None):
    p = "/tmp/b6/%s.pdf" % name
    try: print_ui.write_pdf(win, p, sheets)
    except Exception as e: print("FAIL write_pdf %s raised %s" % (name, str(e)[:150]), flush=True); N[1] += 1; return None, ""
    inf = subprocess.run(["pdfinfo", p], capture_output=True, text=True).stdout
    pages = int([l for l in inf.splitlines() if l.startswith("Pages:")][0].split()[1]); size = [l for l in inf.splitlines() if l.startswith("Page size")][0]
    return pages, size
win.load("/tmp/b6/gallery.xlsx"); pump(0.3); g = win.grid; b = win.book
pg, size = pdf("gallery"); info("gallery pdf: pages, size", (pg, size)); subprocess.run(["pdftoppm", "-r", "70", "-png", "/tmp/b6/gallery.pdf", "/tmp/b6/galpdf"])
win.load("/tmp/b6/rt_in.xlsx"); pump(0.3); g = win.grid; b = win.book
pg, size = pdf("rt"); info("rich workbook pdf (landscape A4, fit to 1 page wide, print area A1:F7, titles row 1): pages, size", (pg, size)); subprocess.run(["pdftoppm", "-r", "70", "-png", "/tmp/b6/rt.pdf", "/tmp/b6/rtpdf"])
txt = subprocess.run(["pdftotext", "-layout", "/tmp/b6/rt.pdf", "-"], capture_output=True, text=True).stdout; info("text of page 1", txt[:400])
# a long sheet: page count, repeated titles, page numbers
win.new_book(); pump(0.2); g = win.grid; b = win.book
for r in range(200):
    for c in range(12): b.set_input(0, r, c, "H%d" % c if r == 0 else str(r * c))
pg, size = pdf("long"); info("200 rows x 12 cols, defaults: pages, size (Excel Letter portrait: 8 pages = 4 down x 2 across)", (pg, size))
info("page_setup", b.page_setup(0)); info("pages()", (type(b).pages.__doc__, ))
done()
