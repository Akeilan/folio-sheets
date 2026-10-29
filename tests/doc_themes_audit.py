# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): SH-22 document themes — Page Layout › Themes, Colors,
# Fonts, Effects; the palette and everything in theme colours and fonts follow; custom sets; .thmx files.
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
from folio_sheets import themes_ui as T  # noqa: E402
from folio_sheets import ribbon_defs as R  # noqa: E402

INFO = []
win.dialogs.info = lambda text, title="": INFO.append((title, text))
win.dialogs.message = lambda text, title="": INFO.append((title, text))
tmp = tempfile.mkdtemp()
win._theme_dir = os.path.join(tmp, "themes")
win.resize(1280, 800)
pump(0.4)
g = win.grid
area = g.area
b = lambda: win.book  # noqa: E731
S = g.sheet


def px(r, c):
    pump(0.2)
    w = area.get_window()
    img = cairo.ImageSurface(cairo.FORMAT_ARGB32, w.get_width(), w.get_height())
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, w, 0, 0)
    cr.paint()
    img.flush()
    x, y, ww, hh = b().cell_rect(S, r, c, g.params())
    d = img.get_data()
    o = int(y + hh / 2) * img.get_stride() + int(x + 4) * 4
    return (d[o + 2] << 16) | (d[o + 1] << 8) | d[o]


def near(a, e, tol=6):
    return all(abs(((a >> s) & 255) - ((e >> s) & 255)) <= tol for s in (16, 8, 0))


def click_row(pop, label):
    for row in getattr(pop, "rows", []) + getattr(pop, "cards", []):
        if getattr(row, "label_text", None) == label or getattr(row, "theme_name", None) == label:
            row.clicked()
            pump(0.1)
            return True
    return False


info = b().theme_info()
check("a new workbook has the default theme's colours and fonts", info["colors"][4] == 0x4472C4
      and info["minor"] == "Calibri", info)
check("the palette shows the workbook's theme", R.ACTIVE_THEME[4] == 0x4472C4)

# ---- the palette picks theme colours as theme references
pop = Gtk.Popover()
box = R.color_palette(lambda c: None)
grid = [w for w in box.get_children() if isinstance(w, Gtk.Grid)][0]
vals = [c.value for c in grid.get_children()]
check("the palette's theme columns are theme references", all(R.split_ref(v) for v in vals) and len(vals) == 60)
accent1_40 = R.theme_ref(4, 0.4)
check("and they're named as in the desktop spreadsheet", R.color_name(accent1_40) == "Accent 1, Lighter 40%")
g.sel.ranges = [(0, 0, 0, 0)]
g.sel.active = (0, 0)
win.run("fill-color:%X" % R.theme_ref(4))
b().set_input(S, 0, 1, "Title")
g.sel.ranges = [(0, 1, 0, 1)]
g.sel.active = (0, 1)
from folio_sheets.format_cells import CELL_STYLES  # noqa: E402
title = [c for h, items in CELL_STYLES for n, c in items if n == "Title"][0]
win.apply_style("Cell Style: Title", title)
accent = [c for h, items in CELL_STYLES for n, c in items if n == "40% - Accent2"]
check("the themed cell styles include 40% - Accent", len(accent) == 1)
g.refresh()
check("an accent fill shows in the accent colour", near(px(0, 0), 0x4472C4), "%06X" % px(0, 0))
fill = b().cell_style(S, 0, 0)["fill"]
spec = b().style_spec(S, 0, 0)
check("and is kept as a theme colour", R.split_ref(spec["fill"]["fg"]) == (4, 0.0), spec["fill"])

# a shape and a sparkline in theme colours
i = b().insert_shape(S, "rect", 400, 200, 80, 40, False, False)
b().shape_set_fill(S, [i], R.theme_ref(5))
for c in range(4):
    b().set_input(S, 3, c, str([3, 1, 4, 2][c]))
b().add_sparklines(S, "column", "A4:D4", "E4")

# ---- Colors
pop = T.colors_menu(win)
names = [r.label_text for r in pop.rows]
check("Colors lists the colour sets", names[:3] == ["Office", "Office 2023", "Office 2007 - 2010"] and "Green" in names
      and len(names) >= 20, names[:5])
click_row(pop, "Green")
info = b().theme_info()
check("picking one changes the theme's colours", info["colors_name"] == "Green" and info["colors"][4] == 0x549E39)
check("the fonts stay", info["minor"] == "Calibri")
check("cells in theme colours follow", near(px(0, 0), 0x549E39), "%06X" % px(0, 0))
check("so does the palette", R.ACTIVE_THEME[4] == 0x549E39)
check("and shapes", b().shape_info(S, i)["fill"] == 0x8AB833, b().shape_info(S, i)["fill"])
sp = b().sparklines(S)[0]
check("and sparklines' default colours are theme colours", R.split_ref(sp["color"]) == (4, -0.5))
win.run("undo")
check("undo puts the colours back", b().theme_info()["colors"][4] == 0x4472C4 and R.ACTIVE_THEME[4] == 0x4472C4)
win.run("redo")
check("redo again", b().theme_info()["colors_name"] == "Green")

# ---- Fonts
pop = T.fonts_menu(win)
names = [r.label_text for r in pop.rows]
check("Fonts lists the font sets", "Office" in names and "Georgia" in names and "Franklin Gothic" in names, names[:4])
click_row(pop, "Georgia")
info = b().theme_info()
check("picking one changes headings and body fonts", info["major"] == "Georgia" and info["minor"] == "Georgia")
check("cells in the body font follow", b().cell_style(S, 5, 5)["font"] == "Georgia", b().cell_style(S, 5, 5)["font"])
check("the Title cell style is in the headings font, and follows it", b().cell_style(S, 0, 1)["font"] == "Georgia",
      b().cell_style(S, 0, 1)["font"])

# ---- Themes
pop = T.themes_menu(win)
cards = [c.theme_name for c in pop.cards]
check("Themes shows this workbook's theme and the ready-made ones", "Facet" in cards and "Office Theme" in cards, cards)
click_row(pop, "Facet")
info = b().theme_info()
check("a theme changes colours and fonts together", info["name"] == "Facet" and info["colors"][4] == 0x90C226
      and info["minor"] == "Trebuchet MS")

# ---- Customize Colors / Fonts
d = T.ColorsDialog(win)
check("Customize Colors starts from the theme's colours", d.colors()[4] == 0x90C226)
d.set_color(4, 0xC00000)
d.name.set_text("My Colors")
d.apply()
d.destroy()
info = b().theme_info()
check("Save makes a colour set and uses it", info["colors_name"] == "My Colors" and info["colors"][4] == 0xC00000)
pop = T.colors_menu(win)
check("it's listed under Custom next time", pop.rows[0].label_text == "My Colors")
d = T.FontsDialog(win)
d.major.get_child().set_text("DejaVu Serif")
d.minor.get_child().set_text("DejaVu Sans")
d.name.set_text("My Fonts")
d.apply()
d.destroy()
info = b().theme_info()
check("Customize Fonts makes a font set and uses it", (info["fonts_name"], info["major"], info["minor"]) == ("My Fonts", "DejaVu Serif", "DejaVu Sans"))
check("its fonts show in the cells", b().cell_style(S, 5, 5)["font"] == "DejaVu Sans")

# ---- Effects
pop = T.effects_menu(win)
labels = []


def walk(w):
    if isinstance(w, Gtk.Label):
        labels.append(w.get_text())
    if isinstance(w, Gtk.Container):
        for c in w.get_children():
            walk(c)


walk(pop)
check("Effects offers the effect schemes (S3-11)",
      all(any(t.startswith(n) for t in labels) for n in ("Office", "Subtle Solids", "Glossy", "Milk Glass", "Extreme Shadow")), labels)
gl = [r for r in pop.rows if r.get_name() == "effects:Glossy"]
check("Glossy is in the gallery", gl)
if gl:
    gl[0].clicked()
    check("picking Glossy: the effects change, colours and fonts stay", b().theme_info()["effects_name"] == "Glossy" and b().theme_info()["colors_name"] == "My Colors",
          b().theme_info())
    pop = T.effects_menu(win)
pop.rows[0].clicked()
check("picking Office keeps colours and fonts", b().theme_info()["colors_name"] == "My Colors" and b().theme_info()["effects_name"] == "Office")

# ---- Save Current Theme / Browse for Themes
path = T.save_current(win, os.path.join(win._theme_dir, "Document Themes", "Mine.thmx"))
check("Save Current Theme writes a theme file", path and os.path.getsize(path) > 500)
info_saved = b().theme_info()
b().apply_theme("Office Theme")
T.changed(win)
check("(back to the default)", b().theme_info()["colors"][4] == 0x4472C4)
pop = T.themes_menu(win)
check("saved themes are listed under Custom", "Mine" in [c.theme_name for c in pop.cards])
T.browse(win, path)
info = b().theme_info()
check("Browse for Themes uses a theme file", info["colors"] == info_saved["colors"] and info["major"] == "DejaVu Serif")

# a workbook's theme
other = os.path.join(tmp, "other.xlsx")
win.book.apply_theme("Ion")
win.save_to(other)
win.book.apply_theme("Office Theme")
T.changed(win)
T.browse(win, other)
check("or another workbook's theme", b().theme_info()["name"] == "Ion")
T.browse(win, os.path.join(tmp, "nope.txt"))
check("a file without a theme says so", INFO and "couldn't use the theme" in INFO[-1][1])

# ---- saved with the workbook
p2 = os.path.join(tmp, "themed.xlsx")
win.save_to(p2)
again = Book.open(p2)
check("the theme is saved with the workbook", again.theme_info()["name"] == "Ion")
import zipfile  # noqa: E402
st = zipfile.ZipFile(p2).read("xl/styles.xml").decode()
check("theme colours are saved as theme colours", '<fgColor theme="4"' in st, st[:200])
names = zipfile.ZipFile(p2).namelist()
check("with its theme part", "xl/theme/theme1.xml" in names)

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
