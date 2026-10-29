# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): S-08 — light or dark as the desktop is set.
# Excel-like dark mode: ribbon, headers, formula bar, sheet tabs and panes go dark; cells keep
# their own colours. Switching happens while running.
import os
import sys
import time
import traceback

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


from folio_sheets import appearance, icons  # noqa: E402

win.resize(1280, 800)
Gtk.Settings.get_default().set_property("gtk-theme-name", "Adwaita")
pump(0.5)
OUT = os.environ.get("AUDIT_OUT", "/tmp")


import cairo  # noqa: E402


def grab():
    # through cairo: Gdk.pixbuf_get_from_window trips a cairo assertion on some Xvfb setups
    pump(0.4)
    gw = win.get_window()
    img = cairo.ImageSurface(cairo.FORMAT_ARGB32, gw.get_width(), gw.get_height())
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, gw, 0, 0)
    cr.paint()
    img.flush()
    return img


def px(img, x, y):
    d = img.get_data()
    o = int(y) * img.get_stride() + int(x) * 4
    return d[o + 2], d[o + 1], d[o]


def at(w, fx, fy):
    r = w.translate_coordinates(win, int(w.get_allocated_width() * fx), int(w.get_allocated_height() * fy))
    return r[-2:]


def lum(c):
    return (c[0] * 299 + c[1] * 587 + c[2] * 114) / 1000


g = win.grid
area = g.area


def header_px(pb):
    x, y = at(area, 0, 0)
    return px(pb, x + 200, y + 4)      # in the column headers, over column C (A is selected)


def cell_px(pb):
    x, y = at(area, 0, 0)
    return px(pb, x + 300, y + 200)


ribbon = getattr(win, "ribbon", None)
start_dark = appearance.is_dark()
appearance.set_dark(False)
pb = grab()
check("light: ribbon light", ribbon is not None and lum(px(pb, *at(ribbon, 0.995, 0.5))) > 200, ribbon and px(pb, *at(ribbon, 0.995, 0.5)))
check("light: headers light", lum(header_px(pb)) > 200, header_px(pb))
check("light: cells white", cell_px(pb) == (255, 255, 255), cell_px(pb))
print("icons registered:", len(icons._images), sum(1 for i in icons._images if i.get_mapped()))
some_icon = next((img for img in list(icons._images) if img.get_mapped()
                  and os.path.exists(os.path.join(icons.OWN, img._folio_icon[0].replace(".uno:", "").lower() + ".svg"))), None)
before = some_icon.get_pixbuf().get_pixels() if some_icon else None

appearance.set_dark(True)
pb = grab()
pb.write_to_png(os.path.join(OUT, "sheets_dark.png"))
check("dark: GTK's own widgets go dark", Gtk.Settings.get_default().get_property("gtk-application-prefer-dark-theme"))
check("dark: ribbon dark", lum(px(pb, *at(ribbon, 0.995, 0.5))) < 70, px(pb, *at(ribbon, 0.995, 0.5)))
check("dark: column and row headers dark", lum(header_px(pb)) < 70, header_px(pb))
check("dark: the cells keep their colours (white)", cell_px(pb) == (255, 255, 255), cell_px(pb))
print("icon:", some_icon and some_icon._folio_icon, before is not None and len(before), some_icon and len(some_icon.get_pixbuf().get_pixels()))
check("dark: icons redrawn in light ink", some_icon is not None and some_icon.get_pixbuf().get_pixels() != before)
fb = getattr(win, "formula_bar", None)
if fb is not None:
    check("dark: formula bar dark", lum(px(pb, *at(fb, 0.5, 0.1))) < 80, px(pb, *at(fb, 0.5, 0.1)))
tabs = getattr(win, "sheettabs", None) or getattr(win, "tabs", None)
if tabs is not None and isinstance(tabs, Gtk.Widget):
    check("dark: sheet tab bar dark", lum(px(pb, *at(tabs, 0.9, 0.5))) < 80, px(pb, *at(tabs, 0.9, 0.5)))
win.show_backstage()
pbb = grab()
pbb.write_to_png(os.path.join(OUT, "sheets_dark_file.png"))
check("dark: the File view is dark", lum(px(pbb, pbb.get_width() - 30, pbb.get_height() - 30)) < 70, px(pbb, pbb.get_width() - 30, pbb.get_height() - 30))
win.backstage.close()
pump(0.2)
# a coloured cell keeps its colour
b = win.book
try:
    win.grid.state().sel.set_active(2, 2) if hasattr(win.grid.state().sel, "set_active") else None
except Exception:
    pass

appearance.set_dark(False)
pb = grab()
check("back to light: headers light", lum(header_px(pb)) > 200, header_px(pb))
check("back to light: ribbon light", lum(px(pb, *at(ribbon, 0.995, 0.5))) > 200)
check("back to light: icons as before", some_icon is not None and some_icon.get_pixbuf().get_pixels() == before)

os.environ["FOLIO_THEME"] = "dark"
check("FOLIO_THEME=dark is read as dark", appearance.system_prefers_dark())
os.environ["FOLIO_THEME"] = "light"
check("FOLIO_THEME=light is read as light", not appearance.system_prefers_dark())
del os.environ["FOLIO_THEME"]
appearance.set_dark(start_dark)

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
