# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): hyperlinks through the window — Insert and
# Edit Hyperlink (Ctrl+K), following with a click, =HYPERLINK(), Clear and Remove
# Hyperlinks, paste, the pointer, and saving.
import os
import sys
import tempfile
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


SCRIPT = []
SEEN = []


def fake_run(d):
    pump(0.03)
    text = d.get_property("text") if isinstance(d, Gtk.MessageDialog) else None
    SEEN.append((d.get_title(), text))
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

from folio_sheets.links_ui import LINK_COLOR, HyperlinkDialog  # noqa: E402


class Ev:
    def __init__(self, x, y, button=1, kind=Gdk.EventType.BUTTON_PRESS, state=0):
        self.x, self.y, self.button, self.type, self.state = x, y, button, kind, state

    def get_state(self):
        return self.state


win.resize(1280, 800)
pump(0.5)
g = win.grid
b = win.book
S = 0
L = win.links
OPENED = []
L.opener = OPENED.append


def sel(r, c, r2=None, c2=None):
    g.select_range((r, c, r if r2 is None else r2, c if c2 is None else c2), (r, c))
    pump(0.02)


def dialog(kind=None, **kw):
    def fn(d):
        check("the %s dialog" % d.get_title(), isinstance(d, HyperlinkDialog))
        if "look" in kw:
            kw["look"](d)
        if kind:
            d.kind_buttons[kind].set_active(True)
        for k in ("address", "email", "subject", "cellref", "text"):
            if k in kw:
                getattr(d, k).set_text(kw[k])
        if "tooltip" in kw:
            d.tooltip = kw["tooltip"]
        if "place" in kw:
            it = d.places.get_iter_first()
            found = None
            while it is not None and found is None:
                ch = d.places.iter_children(it)
                while ch is not None:
                    if d.places[ch][0] == kw["place"]:
                        found = ch
                        break
                    ch = d.places.iter_next(ch)
                it = d.places.iter_next(it)
            d.place_view.get_selection().select_iter(found)
        return kw.get("resp", Gtk.ResponseType.OK)
    return fn


def click(r, c, hold=0.0):
    x, y, w, h = b.cell_rect(S, r, c, g.params())
    g.on_press(g.area, Ev(x + 5, y + h / 2))
    if hold and getattr(g, "_link_press", None):
        lp = g._link_press
        g._link_press = (lp[0], lp[1], lp[2] - hold)
    g.on_release(g.area, Ev(x + 5, y + h / 2))
    pump(0.1)


win.run("insert-sheet")
pump(0.1)
names = [b.sheet_info(i)["name"] for i in range(b.sheet_count)]
other = [n for n in names if n != b.sheet_info(0)["name"]][0]
g.set_sheet(0)
win.sheet_changed()
pump(0.05)

# ---- a web page on an empty cell
sel(0, 0)
SCRIPT.append(dialog("web", address="www.example.org", tooltip="Our site"))
win.run("link")
lk = b.link_at(S, 0, 0)
check("Insert Hyperlink puts a link on the cell", lk is not None and lk["target"] == "https://www.example.org", lk)
check("an address without a scheme becomes a web address", lk and lk["target"].startswith("https://"))
check("the address as typed is shown in the empty cell", b.input(S, 0, 0) == "www.example.org", b.input(S, 0, 0))
st = b.cell_style(S, 0, 0)
check("and looks like a link", st["color"] == LINK_COLOR and st["underline"], (st["color"], st["underline"]))
check("the ScreenTip is kept", lk and lk["tooltip"] == "Our site")
win.run("undo")
check("one undo takes all of it back", b.link_at(S, 0, 0) is None and b.input(S, 0, 0) == "" and
      not b.cell_style(S, 0, 0)["underline"])
win.run("redo")
check("redo puts it back", b.link_at(S, 0, 0) is not None and b.input(S, 0, 0) == "www.example.org")

# ---- the pointer and following
check("the tip says where it goes and how to select", (L.tip_for(0, 0) or "").startswith("Our site - Click once"))
x, y, w, h = b.cell_rect(S, 0, 0, g.params())


class Motion:
    def __init__(self, x, y):
        self.x, self.y, self.state = x, y, 0

    def get_state(self):
        return 0


g.on_motion(g.area, Motion(x + 5, y + h / 2))
check("over a link the area shows the tip", g.area.get_tooltip_text() == L.tip_for(0, 0))
g.on_motion(g.area, Motion(x + 5, y + h * 3.5))
check("off it the tip goes", g.area.get_tooltip_text() is None)
sel(5, 5)
click(0, 0)
check("a click follows the link", OPENED[-1:] == ["https://www.example.org"], OPENED)
check("and selects the cell", tuple(g.sel.active) == (0, 0))
n = len(OPENED)
sel(5, 5)
click(0, 0, hold=1.0)
check("click and hold selects without following", len(OPENED) == n and tuple(g.sel.active) == (0, 0))
sel(5, 5)
x, y, w, h = b.cell_rect(S, 0, 0, g.params())
g.on_press(g.area, Ev(x + 5, y + h / 2))
g._drag_to(x + 5, y + h * 3.5)
g.on_release(g.area, Ev(x + 5, y + h * 3.5))
pump(0.1)
check("dragging from a link selects cells, it doesn't follow", len(OPENED) == n and not g.sel.is_single_cell())

# ---- edit it; the text stays unless it is retyped
b.set_input(S, 1, 0, "Report")
sel(1, 0)
SCRIPT.append(dialog("web", address="docs/report.xlsx"))
win.run("link")
lk = b.link_at(S, 1, 0)
check("a file address is kept as typed", lk and lk["target"] == "docs/report.xlsx", lk)
check("text already in the cell stays", b.input(S, 1, 0) == "Report")
seen = {}
SCRIPT.append(dialog(look=lambda d: seen.update(title=d.get_title(), addr=d.address.get_text(),
                                                kind=d.kind, text=d.text.get_text()),
                     address="https://example.org/r"))
win.run("link")
check("on a linked cell it is Edit Hyperlink, filled in",
      seen == {"title": "Edit Hyperlink", "addr": "docs/report.xlsx", "kind": "web", "text": "Report"}, seen)
check("the edit changes the address", b.link_at(S, 1, 0)["target"] == "https://example.org/r")
check("one link on the cell, not two", b.link_count(S) == 2, b.link_count(S))
check("the right-click menu offers Edit, Open and Remove",
      [x[1] for x in L.menu_items()] == ["link", "open-hyperlink", "remove-hyperlinks"])
win.run("open-hyperlink")
check("Open Hyperlink follows it", OPENED[-1] == "https://example.org/r")

# ---- a number keeps its value
b.set_input(S, 2, 0, "42")
sel(2, 0)
SCRIPT.append(dialog("web", address="https://example.org/n", look=lambda d: seen.update(ed=d.text.get_sensitive())))
win.run("link")
check("a number's text can't be retyped", seen.get("ed") is False)
check("and the number stays a number", b.input(S, 2, 0) == "42" and b.link_at(S, 2, 0) is not None)

# ---- a place in the workbook, and e-mail
sel(3, 0)
SCRIPT.append(dialog("place", place=other, cellref="B3"))
win.run("link")
lk = b.link_at(S, 3, 0)
check("a place in this document", lk and lk["location"] == "%s!B3" % other and not lk["target"], lk)
check("is shown as its place", b.input(S, 3, 0) == "%s!B3" % other)
click(3, 0)
check("following it goes to that sheet and cell", g.sheet == 1 and tuple(g.sel.active) == (2, 1),
      (g.sheet, g.sel.active))
g.set_sheet(0)
win.sheet_changed()
pump(0.05)
sel(4, 0)
SCRIPT.append(dialog("place", cellref="Z99Z"))
SCRIPT.append(lambda d: Gtk.ResponseType.OK)          # the message
SCRIPT.append(dialog("place", cellref="C5"))
win.run("link")
check("a bad reference is reported and the dialog stays", "not valid" in (SEEN[-2][1] or "") and
      b.link_at(S, 4, 0)["location"].endswith("!C5"), SEEN[-3:])
sel(5, 0)
SCRIPT.append(dialog("email", email="ann@example.org", subject="Hi there"))
win.run("link")
lk = b.link_at(S, 5, 0)
check("an e-mail link", lk and lk["target"] == "mailto:ann@example.org?subject=Hi%20there", lk)
check("shows its address", b.input(S, 5, 0) == "mailto:ann@example.org")
SCRIPT.append(dialog(look=lambda d: seen.update(mk=d.kind, em=d.email.get_text(), sj=d.subject.get_text()),
                     resp=Gtk.ResponseType.CANCEL))
win.run("link")
check("and opens again as e-mail", (seen.get("mk"), seen.get("em"), seen.get("sj")) ==
      ("email", "ann@example.org", "Hi there"), seen)
SCRIPT.append(dialog("web", address=""))
SCRIPT.append(lambda d: Gtk.ResponseType.OK)
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
sel(6, 0)
win.run("link")
check("an empty address is refused", b.link_at(S, 6, 0) is None and "address" in (SEEN[-2][1] or ""), SEEN[-3:])

# ---- =HYPERLINK()
b.set_input(S, 7, 0, '=HYPERLINK("https://example.org/f","go")')
b.set_input(S, 8, 0, '=HYPERLINK("#C20","here")')
sel(10, 10)
click(7, 0)
check("clicking =HYPERLINK() opens its address", OPENED[-1] == "https://example.org/f")
click(8, 0)
check("a # address goes to the place", tuple(g.sel.active) == (19, 2), g.sel.active)

# ---- paste, clear, remove
sel(1, 0)
win.run("copy")
sel(0, 3)
win.run("paste")
check("paste brings the link along", (b.link_at(S, 0, 3) or {}).get("target") == "https://example.org/r")
sel(0, 3)
win.run("clear-hyperlinks")
check("Clear Hyperlinks takes the link off", b.link_at(S, 0, 3) is None)
check("and keeps the look", b.cell_style(S, 0, 3)["underline"])
sel(2, 0)
win.run("remove-hyperlinks")
st = b.cell_style(S, 2, 0)
check("Remove Hyperlinks takes the look too", b.link_at(S, 2, 0) is None and not st["underline"] and
      st["color"] != LINK_COLOR, st)
sel(5, 0)
win.run("clear-all")
check("Clear All takes the link with everything else", b.link_at(S, 5, 0) is None and b.input(S, 5, 0) == "")
win.run("undo")
check("and undo brings it all back", b.link_at(S, 5, 0) is not None and b.input(S, 5, 0) != "")
sel(1, 0)
SCRIPT.append(lambda d: 2)                              # Remove Link
win.run("link")
check("Remove Link in the dialog", b.link_at(S, 1, 0) is None and b.input(S, 1, 0) == "Report")

# ---- rows
g.select_range((0, 0, 0, 16383), (0, 0))
win.run("insert-rows")
check("inserting rows moves links", b.link_at(S, 1, 0) is not None and b.link_at(S, 0, 0) is None)
win.run("undo")

# ---- saved and opened again
path = os.path.join(tempfile.mkdtemp(), "links.xlsx")
b.save(path)
from folio_sheets.core import Book  # noqa: E402
b2 = Book.open(path)
check("links survive saving", (b2.link_at(S, 0, 0) or {}).get("target") == "https://www.example.org" and
      (b2.link_at(S, 3, 0) or {}).get("location") == "%s!B3" % other and
      (b2.link_at(S, 0, 0) or {}).get("tooltip") == "Our site")

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
