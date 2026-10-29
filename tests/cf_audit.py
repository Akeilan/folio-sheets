# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): conditional formatting from the menu, the
# rule dialogs, Manage Rules, Clear Rules, copy/paste and the file.
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
vals = [3, 8, 1, 9, 5, 7, 2, 6, 4, 10]
for i, v in enumerate(vals):
    for c in range(6):
        b.set_input(S, i, c, str(v))
words = ["apple", "banana", "apple", "cherry", "date", "fig", "grape", "kiwi", "apple", "lime"]
for i, w in enumerate(words):
    b.set_input(S, i, 6, w)


def sel(r1, c1, r2, c2):
    g.sel.ranges = [(r1, c1, r2, c2)]
    g.sel.active = (r1, c1)


# the menu builds (with its galleries)
from folio_sheets import cf_ui
items = cf_ui.menu(win)
cmds = [it[1] for it in items if isinstance(it, tuple) and it[0] != "#"]
check("the menu has the rule commands", "conditional-formatting:gt" in cmds and "conditional-formatting:manage" in cmds)
gal = [w for it in items if isinstance(it, Gtk.FlowBox) for w in it.get_children()]
check("the menu has galleries of bars, scales and icon sets", len(gal) >= 12 + 12 + 17, len(gal))


# Greater Than... 5 with the default preset
def gt(d):
    find(d, "value").set_text("5")
    return Gtk.ResponseType.OK


sel(0, 0, 9, 0)
SCRIPT.append(gt)
win.run("conditional-formatting:gt")
check("Greater Than colours the cells above 5", b.cf_at(S, 1, 0)["fill"] == 0xFFC7CE and b.cf_at(S, 0, 0) is None,
      (b.cf_at(S, 1, 0), b.cf_at(S, 0, 0)))
check("as one undo step", b.undo_labels()[0] == "Conditional Formatting", b.undo_labels()[:2])


# Between 3 and 6 with Green fill
def between(d):
    find(d, "value1").set_text("3")
    find(d, "value2").set_text("6")
    find(d, "format").set_active(2)
    return Gtk.ResponseType.OK


sel(0, 1, 9, 1)
SCRIPT.append(between)
win.run("conditional-formatting:between")
check("Between 3 and 6", b.cf_at(S, 0, 1)["fill"] == 0xC6EFCE and b.cf_at(S, 2, 1) is None)


# Top 3
def top3(d):
    find(d, "rank").set_value(3)
    return Gtk.ResponseType.OK


sel(0, 2, 9, 2)
SCRIPT.append(top3)
win.run("conditional-formatting:top10")
check("Top 3 Items", [r for r in range(10) if b.cf_at(S, r, 2)] == [1, 3, 9])

# galleries
sel(0, 3, 9, 3)
win.run("conditional-formatting:bar:g:638EC6")
check("a gradient data bar", b.cf_at(S, 9, 3)["bar"][1] > b.cf_at(S, 2, 3)["bar"][1])
sel(0, 4, 9, 4)
win.run("conditional-formatting:scale:0")
check("Green - Yellow - Red scale: highest green", b.cf_at(S, 9, 4)["fill"] == 0x63BE7B and b.cf_at(S, 2, 4)["fill"] == 0xF8696B,
      (b.cf_at(S, 9, 4), b.cf_at(S, 2, 4)))
sel(0, 5, 9, 5)
win.run("conditional-formatting:icons:3Arrows")
check("3 Arrows icon set", b.cf_at(S, 9, 5)["icon"] == ("3Arrows", 2) and b.cf_at(S, 2, 5)["icon"] == ("3Arrows", 0))


# Duplicate Values
def dup(d):
    return Gtk.ResponseType.OK


sel(0, 6, 9, 6)
SCRIPT.append(dup)
win.run("conditional-formatting:dup")
check("Duplicate Values", b.cf_at(S, 0, 6) is not None and b.cf_at(S, 3, 6) is None)


# New Rule: a formula
def formula_rule(d):
    types = find(d, "types")
    types.select_row(types.get_row_at_index(5))
    pump(0.02)
    find(d, "formula").set_text("=MOD($A1,2)=0")
    return Gtk.ResponseType.OK


def fmt_bold(d):
    find(d, "bold").set_active(True)
    find(d, "fill-on").set_active(True)
    return Gtk.ResponseType.OK


sel(0, 7, 9, 7)
for i, v in enumerate(vals):
    b.set_input(S, i, 7, "x")


def new_with_format(d):
    types = find(d, "types")
    types.select_row(types.get_row_at_index(5))
    pump(0.02)
    find(d, "formula").set_text("=MOD($A1,2)=0")
    SCRIPT.insert(0, fmt_bold)
    find(d, "format-button").clicked()
    return Gtk.ResponseType.OK


SCRIPT.append(new_with_format)
win.run("conditional-formatting:new")
check("New Rule with a formula (relative rows)", b.cf_at(S, 1, 7) and b.cf_at(S, 1, 7)["bold"] and b.cf_at(S, 0, 7) is None,
      (b.cf_at(S, 1, 7), b.cf_at(S, 0, 7)))

# Manage Rules: this sheet lists everything; delete the top rule (the formula), cancel undoes
rules = b.cf_rules(S)
check("Manage Rules lists the rules by priority", rules[0]["desc"] == "Formula: =MOD($A1,2)=0" and len(rules) == 8,
      [r["desc"] for r in rules])


def manage_delete(d):
    find(d, "scope").set_active_id("sheet")
    pump(0.02)
    view = find(d, "rules")
    view.get_selection().select_path(Gtk.TreePath(0))
    find(d, "delete").clicked()
    pump(0.02)
    return Gtk.ResponseType.CANCEL


SCRIPT.append(manage_delete)
win.run("conditional-formatting:manage")
check("Cancel in Manage Rules keeps the rules", len(b.cf_rules(S)) == 8)


def manage_down(d):
    find(d, "scope").set_active_id("sheet")
    pump(0.02)
    view = find(d, "rules")
    view.get_selection().select_path(Gtk.TreePath(0))
    find(d, "down").clicked()
    pump(0.02)
    return Gtk.ResponseType.OK


SCRIPT.append(manage_down)
win.run("conditional-formatting:manage")
check("Move Down changes the order", b.cf_rules(S)[1]["desc"] == "Formula: =MOD($A1,2)=0", [r["desc"] for r in b.cf_rules(S)][:3])

# copy and paste carries the rules
sel(0, 0, 9, 0)
win.run("copy")
sel(20, 0, 20, 0)
win.run("paste")
check("paste brings the rule along", b.cf_at(S, 21, 0) is not None and b.cf_at(S, 20, 0) is None, (b.cf_at(S, 21, 0), b.cf_at(S, 20, 0)))

# rows inserted move them
b.insert_rows(S, 0, 1)
check("inserted rows move the rules", b.cf_at(S, 2, 0) is not None and b.cf_at(S, 1, 0) is None)
win.run("undo")

# clear from the selection, then the sheet
sel(0, 1, 9, 1)
win.run("conditional-formatting:clear-sel")
check("Clear Rules from Selected Cells", b.cf_at(S, 0, 1) is None and b.cf_at(S, 1, 0) is not None)
win.run("undo")
check("undo brings them back", b.cf_at(S, 0, 1) is not None)

# saved and opened again
path = "/tmp/cf_audit.xlsx"
win.save_to(path)
from folio_sheets import core
b2 = core.Book.open(path)
check("the rules are saved", len(b2.cf_rules(0)) == len(b.cf_rules(S)) and b2.cf_at(0, 1, 0)["fill"] == 0xFFC7CE,
      (len(b2.cf_rules(0)), len(b.cf_rules(S))))
win.run("conditional-formatting:clear-sheet")
check("Clear Rules from Entire Sheet", b.cf_rules(S) == [])
win.run("undo")
pump(0.1)
shot = os.environ.get("FOLIO_SHOT", "/tmp/cf_audit.png")
b.render_png(S, shot, g.params({"width": 900.0, "height": 500.0}))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
