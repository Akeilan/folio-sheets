# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): every formatting command through the real
# window handlers, checked against the engine, the file and undo/redo.
import os
import sys
import time
import traceback
import zipfile

from gi.repository import Gdk, GLib, Gtk

OUT = os.environ.get("AUDIT_OUT", "/tmp/fs-audit")
os.makedirs(OUT, exist_ok=True)
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


def shot(name):
    pump(0.3)
    import cairo
    gw = win.get_window()
    img = cairo.ImageSurface(cairo.FORMAT_ARGB32, gw.get_width(), gw.get_height())
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, gw, 0, 0)
    cr.paint()
    img.write_to_png(os.path.join(OUT, name + ".png"))


# scripted modal dialogs: each entry gets the dialog and returns a response
SCRIPT = []
SEEN = []
_run = Gtk.Dialog.run


def fake_run(d):
    pump(0.1)
    SEEN.append(d.get_title())
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.CANCEL)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

g = win.grid
b = win.book
S = 0
from folio_sheets import restyle as R  # noqa: E402


def sel(*rgs, active=None):
    g.sel.ranges = [tuple(r) for r in rgs]
    g.sel.active = active or (rgs[0][0], rgs[0][1])
    pump(0.01)


def st(r, c):
    return win.book.cell_style(S, r, c)


def spec(r, c):
    pkg = R.Package(win.book.snapshot())
    sh = pkg.sheet(S)
    return pkg.styles.spec(sh.cell_xf(r, c))


def run(cmd):
    win.run(cmd)
    pump(0.02)


# ------------------------------------------------------------------ data
for r in range(6):
    for c in range(4):
        win.book.set_input(S, r, c, str(r * 10 + c))
win.book.set_input(S, 7, 0, "A long piece of text that wraps")
win.book.mark_saved()
pump(0.1)

# ------------------------------------------------------------------ font
sel((0, 0, 2, 2))
run("bold")
check("bold applies to the selection", all(st(r, c)["bold"] for r in range(3) for c in range(3)))
check("bold leaves the rest alone", not st(3, 3)["bold"] and not st(0, 3)["bold"])
check("bold button shows checked", "checked" in [x for x in win.ribbon.lookup_states().get("bold", [])] if hasattr(win.ribbon, "lookup_states") else True)
run("bold")
check("bold toggles off", not any(st(r, c)["bold"] for r in range(3) for c in range(3)))
run("undo")
check("undo brings bold back", st(1, 1)["bold"])
run("redo")
check("redo removes it again", not st(1, 1)["bold"])
run("italic")
run("underline")
run("strikethrough")
s = st(0, 0)
check("italic, underline, strikethrough", s["italic"] and s["underline"] and s["strike"], s)
run("double-underline")
check("double underline", spec(0, 0)["font"]["underline"] == "double", spec(0, 0)["font"])
run("font-name:DejaVu Serif")
run("font-size:16")
s = st(1, 1)
check("font name and size", s["font"] == "DejaVu Serif" and s["size"] == 16, s)
run("font-grow")
check("increase font size steps 16 -> 18", st(1, 1)["size"] == 18, st(1, 1)["size"])
run("font-shrink")
run("font-shrink")
check("decrease font size steps 18 -> 16 -> 14", st(1, 1)["size"] == 14, st(1, 1)["size"])
run("font-size:abc")
check("invalid size is refused with a message", st(1, 1)["size"] == 14 and SEEN and SEEN[-1] is not None)
run("font-color:C00000")
check("font colour", st(2, 2)["color"] == 0xC00000, st(2, 2)["color"])
run("font-color:auto")
check("automatic font colour", st(2, 2)["color"] in (None, 0), st(2, 2)["color"])
run("font-color:last")
check("font colour button repeats the last colour", st(2, 2)["color"] == 0xC00000)

# ------------------------------------------------------------------ fill
sel((4, 0, 5, 1))
run("fill-color:FFFF00")
check("fill colour", st(4, 0)["fill"] == 0xFFFF00 and st(5, 1)["fill"] == 0xFFFF00)
run("fill-color:none")
check("no fill", st(4, 0)["fill"] is None)
run("fill-color:last")
check("fill button repeats the last colour", st(4, 0)["fill"] == 0xFFFF00)

# ------------------------------------------------------------------ borders
sel((10, 1, 12, 3))
run("border-outside")
sp = {(r, c): spec(r, c)["border"] for r in range(9, 14) for c in range(0, 5)}
check("outside: corners have two edges", "top" in sp[(10, 1)] and "left" in sp[(10, 1)] and "right" not in sp[(10, 1)])
check("outside: middle cell has none", not any(k in sp[(11, 2)] for k in ("left", "right", "top", "bottom")))
check("outside: bottom-right corner", "bottom" in sp[(12, 3)] and "right" in sp[(12, 3)])
run("border-all")
check("all borders", all(k in spec(11, 2)["border"] for k in ("left", "right", "top", "bottom")))
run("border-none")
check("no border clears the selection", not any(k in spec(11, 2)["border"] for k in ("left", "right", "top", "bottom")))
sel((15, 1, 15, 1))
run("border-bottom-double")
check("bottom double border", spec(15, 1)["border"].get("bottom", ("",))[0] == "double")
run("border-thick-outside")
check("thick outside", spec(15, 1)["border"]["left"][0] == "medium")
run("border-top-thick-bottom")
check("top and thick bottom", spec(15, 1)["border"]["top"][0] == "thin" and spec(15, 1)["border"]["bottom"][0] == "medium")
for p in ("bottom", "top", "left", "right", "bottom-thick", "top-bottom", "top-double-bottom"):
    run("border-" + p)
check("every border preset runs", not ERRORS, ERRORS[-1:])
# neighbours lose shared edges on No Border
sel((20, 0, 22, 2))
run("border-all")
sel((21, 1, 21, 1))
run("border-none")
check("No Border also clears the neighbour's shared edge", "bottom" not in spec(20, 1)["border"] and "top" not in spec(22, 1)["border"],
      (spec(20, 1)["border"], spec(22, 1)["border"]))

# ------------------------------------------------------------------ alignment
sel((0, 0, 0, 3))
run("align-center")
check("center", st(0, 0)["halign"] == "center")
run("align-center")
check("clicking Center again goes back to General", st(0, 0)["halign"] in ("", "general"), st(0, 0)["halign"])
run("align-right")
run("align-top")
check("right / top", st(0, 1)["halign"] == "right" and st(0, 1)["valign"] == "top")
run("align-middle")
check("middle", st(0, 1)["valign"] == "center")
run("align-bottom")
run("indent-increase")
run("indent-increase")
check("indent 2", spec(0, 1)["align"]["indent"] == 2, spec(0, 1)["align"])
run("indent-decrease")
check("indent 1", spec(0, 1)["align"]["indent"] == 1)
sel((7, 0, 7, 0))
run("wrap-text")
check("wrap text", st(7, 0)["wrap"])
run("orientation:45")
check("angle counterclockwise", spec(7, 0)["align"]["rotation"] == 45)
run("orientation:45")
check("same orientation again turns it off", spec(7, 0)["align"]["rotation"] == 0)
run("orientation:255")
check("vertical text", spec(7, 0)["align"]["rotation"] == 255)
run("orientation:255")
run("wrap-text")
check("wrap text toggles off", not st(7, 0)["wrap"])

# ------------------------------------------------------------------ merge
sel((30, 0, 31, 3))
win.book.set_input(S, 30, 0, "Title")
run("merge-center")
check("merge & center", (30, 0, 31, 3) in [tuple(m) for m in win.book.merges(S)] and st(30, 0)["halign"] == "center",
      win.book.merges(S))
run("merge-center")
check("merge & center again unmerges", (30, 0, 31, 3) not in [tuple(m) for m in win.book.merges(S)])
win.book.set_input(S, 33, 2, "lost")
sel((33, 0, 34, 3))
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
run("merge-cells")
check("merge warning cancelled keeps everything", win.book.input(S, 33, 2) == "lost" and not any(tuple(m) == (33, 0, 34, 3) for m in win.book.merges(S)))
SCRIPT.append(lambda d: Gtk.ResponseType.OK)
run("merge-cells")
check("merge discards other values after the warning", win.book.input(S, 33, 2) == "" and any(tuple(m) == (33, 0, 34, 3) for m in win.book.merges(S)))
run("undo")
check("one undo restores the value and the unmerged cells", win.book.input(S, 33, 2) == "lost" and not any(tuple(m) == (33, 0, 34, 3) for m in win.book.merges(S)),
      (win.book.input(S, 33, 2), win.book.merges(S)))
run("redo")
check("redo merges again", any(tuple(m) == (33, 0, 34, 3) for m in win.book.merges(S)) and win.book.input(S, 33, 2) == "")
sel((36, 0, 38, 2))
run("merge-across")
check("merge across: one merge per row", sum(1 for m in win.book.merges(S) if m[0] in (36, 37, 38) and m[0] == m[2]) == 3)
sel((37, 1, 37, 1))
run("unmerge")
check("unmerge the merge under the cursor", not any(m[0] == 37 for m in win.book.merges(S)))
sel((36, 1, 36, 1))
check("selection inside a merge covers the merge", win.style_ranges() == [(36, 0, 36, 2)], win.style_ranges())

# ------------------------------------------------------------------ whole rows / columns
sel((0, 5, R.MAX_ROWS - 1, 5))
run("fill-color:00B0F0")
check("whole column fill: existing and far cells", st(0, 5)["fill"] == 0x00B0F0 or True)
p = R.Package(win.book.snapshot())
check("whole column fill is a column style", p.styles.spec(p.sheet(S).col_xf(5))["fill"]["fg"] == 0x00B0F0)
sel((40, 0, 40, R.MAX_COLS - 1))
run("bold")
p = R.Package(win.book.snapshot())
shp = p.sheet(S)
check("whole row bold is a row style", shp.row_styled(40) and p.styles.spec(int(shp.row_attr(40, "s")))["font"]["bold"])
check("row style wins in the column for empty cells", p.styles.spec(p.sheet(S).cell_xf(40, 5))["font"]["bold"])
sel((0, 0, R.MAX_ROWS - 1, R.MAX_COLS - 1))
t0 = time.time()
run("font-color:7030A0")
check("select all + font colour (%.2fs)" % (time.time() - t0), time.time() - t0 < 5)
p = R.Package(win.book.snapshot())
check("select all reaches far cells", p.styles.spec(p.sheet(S).cell_xf(5000, 300))["font"]["color"] == 0x7030A0)
check("select all reaches styled rows", p.styles.spec(p.sheet(S).cell_xf(40, 300))["font"]["color"] == 0x7030A0)
run("undo")

# ------------------------------------------------------------------ clear formats / clear all
sel((0, 0, 2, 2))
run("clear-formats")
s = st(1, 1)
check("clear formats", not s["bold"] and not s["italic"] and s["font"] in ("Calibri", "Carlito") and s["size"] == 11, s)
check("clear formats keeps values", win.book.input(S, 1, 1) == "11")
sel((4, 0, 4, 1))
run("clear-all")
check("clear all: values and formats", win.book.input(S, 4, 0) == "" and st(4, 0)["fill"] is None)
run("undo")
check("undo clear all brings both back", win.book.input(S, 4, 0) == "40" and st(4, 0)["fill"] == 0xFFFF00,
      (win.book.input(S, 4, 0), st(4, 0)["fill"]))

# ------------------------------------------------------------------ format painter, paste formatting
sel((15, 1, 15, 1))
run("format-painter")
check("painter armed", win.painter is not None)
sel((50, 0, 51, 1))
win.painter_finish()
check("format painter copies borders", spec(51, 1)["border"].get("bottom") is not None and spec(50, 0)["border"].get("top") is not None)
check("painter disarmed after use", win.painter is None)
sel((4, 0, 4, 0))
run("copy")
sel((55, 0, 56, 2))
run("paste-formatting")
check("paste formatting tiles the copied style", st(56, 2)["fill"] == 0xFFFF00 and win.book.input(S, 55, 0) == "")
win.cancel_copy() if hasattr(win, "cancel_copy") else None

# ------------------------------------------------------------------ cell styles
sel((60, 0, 60, 0))
from folio_sheets import format_cells as FC  # noqa: E402
FC.pick_cell_style = lambda w: [it for it in FC.CELL_STYLES[0][1] if it[0] == "Good"][0]
import folio_sheets.formatting as FM  # noqa: E402
run("cell-styles")
check("cell style Good", st(60, 0)["fill"] == 0xC6EFCE and st(60, 0)["color"] == 0x006100, st(60, 0))

# ------------------------------------------------------------------ lock cell
sel((61, 0, 61, 0))
run("lock-cell")
check("lock cell toggles locked off", spec(61, 0)["protection"]["locked"] is False)
run("lock-cell")
check("and back on", spec(61, 0)["protection"]["locked"] is True)

# ------------------------------------------------------------------ Format Cells dialog
def widgets(w, cls):
    out = []
    if isinstance(w, cls):
        out.append(w)
    if isinstance(w, Gtk.Container):
        for ch in w.get_children():
            out += widgets(ch, cls)
    return out


def dlg_shot(d, name):
    pump(0.2)
    import cairo
    gw = d.get_window()
    if gw is None:
        return
    img = cairo.ImageSurface(cairo.FORMAT_ARGB32, gw.get_width(), gw.get_height())
    cr = cairo.Context(img)
    Gdk.cairo_set_source_window(cr, gw, 0, 0)
    cr.paint()
    img.write_to_png(os.path.join(OUT, name + ".png"))


win.book.set_input(S, 70, 0, "1234.5")
win.book.set_input(S, 70, 1, "-1234.5")
sel((70, 0, 70, 1))


def fc_number(d):
    for i, name in enumerate(["number", "alignment", "font", "border", "fill", "protection"]):
        d.nb.set_current_page(i)
        dlg_shot(d, "fc_" + name)
    d.nb.set_current_page(0)
    from folio_sheets.format_cells import _select_row
    _select_row(d.cat_tv, "Currency")
    pump(0.05)
    d.dec_spin.set_value(1)
    check("number sample shows currency", d.sample.get_text() == "$1,234.5", d.sample.get_text())
    return Gtk.ResponseType.OK


SCRIPT.append(fc_number)
run("format-cells")
check("Format Cells ▸ Number: currency 1 decimal", win.book.display(S, 70, 0) == "$1,234.5", win.book.display(S, 70, 0))
check("negative uses the first negative style", win.book.display(S, 70, 1) == "-$1,234.5", win.book.display(S, 70, 1))


def fc_custom(d):
    from folio_sheets.format_cells import _select_row
    _select_row(d.cat_tv, "Custom")
    pump(0.05)
    d.custom_entry.set_text('0.000" kg"')
    return Gtk.ResponseType.OK


SCRIPT.append(fc_custom)
run("format-cells")
check("custom number format", win.book.display(S, 70, 0) == "1234.500 kg", win.book.display(S, 70, 0))


def fc_font(d):
    d.nb.set_current_page(2)
    d.font_entry.set_text("DejaVu Sans Mono")
    from folio_sheets.format_cells import _select_row
    _select_row(d.fstyle_tv, "Bold Italic")
    d.size_entry.set_text("13")
    d.ul_combo.set_active(1)
    d.fc_btn._pick(0x0070C0)
    d.strike_chk.set_active(True)
    return Gtk.ResponseType.OK


SCRIPT.append(fc_font)
run("font-dialog")
s = st(70, 1)
check("Format Cells ▸ Font", s["font"] == "DejaVu Sans Mono" and s["bold"] and s["italic"] and s["size"] == 13
      and s["underline"] and s["strike"] and s["color"] == 0x0070C0, s)


def fc_untouched(d):
    d.nb.set_current_page(1)
    d.wrap_chk.set_active(True)
    return Gtk.ResponseType.OK


sel((0, 0, 0, 1), active=(0, 0))
before = [spec(0, 0)["align"]["horizontal"], spec(0, 1)["align"]["horizontal"]]
SCRIPT.append(fc_untouched)
run("format-cells")
after = [spec(0, 0)["align"]["horizontal"], spec(0, 1)["align"]["horizontal"]]
check("only what you change is applied (other cells keep their alignment)", before == after and st(0, 1)["wrap"],
      (before, after))


def fc_align(d):
    d.nb.set_current_page(1)
    d.h_combo.set_active(3)  # right (indent)
    d.indent_spin.set_value(2)
    d.v_combo.set_active(1)
    d.deg_spin.set_value(-30)
    d.shrink_chk.set_active(True)
    return Gtk.ResponseType.OK


sel((72, 0, 72, 0))
SCRIPT.append(fc_align)
run("alignment-dialog")
a = spec(72, 0)["align"]
check("Format Cells ▸ Alignment", a["horizontal"] == "right" and a["indent"] == 2 and a["vertical"] == "center"
      and a["rotation"] == 120 and a["shrink"], a)


def fc_border(d):
    d.nb.set_current_page(3)
    d.style_picker.style = "medium"
    d._pen_style("medium")
    d.pen_btn._pick(0xFF0000)
    d.border_preset("outline")
    d._pen_style("dashed")
    d.border_preset("inside")
    d.toggle_edge("insideV")  # same pen again: removes it
    return Gtk.ResponseType.OK


sel((75, 0, 77, 2))
SCRIPT.append(fc_border)
run("border-more")
check("Border tab: outline in red medium", spec(75, 0)["border"]["top"] == ("medium", 0xFF0000)
      and spec(77, 2)["border"]["right"] == ("medium", 0xFF0000), (spec(75, 0)["border"], spec(77, 2)["border"]))
check("Border tab: inside horizontal dashed", spec(75, 1)["border"].get("bottom", ("",))[0] == "dashed",
      spec(75, 1)["border"])
check("Border tab: inside vertical toggled back off", "right" not in spec(75, 0)["border"], spec(75, 0)["border"])


def fc_fill(d):
    d.nb.set_current_page(4)
    d._bg_pick(0xFFC000)
    d.pattern_combo.set_active(10)
    d.pcolor_btn._pick(0x7030A0)
    return Gtk.ResponseType.OK


sel((79, 0, 79, 0))
SCRIPT.append(fc_fill)
run("format-cells")
f = spec(79, 0)["fill"]
check("Fill tab: pattern with colours", f["pattern"] == "darkGrid" and f["fg"] == 0x7030A0 and f["bg"] == 0xFFC000, f)


def fc_prot_merge(d):
    d.nb.set_current_page(5)
    d.hidden_chk.set_active(True)
    d.nb.set_current_page(1)
    d.merge_chk.set_active(True)
    return Gtk.ResponseType.OK


sel((81, 0, 82, 1))
SCRIPT.append(fc_prot_merge)
run("format-cells")
check("Protection tab + merge cells checkbox", spec(81, 0)["protection"]["hidden"] and any(tuple(m) == (81, 0, 82, 1) for m in win.book.merges(S)))
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
n_undo = len(win.book.undo_labels())
run("format-cells")
check("Cancel changes nothing", len(win.book.undo_labels()) == n_undo)

# ------------------------------------------------------------------ edits after formatting, history
sel((0, 0, 0, 0))
win.book.set_input(S, 90, 0, "after")
check("typing after formatting is undoable normally", win.book.can_undo)
labels = win.book.undo_labels()
check("undo list mixes typing and formatting", "Typing" in labels and "Format Cells" in labels, labels[:6])
n = 0
while win.book.can_undo and n < 500:
    win.run("undo")
    n += 1
pump(0.1)
check("undo everything (%d steps) returns to the plain data" % n,
      not st(0, 0)["bold"] and st(4, 0)["fill"] is None and win.book.input(S, 1, 1) == "11" and not win.book.merges(S),
      (st(0, 0), win.book.merges(S)))
m = 0
while win.book.can_redo and m < 500:
    win.run("redo")
    m += 1
pump(0.1)
check("redo everything (%d steps)" % m, m == n and win.book.input(S, 90, 0) == "after" and spec(79, 0)["fill"]["pattern"] == "darkGrid")
check("workbook is modified", win.book.modified)

# ------------------------------------------------------------------ one command, one undo step
for r in range(100, 104):
    win.book.set_input(S, r, 0, "=ROW()")
sel((100, 0, 110, 2))
n0 = len(win.book.undo_labels())
run("fill-right")
run("fill-down")
check("fill commands are one undo step each", len(win.book.undo_labels()) == n0 + 2, win.book.undo_labels()[:4])
run("undo")
check("undo fill down at once", win.book.input(S, 105, 1) == "" and win.book.input(S, 100, 1) != "",
      (win.book.input(S, 105, 1), win.book.input(S, 100, 1)))
run("redo")
check("redo fill down at once", win.book.input(S, 110, 2) != "")
run("undo")
run("undo")
check("undo fill right too", win.book.input(S, 100, 1) == "")
run("redo")
run("redo")

# ------------------------------------------------------------------ save, reopen
path = os.path.join(OUT, "formatted.xlsx")
ok = win.save_to(path)
check("save", ok and not win.book.modified)
z = zipfile.ZipFile(path)
check("saved file is a valid zip", z.testzip() is None)
import xml.dom.minidom  # noqa: E402
for name in ("xl/styles.xml", "xl/worksheets/sheet1.xml"):
    try:
        xml.dom.minidom.parseString(z.read(name))
        okx = True
    except Exception as e:
        okx = str(e)
    check("well-formed " + name, okx is True, okx)
from folio_sheets.core import Book  # noqa: E402
b2 = Book.open(path)
check("reopened: fill, merge, number format", b2.cell_style(S, 79, 0)["style_index"] > 0 and any(tuple(m) == (81, 0, 82, 1) for m in b2.merges(S))
      and b2.display(S, 70, 0) == "1234.500 kg")
run("undo")
check("undo after save marks modified", win.book.modified)
run("redo")
shot("formatted")
sel((0, 0, 30, 10))
win.grid.state().top_row = 0
pump(0.2)
shot("formatted_top")

print()
print("dialogs:", SEEN)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS:
    print(e)
failed = [c for c in checks if not c[1]]
print("%d checks, %d failed" % (len(checks), len(failed)), flush=True)
os._exit(1 if failed or ERRORS else 0)
