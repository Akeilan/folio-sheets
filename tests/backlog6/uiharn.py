# Backlog 6 UI harness: real X input (xdotool) into the running app. exec'd by each ui_*.py probe.
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
