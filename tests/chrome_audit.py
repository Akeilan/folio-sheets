# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): the window chrome against the Folio Studio
# Ribbon Spec — title bar, tab row and ribbon heights, the Quick Access menu, KeyTips.
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


win.resize(1280, 800)
pump(0.6)
title_h = win.header.get_allocated_height()
tabs_h = win.ribbon.tabbar.get_allocated_height()
body_h = win.ribbon.revealer.get_allocated_height()
check("title bar is 40 px (+-1)", abs(title_h - 40) <= 1, title_h)
check("tab row is 34 px (+-1)", abs(tabs_h - 34) <= 1, tabs_h)
check("ribbon body is 96 px (+-1)", abs(body_h - 96) <= 1, body_h)

# Quick Access: Save, Undo, Redo by default, and a menu to add more
names = [c.get_tooltip_text() for c in win.qat.get_children() if not isinstance(c, Gtk.MenuButton)]
check("Quick Access holds Save, Undo, Redo", [n.split(" (")[0] for n in names] == ["Save", "Undo", "Redo"], names)
check("Quick Access has its customise menu", any(isinstance(c, Gtk.MenuButton) for c in win.qat.get_children()))
win.set_qat("print", True)
pump(0.05)
names = [c.get_tooltip_text().split(" (")[0] for c in win.qat.get_children() if not isinstance(c, Gtk.MenuButton)]
check("Print can be added", names == ["Save", "Undo", "Redo", "Print"], names)
win.set_qat("print", False)
win.set_qat("new", True)
pump(0.05)
names = [c.get_tooltip_text().split(" (")[0] for c in win.qat.get_children() if not isinstance(c, Gtk.MenuButton)]
check("commands keep the menu's order", names == ["New", "Save", "Undo", "Redo"], names)
win.set_qat("new", False)
pump(0.05)

# KeyTips number the Quick Access buttons, not the menu
win.keytips.start()
pump(0.2)
nums = sorted(k for k in win.keytips.targets if k.isdigit())
check("KeyTips number the Quick Access buttons 1-3", nums == ["1", "2", "3"], nums)
win.keytips.leave()

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
