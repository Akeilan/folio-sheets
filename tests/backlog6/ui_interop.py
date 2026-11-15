exec(open("/tmp/b6/uiharn.py").read())
import inspect
from folio_sheets import commands
ANSWER[0] = Gtk.ResponseType.OK
def fr(d):
    pump(0.01); n = getattr(d, "_walk_runs", 0); d._walk_runs = n + 1
    return Gtk.ResponseType.OK if n == 0 else Gtk.ResponseType.CANCEL
Gtk.Dialog.run = fr; Gtk.MessageDialog.run = fr
SKIP = {"close", "exit", "new", "open", "print", "new-window", "help", "switch-windows", "context-menu", "share", "account", "feedback", "send-attachment", "send-pdf", "undo", "redo", "delete-sheet", "clear-all", "clear-contents", "delete-rows", "delete-columns", "delete-cells", "cut", "save", "save-as", "visual-basic", "macros", "record-macro"}
for r in range(12):
    for c in range(6):
        b.set_input(0, r, c, ["Name", "Qty", "Price", "When", "Group", "Total"][c] if r == 0 else ["n%d" % r, str(r * 3 % 7 + 1), str(r * 1.5), "1/%d/2024" % r, "g%d" % (r % 3), "=B%d*C%d" % (r + 1, r + 1)][c])
sels = [[(0, 0, 11, 5)], [(1, 1, 5, 2)], [(14, 8, 14, 8)], [(0, 1, 11, 1)], [(20, 0, 22, 2)]]
def needs_arg(k):
    fn = getattr(win, commands.method_name(k), None)
    return fn is not None and any(p.default is p.empty for p in inspect.signature(fn).parameters.values())
cmds = [k for k in commands.LABELS if k not in SKIP and not needs_arg(k) and not k.startswith(("delete", "clear", "pivot-clear", "remove", "table-convert", "ungroup", "unmerge"))]
ran = 0
for i, cmd in enumerate(cmds):
    rg = sels[i % len(sels)]; g.sel.ranges = list(rg); g.sel.active = rg[0][:2]; n = len(ERRORS)
    try: win.run(cmd)
    except Exception: ERRORS.append(cmd + ": " + traceback.format_exc())
    if win.painter: win.painter_finish()
    if win.last_menu is not None: win.last_menu.popdown(); win.last_menu = None
    if g.editing(): g.cancel_edit()
    if win.backstage_open(): win.backstage.close()
    if g.sheet != 0 and i % 7: g.set_sheet(0)
    pump(0.005); ran += 1
info("commands run", ran); info("sheets", b.sheet_count); info("objects per sheet", [len(b.objects(s)) for s in range(b.sheet_count)])
ok = win.save_to("/tmp/b6/walk.xlsx"); info("saved", ok)
import folio_sheets_core as core
bb = core.Book.open("/tmp/b6/walk.xlsx"); bb.save("/tmp/b6/walk2.xlsx"); info("reopened and saved again", bb.sheet_count)
done()
