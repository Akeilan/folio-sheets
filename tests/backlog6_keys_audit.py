# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Backlog 6 editing behaviour driven with real X input (xdotool).
import shutil as _sh
if _sh.which("xdotool") is None:
    print("NOTE xdotool is not installed; real-input checks skipped")
    import os as _os; _os._exit(0)
import os, sys, time, subprocess, traceback
from gi.repository import Gtk, Gdk, GLib
ERRORS = []
_hook = sys.excepthook
sys.excepthook = lambda t, v, tb: (ERRORS.append("".join(traceback.format_exception(t, v, tb))), _hook(t, v, tb))
N = [0, 0]
def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending(): Gtk.main_iteration_do(False)
        time.sleep(0.004)
def ck(name, got, want=True):
    N[0] += 1
    ok = (abs(got - want) < 1e-9) if isinstance(want, float) and isinstance(got, float) else got == want
    if not ok: N[1] += 1
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else "   want %r   got %r" % (want, got)), flush=True)
def info(name, v): print("INFO " + name + "  " + repr(v), flush=True)
DLG = []      # titles of dialogs that ran
ANSWER = [Gtk.ResponseType.CANCEL]
def fake_run(d):
    pump(0.05); DLG.append(d.get_title() or type(d).__name__)
    a = ANSWER[0]
    return a(d) if callable(a) else a
Gtk.Dialog.run = fake_run; Gtk.MessageDialog.run = fake_run
try: Gtk.NativeDialog.run = lambda d: (DLG.append("native:" + (d.get_title() or "")), Gtk.ResponseType.CANCEL)[1]
except Exception: pass
MSG = []
try:
    _m = win.dialogs.message
    win.dialogs.message = lambda t, *a, **k: MSG.append(t)
except Exception: pass
g = win.grid; b = win.book
XID = win.get_window().get_xid()
def xdo(*a):
    p = subprocess.Popen(["xdotool"] + [str(x) for x in a], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    while p.poll() is None: pump(0.01)
    pump(0.08)
def focus():
    win.present(); pump(0.1); xdo("windowfocus", "--sync", XID); g.area.grab_focus(); pump(0.1)
def key(*ks):
    for k in ks: xdo("key", "--clearmodifiers", k)
def typ(s): xdo("type", "--delay", "12", "--", s); pump(0.05)
def at(r, c):
    if g.editing(): g.cancel_edit()
    g.select_cell(r, c); pump(0.05)
def A(): return tuple(g.sel.active)
def R(): return [tuple(x) for x in g.sel.ranges]
def V(r, c, s=None): 
    x = b.value(g.sheet if s is None else s, r, c); return tuple(x) if isinstance(x, list) else x
def I(r, c, s=None): return b.input(g.sheet if s is None else s, r, c)
def D(r, c, s=None): return b.display(g.sheet if s is None else s, r, c)
def cell_xy(r, c):
    x, y, w, h = b.cell_rect(g.sheet, r, c, g.params())
    ox, oy = g.area.translate_coordinates(win, 0, 0); wx, wy = win.get_window().get_origin()[1:]
    return int(wx + ox + x + w / 2), int(wy + oy + y + h / 2)
def click(r, c, button=1, n=1, mods=None):
    x, y = cell_xy(r, c); xdo("mousemove", x, y)
    if mods: xdo("keydown", mods)
    xdo("click", "--repeat", n, "--delay", 60, button)
    if mods: xdo("keyup", mods)
    pump(0.15)
def clear_sheet():
    if g.editing(): g.cancel_edit()
    ext = b.used_extent(g.sheet)
    b.clear_contents(g.sheet, [(0, 0, 200, 40)]); g.refresh(); pump(0.05)
def done():
    print("%d checks, %d failed" % tuple(N), flush=True); print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
    for e in ERRORS[:8]: print(e)
    sys.stdout.flush(); b.mark_saved(); os._exit(0)
import datetime
focus()
# AutoComplete with real keys
b.set_input(0, 0, 0, "apple"); b.set_input(0, 1, 0, "banana")
at(2, 0); focus(); typ("b"); pump(0.2); key("Return"); pump(0.1)
ck("real keys: AutoComplete completes banana", I(2, 0), "banana")
at(3, 0); focus(); typ("ax"); key("Return")
ck("real keys: typing on replaces the suggestion", I(3, 0), "ax")
# Alt+Enter wraps
at(5, 0); focus(); typ("one"); key("alt+Return"); typ("two"); key("Return")
ck("real keys: Alt+Enter wraps", (I(5, 0), bool(b.cell_style(0, 5, 0).get("wrap"))), ("one\ntwo", True))
# brackets
at(6, 0); focus(); typ("=SUM(1,2"); key("Return")
ck("real keys: closing bracket added", I(6, 0), "=SUM(1,2)")
# Ctrl+;
at(7, 0); focus(); key("ctrl+semicolon"); key("Return")
ck("real keys: Ctrl+; enters today", V(7, 0), float((datetime.date.today() - datetime.date(1899, 12, 30)).days))
# Backspace + Esc
b.set_input(0, 8, 0, "keep")
at(8, 0); focus(); key("BackSpace"); key("Escape")
ck("real keys: Backspace then Esc keeps the value", I(8, 0), "keep")
# Enter pastes after copy
at(8, 0); focus(); key("ctrl+c"); at(9, 0); focus(); key("Return")
ck("real keys: Enter pastes after a copy", I(9, 0), "keep")
# F11 chart sheet
for r, row in enumerate([["", "A"], ["x", 1], ["y", 2]]):
    for c, v in enumerate(row):
        if v != "": b.set_input(0, 20 + r, c, str(v))
at(21, 1); focus(); key("F11"); pump(0.3)
ck("real keys: F11 makes a chart sheet", b.sheet_info(g.sheet)["kind"], "chart")
key("ctrl+Next"); pump(0.2)
ck("real keys: Ctrl+PgDn goes on to the worksheet", b.sheet_info(g.sheet)["kind"], "work")
print("UI %d checks, %d failed; errors %d" % (N[0], N[1], len(ERRORS)))
for e in ERRORS[:3]: print(e)
import os; os._exit(1 if N[1] or ERRORS else 0)
