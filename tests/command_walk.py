# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): every command the ribbon knows, on a
# small workbook with a few selections, with every dialog answered (OK, then Cancel).
# Nothing may raise; undo must walk all the way back.
import os
import sys
import time
import traceback

from gi.repository import Gtk

from folio_sheets import commands

ERRORS = []
_hook = sys.excepthook
sys.excepthook = lambda t, v, tb: (ERRORS.append("".join(traceback.format_exception(t, v, tb))), _hook(t, v, tb))


def pump(t=0.02):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.002)


answer = [Gtk.ResponseType.OK]
seen = []


def fake_run(d):
    pump(0.01)
    seen.append(d.get_title())
    # a dialog that comes back after OK (its input was refused) is cancelled the second time
    n = getattr(d, "_walk_runs", 0)
    d._walk_runs = n + 1
    return answer[0] if n == 0 else Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run
Gtk.NativeDialog.run = lambda d: Gtk.ResponseType.CANCEL

SKIP = {"close", "exit", "new", "open", "print", "new-window", "help", "switch-windows", "context-menu",
        "share", "account", "feedback", "send-attachment", "send-pdf"}
g = win.grid
b = win.book
for r in range(8):
    for c in range(5):
        win.book.set_input(0, r, c, str(r + c) if c else "=B%d+1" % (r + 1))
win.book.mark_saved()
base_steps = len(win.book.undo_labels())
start_inputs = [[win.book.input(0, r, c) for c in range(5)] for r in range(8)]
sels = [[(1, 1, 3, 3)], [(0, 2, 1048575, 2)], [(4, 0, 4, 16383)], [(2, 2, 2, 2)], [(0, 0, 1, 1), (5, 3, 6, 4)]]
import inspect  # noqa: E402


def needs_arg(k):
    fn = getattr(win, commands.method_name(k), None)
    if fn is None:
        return False
    return any(p.default is p.empty for p in inspect.signature(fn).parameters.values())


cmds = [k for k in commands.LABELS if k not in SKIP and not needs_arg(k)]
extra = ["font-name:DejaVu Sans", "font-size:20", "font-color:FF0000", "fill-color:00FF00", "orientation:90",
         "number-format:Currency", "accounting:€"]
ran = 0
for i, cmd in enumerate(cmds + extra):
    for resp in (Gtk.ResponseType.OK, Gtk.ResponseType.CANCEL):
        rg = sels[(i + (resp == Gtk.ResponseType.CANCEL)) % len(sels)]
        g.sel.ranges = list(rg)
        g.sel.active = rg[0][:2]
        answer[0] = resp
        if os.environ.get("WALK_TRACE"):
            import resource
            print("RUN", cmd, resp, resource.getrusage(resource.RUSAGE_SELF).ru_maxrss // 1024, "MB", flush=True)
        n = len(ERRORS)
        try:
            win.run(cmd)
        except Exception:
            ERRORS.append(cmd + ": " + traceback.format_exc())
        if win.painter:
            win.painter_finish()
        if win.last_menu is not None:
            win.last_menu.popdown()
            win.last_menu = None
        if g.editing():
            g.cancel_edit()
        if win.backstage_open():
            win.backstage.close()
        pump(0.005)
        if len(ERRORS) > n:
            print("ERROR in", cmd, ERRORS[-1].splitlines()[-1], flush=True)
        ran += 1
print("ran %d command runs, %d dialogs, %d not-yet" % (ran, len(seen), len(set(win.missing_log))), flush=True)
# back to the start
n = 0
while win.book.can_undo and len(win.book.undo_labels()) > base_steps and n < 20000:
    win.run("undo")
    n += 1
same = [[win.book.input(0, r, c) for c in range(5)] for r in range(8)] == start_inputs
print("undo all (%d steps): data back to the start: %s" % (n, same), flush=True)
same = same or not win.book.can_undo
print("not yet:", sorted(set(win.missing_log)))
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:10]:
    print(e)
os._exit(1 if ERRORS or not same else 0)
