# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): Protect Sheet and Protect Workbook — the
# dialogs and passwords, typing and formatting on locked and unlocked cells, hidden
# formulas, the permissions, saving, and a walk over every command that must leave a
# protected sheet's locked cells as they were.
import inspect
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


SCRIPT = []
SEEN = []
WALK = [None]


def fake_run(d):
    pump(0.01)
    text = d.get_property("text") if isinstance(d, Gtk.MessageDialog) else None
    SEEN.append((d.get_title(), text))
    if WALK[0] is not None:
        n = getattr(d, "_walk_runs", 0)
        d._walk_runs = n + 1
        return WALK[0] if n == 0 else Gtk.ResponseType.CANCEL
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run
Gtk.NativeDialog.run = lambda d: Gtk.ResponseType.CANCEL

from folio_sheets import commands, protect_ui  # noqa: E402
from folio_sheets.protect_ui import REFUSED, ProtectSheetDialog  # noqa: E402

g = win.grid
b = win.book
S = 0
for r in range(8):
    for c in range(5):
        b.set_input(S, r, c, str(r * 10 + c))
b.set_input(S, 0, 4, "=A1*2")


def sel(r, c, r2=None, c2=None):
    g.select_range((r, c, r if r2 is None else r2, c if c2 is None else c2), (r, c))
    pump(0.01)


def password(pw):
    def fn(d):
        d.entry.set_text(pw)
        return Gtk.ResponseType.OK
    return fn


# unlock B2:B3, hide the formula in E1
sel(1, 1, 2, 1)
win.run("lock-cell")
check("Lock Cell unlocks B2:B3", not b.locked_in(S, [(1, 1, 2, 1)]) and b.locked_in(S, [(0, 0, 0, 0)]))
sel(0, 4)

win.restyle("Hidden", [{"protection": {"hidden": True}}])
check("E1's formula is set to Hidden", b.style_spec(S, 0, 4)["protection"]["hidden"])

# ---- Protect Sheet with a password


def protect_dialog(pw="", allow=()):
    def fn(d):
        check("the Protect Sheet dialog", isinstance(d, ProtectSheetDialog))
        d.password.set_text(pw)
        for k in allow:
            d.perms[k].set_active(True)
        return Gtk.ResponseType.OK
    return fn


SCRIPT.append(protect_dialog("s3cret"))
SCRIPT.append(password("wrong"))
SCRIPT.append(lambda d: Gtk.ResponseType.OK)          # not identical
win.run("protect-sheet")
check("a confirmation that doesn't match is refused", not protect_ui.protected(win) and
      "not identical" in (SEEN[-1][1] or ""), SEEN[-2:])
SCRIPT.append(protect_dialog("s3cret"))
SCRIPT.append(password("s3cret"))
win.run("protect-sheet")
a = dict(b.sheet_protection(S) or [])
check("Protect Sheet protects with a hashed password", protect_ui.protected(win) and a.get("algorithmName") == "SHA-512"
      and a.get("spinCount") == "100000" and "s3cret" not in str(a), a)
check("the defaults: objects and scenarios protected, selecting allowed",
      a.get("objects") == "1" and a.get("scenarios") == "1" and "selectLockedCells" not in a, a)
check("the button says Unprotect Sheet", any(getattr(c, "label", "") == "Unprotect\nSheet" for c in win.ribbon.controls))

# ---- typing
sel(0, 0)
SEEN.clear()
g.start_edit(initial="x")
check("typing into a locked cell is refused", not g.editing() and SEEN and SEEN[-1][1] == REFUSED, SEEN)
sel(1, 1)
g.start_edit(initial="typed")
g.commit_edit()
check("an unlocked cell takes typing", b.input(S, 1, 1) == "typed")
sel(0, 4)
pump(0.02)
check("a Hidden formula isn't shown in the formula bar", win.formula_bar.text() == "", win.formula_bar.text())
sel(0, 3)
check("other formulas and values still are", win.formula_bar.text() == "3")

# ---- formatting, clearing, pasting, structure
before = b.style_spec(S, 0, 0)
sel(0, 0)
SEEN.clear()
win.run("bold")
check("Bold is refused without Format cells", b.style_spec(S, 0, 0) == before and SEEN[-1][1] == REFUSED)
sel(1, 1)
win.run("bold")
check("on unlocked cells too", not b.cell_style(S, 1, 1)["bold"])
sel(0, 0, 3, 3)
win.run("clear-contents")
check("Delete on locked cells is refused", b.input(S, 0, 0) == "0")
sel(2, 1)
win.run("clear-contents")
check("on an unlocked cell it works", b.input(S, 2, 1) == "")
sel(0, 0)
win.run("copy")
sel(1, 1)
b_in = b.input(S, 1, 1)
win.run("paste")
check("pasting into an unlocked cell works", b.input(S, 1, 1) == "0", b.input(S, 1, 1))
sel(0, 0, 1, 1)
win.run("copy")
sel(1, 1)
win.run("paste")
check("a paste that would reach locked cells is taken back", b.input(S, 1, 2) == "12" and b.input(S, 1, 1) == "0",
      (b.input(S, 1, 1), b.input(S, 1, 2)))
check("and can't be redone", not b.can_redo)
sel(3, 0)
win.run("insert-rows")
check("Insert Rows is refused", b.input(S, 3, 0) == "30")
sel(0, 0)
win.run("sort-asc")
check("Sort is refused", b.input(S, 0, 0) == "0")
win.fill_handle((0, 0, 0, 0), (0, 0, 3, 0))
check("the fill handle can't fill locked cells", b.input(S, 1, 0) == "10")

# ---- the walk: no command changes a locked cell of a protected sheet
SKIP = {"close", "exit", "new", "open", "print", "new-window", "help", "switch-windows", "context-menu",
        "share", "account", "feedback", "protect-sheet", "unprotect-sheet", "protect-workbook", "hide-window"}


def needs_arg(k):
    fn = getattr(win, commands.method_name(k), None)
    if fn is None:
        return False
    return any(p.default is p.empty for p in inspect.signature(fn).parameters.values())


def snapshot():
    out = []
    for r in range(8):
        for c in range(5):
            if (r, c) in ((1, 1), (2, 1)):
                continue
            out.append((b.input(S, r, c), b.style_spec(S, r, c) if (r + c) % 3 == 0 else None))
    out.append((b.sheet_count, tuple(map(tuple, b.merges(S) or [])), b.link_count(S)))
    return out


ref = snapshot()
leaks = []
WALK[0] = Gtk.ResponseType.OK
cmds = [k for k in commands.LABELS if k not in SKIP and not needs_arg(k)]
for i, cmd in enumerate(cmds):
    for rg in ((0, 0, 3, 3), (0, 0, 0, 0)):
        g.sel.ranges = [rg]
        g.sel.active = rg[:2]
        g.sheet = 0
        try:
            win.run(cmd)
        except Exception:
            ERRORS.append(cmd + ": " + traceback.format_exc())
        pump(0.005)
        if g.editing():
            g.cancel_edit()
        if g.sheet != 0:
            g.set_sheet(0)
        if snapshot() != ref:
            leaks.append(cmd)
            b.set_sheet_protection(S, None)
            win.run("undo")
            b.set_sheet_protection(S, list(a.items()) if isinstance(a, dict) else a)
            ref = snapshot()
WALK[0] = None
check("no command changes the protected sheet's locked cells (%d commands)" % len(cmds), not leaks, leaks)
check("and it is still protected", protect_ui.protected(win))

# ---- permissions
SCRIPT.append(password("nope"))
SCRIPT.append(lambda d: Gtk.ResponseType.OK)
win.run("protect-sheet")
check("a wrong password doesn't unprotect", protect_ui.protected(win) and "not correct" in (SEEN[-1][1] or ""))
SCRIPT.append(password("s3cret"))
win.run("protect-sheet")
check("the right one does", not protect_ui.protected(win))
SCRIPT.append(protect_dialog("", allow=("formatCells", "insertRows", "formatColumns")))
win.run("protect-sheet")
check("no password: none asked to confirm", protect_ui.protected(win) and "hashValue" not in dict(b.sheet_protection(S)))
sel(0, 0)
win.run("bold")
check("Format cells allowed: Bold works, on locked cells too", b.cell_style(S, 0, 0)["bold"])
sel(5, 0)
win.run("insert-rows")
check("Insert rows allowed", b.input(S, 6, 0) == "50", b.input(S, 6, 0))
win.run("delete-rows")
check("Delete rows still not", b.input(S, 6, 0) == "50")
SEEN.clear()
win.run("protect-sheet")          # unprotect (no password)
check("unprotected with no password asked", not protect_ui.protected(win) and not SEEN, SEEN)
SCRIPT.append(lambda d: ([d.perms["selectLockedCells"].set_active(False)], Gtk.ResponseType.OK)[1])
win.run("protect-sheet")
g.select_cell(2, 1)
g.select_cell(0, 0)
check("without Select locked cells a locked cell can't be selected", tuple(g.sel.active) == (2, 1), g.sel.active)
win.run("protect-sheet")

# ---- Protect Workbook
SCRIPT.append(lambda d: (d.password.set_text("wb"), Gtk.ResponseType.OK)[1])
SCRIPT.append(password("wb"))
win.run("protect-workbook")
check("Protect Workbook locks the structure", protect_ui.structure_locked(win))
n = b.sheet_count
SEEN.clear()
win.run("insert-sheet")
check("no sheets can be added", b.sheet_count == n and "Workbook is protected" in (SEEN[-1][1] or ""))
win.run("rename-sheet")
win.run("delete-sheet")
check("or renamed or deleted", b.sheet_count == n and b.sheet_info(0)["name"] == "Sheet1")
c0 = b.cell_style(S, 7, 4)["bold"]
sel(7, 4)
win.run("bold")
check("the cells themselves still change", b.cell_style(S, 7, 4)["bold"] != c0)

# ---- saved and opened again
sel(0, 0)
SCRIPT.append(protect_dialog("again"))
SCRIPT.append(password("again"))
win.run("protect-sheet")
path = os.path.join(tempfile.mkdtemp(), "prot.xlsx")
ok = win.save_to(path)
check("saved", ok, SEEN[-2:])
from folio_sheets.core import Book  # noqa: E402
b2 = Book.open(path)
pa, wa = b2.sheet_protection(S), b2.workbook_protection()
check("sheet protection survives saving", pa and dict(pa).get("sheet") == "1")
check("its password still opens it", protect_ui.check_password(pa, "again") and not protect_ui.check_password(pa, "Again"))
check("workbook protection too", wa and dict(wa).get("lockStructure") == "1" and
      protect_ui.check_password(wa, "wb", "workbook"))
check("Lock Cell survives", not b2.locked_in(S, [(2, 1, 2, 1)]))
check("an old-style password still opens", protect_ui.check_password([("sheet", "1"), ("password", "83AF")], "password"))
win.run("undo")
check("undo doesn't take protection off", protect_ui.protected(win))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
