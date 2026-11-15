exec(open("/tmp/b6/uiharn.py").read())
focus()
for r in range(6): b.set_input(g.sheet, r, 0, str(r + 1)); b.set_input(g.sheet, r, 1, ["apple", "pear", "apple", "fig", "pear", "kiwi"][r])
g.refresh(); pump(0.1)
click(2, 2); ck("a click selects the cell", A(), (2, 2))
x0, y0 = cell_xy(1, 1); x1, y1 = cell_xy(3, 3); xdo("mousemove", x0, y0); xdo("mousedown", 1); xdo("mousemove", (x0 + x1) // 2, (y0 + y1) // 2); xdo("mousemove", x1, y1); xdo("mouseup", 1); pump(0.2)
ck("dragging selects a range", R(), [(1, 1, 3, 3)])
click(0, 0); click(2, 1, mods="shift"); ck("Shift+click extends", R(), [(0, 0, 2, 1)])
click(0, 0); click(4, 3, mods="ctrl"); ck("Ctrl+click adds a cell", R(), [(0, 0, 0, 0), (4, 3, 4, 3)])
click(1, 1, n=2); pump(0.2); ck("double-click edits in the cell", bool(g.editing()), True); info("editor text", g.editor.text() if g.editing() else None); key("Escape")
# headers
hx, hy = cell_xy(0, 2); ox, oy = g.area.translate_coordinates(win, 0, 0); wy = win.get_window().get_origin()[2]
xdo("mousemove", hx, wy + oy + 8); xdo("click", 1); pump(0.2); ck("clicking a column header selects the column", R(), [(0, 2, 1048575, 2)])
rx = win.get_window().get_origin()[1] + ox + 10; _, ry = cell_xy(3, 0); xdo("mousemove", rx, ry); xdo("click", 1); pump(0.2); ck("clicking a row header selects the row", R(), [(3, 0, 3, 16383)])
xdo("mousemove", win.get_window().get_origin()[1] + ox + 8, wy + oy + 8); xdo("click", 1); pump(0.2); ck("the corner selects the whole sheet", R(), [(0, 0, 1048575, 16383)])
# column border: double-click autofits; drag resizes
b.set_input(g.sheet, 0, 4, "a very long piece of text indeed"); g.refresh(); pump(0.1)
w0 = b.col_px(g.sheet, 4); xr = b.cell_rect(g.sheet, 0, 4, g.params()); bx = win.get_window().get_origin()[1] + ox + int(xr[0] + xr[2]); xdo("mousemove", bx, wy + oy + 8); xdo("click", "--repeat", 2, "--delay", 60, 1); pump(0.3)
ck("double-click on a column border fits the column", b.col_px(g.sheet, 4) > w0 + 40, True); info("col E px before/after", (w0, b.col_px(g.sheet, 4)))
w0 = b.col_px(g.sheet, 0); xr = b.cell_rect(g.sheet, 0, 0, g.params()); bx = win.get_window().get_origin()[1] + ox + int(xr[0] + xr[2]); xdo("mousemove", bx, wy + oy + 8); xdo("mousedown", 1); xdo("mousemove", bx + 20, wy + oy + 8); xdo("mousemove", bx + 40, wy + oy + 8); xdo("mouseup", 1); pump(0.3)
ck("dragging a column border widens it", b.col_px(g.sheet, 0), w0 + 40)
key("ctrl+z"); ck("...and Ctrl+Z undoes the width", b.col_px(g.sheet, 0), w0)
# fill handle: drag, and double-click
at(0, 0); g.select_range((0, 0, 1, 0), (0, 0)); pump(0.1)
b.clear_contents(g.sheet, [(2, 0, 5, 0)]); g.refresh(); pump(0.1)
rr = b.cell_rect(g.sheet, 1, 0, g.params()); fx = win.get_window().get_origin()[1] + ox + int(rr[0] + rr[2]) - 1; fy = wy + oy + int(rr[1] + rr[3]) - 1
xdo("mousemove", fx, fy); xdo("click", "--repeat", 2, "--delay", 60, 1); pump(0.3)
ck("double-clicking the fill handle fills down beside the data", [V(r, 0) for r in range(6)], [1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
# right-click: context menu
shown = []
_pop = Gtk.Menu.popup_at_pointer
Gtk.Menu.popup_at_pointer = lambda m, e=None: shown.append([c.get_label() for c in m.get_children() if isinstance(c, Gtk.MenuItem) and c.get_label()])
for nm in ("popup_at_rect", "popup_at_widget", "popup"):
    if hasattr(Gtk.Menu, nm): setattr(Gtk.Menu, nm, (lambda m, *a, **k: shown.append([c.get_label() for c in m.get_children() if isinstance(c, Gtk.MenuItem) and c.get_label()])))
click(2, 2); click(4, 1, button=3); pump(0.3); info("right-click menu", shown[-1] if shown else [w for w in Gtk.Window.list_toplevels() if w.get_visible() and w is not win])
ck("right-click moves the selection to the cell under it", A(), (4, 1))
g.select_range((1, 0, 3, 1), (1, 0)); pump(0.1); click(2, 1, button=3); pump(0.2); ck("right-click inside a selection keeps it", R(), [(1, 0, 3, 1)]); key("Escape")
# drag the selection border to move cells
at(0, 1); pump(0.1); rr = b.cell_rect(g.sheet, 0, 1, g.params()); sx = win.get_window().get_origin()[1] + ox + int(rr[0] + rr[2] / 2); sy = wy + oy + int(rr[1] + rr[3]) - 1
tx, ty = cell_xy(8, 3); xdo("mousemove", sx, sy); xdo("mousedown", 1); xdo("mousemove", (sx + tx) // 2, (sy + ty) // 2); xdo("mousemove", tx, ty); xdo("mouseup", 1); pump(0.3)
ck("dragging the selection's border moves the cell", (V(0, 1), V(8, 3)), (None, "apple")); key("ctrl+z")
# wheel
top0 = g.visible_rows()[0] if hasattr(g, "visible_rows") else None; xdo("mousemove", *cell_xy(5, 5)); xdo("click", "--repeat", 3, 5); pump(0.3); info("rows visible before/after wheel", (top0, g.visible_rows()[0]))
xdo("click", "--repeat", 3, 4); pump(0.2)
z0 = g.params().get("zoom"); xdo("keydown", "ctrl"); xdo("click", 4); xdo("keyup", "ctrl"); pump(0.3); info("zoom before/after Ctrl+wheel up", (z0, g.params().get("zoom")))
# name box
nb = None
for a in ("name_box", "namebox"):
    if hasattr(win, a): nb = getattr(win, a)
info("name box attr", type(nb).__name__ if nb is not None else [a for a in dir(win) if "name" in a.lower()][:12])
# status bar aggregates
g.select_range((0, 0, 5, 0), (0, 0)); pump(0.3); info("status bar texts", [w.get_text() for w in win.status.get_children() if hasattr(w, "get_text")] if hasattr(win.status, "get_children") else dir(win.status)[:40])
# second batch of shortcuts
def tried(combo, what, fn):
    before = fn(); key(combo); pump(0.3); after = fn(); info("%s (%s): %r -> %r" % (combo, what, before, after), "CHANGED" if before != after else "nothing happened")
    key("Escape"); focus()
at(0, 0); b.set_input(g.sheet, 0, 6, "5"); g.refresh(); at(0, 6); key("ctrl+b"); at(1, 6); tried("F4", "repeat last action (bold)", lambda: b.cell_style(g.sheet, 1, 6).get("bold"))
at(0, 0); tried("F11", "chart sheet", lambda: (b.sheet_count, len(b.objects(g.sheet))))
g.select_range((0, 0, 5, 1), (0, 0)); b.set_rows_hidden(g.sheet, 2, 2, True); tried("alt+semicolon", "select visible cells only", lambda: R()); b.set_rows_hidden(g.sheet, 2, 2, False)
b.set_input(g.sheet, 9, 0, "=A1+A2"); g.refresh(); at(9, 0); tried("ctrl+bracketleft", "select precedents", lambda: R())
at(0, 0); tried("ctrl+bracketright", "select dependents", lambda: R())
tried("ctrl+F1", "collapse the ribbon", lambda: [getattr(win, a).get_visible() for a in dir(win) if a in ("ribbon",) and hasattr(getattr(win, a), "get_visible")] + [win.get_allocated_height(), g.area.get_allocated_height()])
key("ctrl+F1")
tried("F1", "help", lambda: (len(DLG), len([w for w in Gtk.Window.list_toplevels() if w.get_visible()])))
tried("ctrl+8", "outline symbols", lambda: str(g.params().get("outline")) + str(b.sheet_info(g.sheet)))
tried("ctrl+2", "bold", lambda: b.cell_style(g.sheet, 0, 0).get("bold")); tried("ctrl+3", "italic", lambda: b.cell_style(g.sheet, 0, 0).get("italic")); tried("ctrl+4", "underline", lambda: b.cell_style(g.sheet, 0, 0).get("underline"))
tried("ctrl+shift+o", "select cells with notes", lambda: R())
tried("ctrl+F2", "print preview (backstage)", lambda: win.backstage_open()); 
if win.backstage_open(): win.backstage.close()
tried("ctrl+Tab", "next workbook window", lambda: win.is_active())
done()
