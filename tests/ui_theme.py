# X2-01: run inside Folio Sheet (FOLIO_TEST_SCRIPT) started under GTK_THEME=Adwaita:dark or
# FolioTestColour (theme_run.sh); every ribbon tab, the File view, the task panes and every dialog
# a command opens keep their own colours, and all text stays readable.
import os
import sys
import time
import traceback

sys.path.insert(0, os.path.dirname(os.path.abspath(os.environ["FOLIO_TEST_SCRIPT"])))
import themecheck  # noqa: E402
from gi.repository import Gtk  # noqa: E402

OUT = os.environ.get("UI_OUT", "/tmp/ui_out")
os.makedirs(OUT, exist_ok=True)
FAILS = []


def pump(t=0.2):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.003)


def look(w, where):
    pump(0.25)
    probs = themecheck.audit(w, where)
    for p in probs:
        print("FAIL " + p, flush=True)
    if probs:
        themecheck.save(w, os.path.join(OUT, "theme_%s%s_%s.png" % (themecheck.mode(), os.environ.get("FOLIO_THEME", ""), where.replace(" ", "_").replace("/", "_"))))
    FAILS.extend(probs)
    if not probs:
        print("PASS %s (%s)" % (where, themecheck.mode()), flush=True)


try:
    print("theme", os.environ.get("GTK_THEME"), "mode", themecheck.mode(), "app", os.environ.get("FOLIO_THEME", "desktop"))
    b = win.book
    for r in range(6):
        for c in range(4):
            b.set_input(0, r, c, str(r * c) if r else "Head%d" % c)
    for tab in list(win.ribbon.tabs):
        if tab.button is None or not tab.button.get_visible():
            continue
        win.ribbon.select(tab)
        look(win, "tab " + tab.name)
    win.backstage.open()
    for page in ("home", "new", "open", "info", "saveas", "print", "share", "export"):
        win.backstage.nav(page)
        look(win, "file " + page)
    win.backstage.close()
    pump()
    # the task panes
    for cmd, attr in (("selection-pane", "selection_pane"), ("check-accessibility", "access_pane"),
                      ("workbook-links", "links_pane"), ("clipboard-pane", "clipboard_pane"),
                      ("show-comments", "comments_pane"), ("queries-connections", "queries_pane")):
        win.run(cmd)
        pane = getattr(win, attr, None)
        if pane is not None and pane.get_visible():
            look(win, "pane " + cmd)
            pane.hide()
    # Page Layout view
    win.run("view-page-layout")
    look(win, "view page layout")
    win.run("view-normal")
    # every dialog a command opens, once per title
    seen = set()

    def look_run(d):
        t = d.get_title() or type(d).__name__
        if t not in seen:
            seen.add(t)
            d.show_all()
            pump(0.3)
            look(d, "dialog " + t)
        return Gtk.ResponseType.CANCEL

    Gtk.Dialog.run = look_run
    Gtk.MessageDialog.run = look_run
    Gtk.NativeDialog.run = lambda d: Gtk.ResponseType.CANCEL
    Gtk.PrintOperation.run = lambda *a: Gtk.PrintOperationResult.CANCEL
    from folio_sheets import commands
    skip = {"close", "exit", "new", "open", "print", "new-window", "help", "switch-windows", "context-menu", "share",
            "account", "feedback", "send-attachment", "send-pdf"}
    import inspect
    for cmd in list(commands.LABELS):
        if cmd in skip:
            continue
        fn = getattr(win, commands.method_name(cmd), None)
        if fn is None or any(p.default is p.empty for p in inspect.signature(fn).parameters.values()):
            continue
        win.grid.sel.ranges = [(0, 0, 3, 2)]
        win.grid.sel.active = (0, 0)
        try:
            win.run(cmd)
        except Exception:
            pass
        if win.last_menu is not None:
            win.last_menu.popdown()
            win.last_menu = None
        if win.grid.editing():
            win.grid.cancel_edit()
        if win.backstage_open():
            win.backstage.close()
        for w in Gtk.Window.list_toplevels():
            if w is not win and w.get_mapped() and isinstance(w, Gtk.Dialog):
                w.destroy()
        pump(0.01)
    print("dialogs checked:", len(seen), flush=True)
except Exception:
    traceback.print_exc()
    FAILS.append("exception")
print("%d checks, %d failed" % (len(FAILS) + 1, len(FAILS)), flush=True)
win.book.mark_saved()
os._exit(1 if FAILS else 0)
