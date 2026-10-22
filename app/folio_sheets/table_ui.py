"""Tables: Format as Table (the style gallery and Create Table), the Table Design tab,
and the total-row drop-down."""
from gi.repository import GLib, Gtk

from .core import a1

FAMILIES = [("Light", 21), ("Medium", 28), ("Dark", 11)]
DEFAULT_STYLE = "TableStyleMedium2"

TOTALS = [("None", "none"), ("Average", "average"), ("Count", "count"), ("Count Numbers", "countNums"), ("Max", "max"),
          ("Min", "min"), ("Sum", "sum"), ("StdDev", "stdDev"), ("Var", "var")]


def style_label(name):
    for fam, _ in FAMILIES:
        if name.startswith("TableStyle" + fam):
            return "%s %s" % (fam, name[len("TableStyle" + fam):])
    return name


def gallery(win, on_pick, with_clear=False):
    """A popover with every built-in style."""
    pop = Gtk.Popover()
    sw = Gtk.ScrolledWindow()
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    sw.set_min_content_height(420)
    sw.set_min_content_width(520)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.set_border_width(8)
    sw.add(box)
    pop.add(sw)
    pop.buttons = []
    for fam, n in FAMILIES:
        lab = Gtk.Label(xalign=0)
        lab.set_markup("<b>%s</b>" % fam)
        box.pack_start(lab, False, False, 0)
        flow = Gtk.FlowBox()
        flow.set_selection_mode(Gtk.SelectionMode.NONE)
        flow.set_max_children_per_line(7)
        flow.set_min_children_per_line(7)
        for k in range(1, n + 1):
            name = "TableStyle%s%d" % (fam, k)
            b = Gtk.Button()
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_tooltip_text(style_label(name))
            area = Gtk.DrawingArea()
            area.set_size_request(61, 46)
            area.connect("draw", lambda a, cr, name=name: (win.book.render_table_style(cr, name, float(a.get_allocated_width()),
                                                                                       float(a.get_allocated_height())), False)[1])
            b.add(area)
            b.style_name = name

            def go(_b, name=name):
                pop.popdown()
                GLib.idle_add(lambda: (on_pick(name), False)[1])
            b.connect("clicked", go)
            flow.add(b)
            pop.buttons.append(b)
        box.pack_start(flow, False, False, 0)
    if with_clear:
        clear = Gtk.Button(label="Clear")
        clear.set_relief(Gtk.ReliefStyle.NONE)
        clear.connect("clicked", lambda _b: (pop.popdown(), on_pick(None)))
        box.pack_start(clear, False, False, 0)
    sw.show_all()
    return pop


def _guess_headers(win, rg):
    b = win.book
    s = win.grid.sheet
    r1, c1, r2, c2 = rg
    if r2 <= r1:
        return True
    for c in range(c1, c2 + 1):
        if isinstance(b.value(s, r1, c), float):
            return False
    return any(isinstance(b.value(s, r1, c), str) for c in range(c1, c2 + 1))


def create(win, style=DEFAULT_STYLE):
    """Create Table: where the data is, and whether it has headers."""
    g = win.grid
    b = win.book
    s = g.sheet
    i = b.table_at(s, *g.sel.active)
    if i is not None:
        # already a table: Format as Table changes its style
        b.set_table_style(s, i, style)
        g.refresh()
        win.update_title()
        return True
    rg = tuple(g.sel.ranges[-1])
    if rg[0] == rg[2] and rg[1] == rg[3]:
        rg = tuple(b.current_region(s, rg[0], rg[1]))
    d = Gtk.Dialog(title="Create Table", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    box.set_border_width(12)
    d.get_content_area().pack_start(box, True, True, 0)
    box.pack_start(Gtk.Label(label="Where is the data for your table?", xalign=0), False, False, 0)
    ref = Gtk.Entry(text="=$%s$%d:$%s$%d" % (a1(rg[0], rg[1]).rstrip("0123456789"), rg[0] + 1,
                                             a1(rg[2], rg[3]).rstrip("0123456789"), rg[2] + 1))
    ref.set_name("range")
    ref.set_activates_default(True)
    box.pack_start(ref, False, False, 0)
    hdr = Gtk.CheckButton(label="My table has headers")
    hdr.set_name("headers")
    hdr.set_active(_guess_headers(win, rg))
    box.pack_start(hdr, False, False, 0)
    d.show_all()
    while True:
        r = d.run()
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        try:
            x = b.parse_ref(ref.get_text().lstrip("=")) if hasattr(b, "parse_ref") else None
        except ValueError:
            x = None
        if x is None:
            from . import core
            try:
                x = core.parse_ref(ref.get_text().lstrip("="))
            except Exception:
                x = None
        if not x:
            win.dialogs.message("The reference isn't valid.", "Create Table")
            continue
        rect = tuple(x[-4:]) if len(x) > 4 else tuple(x)
        try:
            b.create_table(s, rect, hdr.get_active(), style)
        except ValueError as e:
            win.dialogs.message(str(e), "Create Table")
            continue
        break
    d.destroy()
    t = b.tables(s)[b.table_at(s, rect[0], rect[1])]
    g.sel.ranges = [tuple(t["ref"])]
    g.sel.active = (t["ref"][0], t["ref"][1])
    g.refresh()
    win.update_title()
    win.on_selection_changed()
    tab = win.ribbon.tab_by_name("Table Design")
    if tab is not None:
        win.ribbon.select(tab)
    return True


def current(win):
    g = win.grid
    i = win.book.table_at(g.sheet, *g.sel.active)
    if i is None:
        win.status.extra.set_text("Select a cell in a table first.")
    return i


def totals_menu(win, r, c, rect):
    """The drop-down of a total-row cell."""
    g = win.grid
    b = win.book
    s = g.sheet
    i = b.table_at(s, r, c)
    if i is None:
        return
    t = b.tables(s)[i]
    col = c - t["ref"][1]
    menu = Gtk.Menu()
    for lab, key in TOTALS:
        mi = Gtk.MenuItem(label=lab)
        mi.connect("activate", lambda _m, key=key: GLib.idle_add(lambda: (set_total(win, i, col, key), False)[1]))
        menu.append(mi)
    menu.show_all()
    menu.attach_to_widget(g.area, None)
    from gi.repository import Gdk
    rr = Gdk.Rectangle()
    x, y, w, h = rect
    rr.x, rr.y, rr.width, rr.height = int(x), int(y), int(w), int(h)
    menu.popup_at_rect(g.area.get_window(), rr, Gdk.Gravity.SOUTH_WEST, Gdk.Gravity.NORTH_WEST, None)
    win.totals_menu = menu
    return menu


def set_total(win, i, col, key):
    try:
        win.book.set_table_total(win.grid.sheet, i, col, key)
    except ValueError as e:
        win.dialogs.message(str(e))
        return
    win.grid.refresh()
    win.update_title()


def resize(win, i):
    b = win.book
    s = win.grid.sheet
    t = b.tables(s)[i]
    d = Gtk.Dialog(title="Resize Table", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    box.set_border_width(12)
    d.get_content_area().pack_start(box, True, True, 0)
    box.pack_start(Gtk.Label(label="Select the new data range for your table:", xalign=0), False, False, 0)
    r1, c1, r2, c2 = t["ref"]
    e = Gtk.Entry(text="=%s:%s" % (a1(r1, c1), a1(r2, c2)))
    e.set_name("range")
    e.set_activates_default(True)
    box.pack_start(e, False, False, 0)
    box.pack_start(Gtk.Label(label="Note: The headers must remain in the same row,\nand the resulting table range must overlap the original table range.", xalign=0), False, False, 0)
    d.show_all()
    from . import core
    while True:
        r = d.run()
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        try:
            x = core.parse_ref(e.get_text().lstrip("="))
            rect = tuple(x[-4:])
            b.resize_table(s, i, rect)
        except Exception as ex:
            win.dialogs.message(str(ex) or "The reference isn't valid.", "Resize Table")
            continue
        break
    d.destroy()
    win.grid.refresh()
    win.update_title()
    return True


OPTIONS = [("Header Row", "header"), ("Total Row", "totals"), ("Banded Rows", "row_stripes"),
           ("First Column", "first_col"), ("Last Column", "last_col"), ("Banded Columns", "col_stripes"),
           ("Filter Button", "filter_button")]


def build_tab(win, B):
    """The Table Design tab."""
    from .ribbon import Tab
    r = win.ribbon
    t = r.building(Tab("Table Design", "JT", contextual=True))
    win.table_name = Gtk.Entry()
    win.table_name.set_width_chars(12)
    win.table_name.set_name("table-name")

    def rename(e):
        i = current(win)
        if i is None:
            return
        try:
            win.book.rename_table(win.grid.sheet, i, e.get_text())
        except ValueError as ex:
            win.dialogs.message(str(ex), "Table Name")
            win.sync_table_tab()
            return
        win.update_title()
        win.grid.area.grab_focus()
    win.table_name.connect("activate", rename)
    name_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    name_box.pack_start(Gtk.Label(label="Table Name:", xalign=0), False, False, 0)
    name_box.pack_start(win.table_name, False, False, 0)
    t.controls.append(win.table_name)
    B.group(t, "Properties", [name_box, B.small("Resize Table", "r-resize-table-sm", "table-resize", "RE")])
    B.group(t, "Tools", [[B.small("Remove Duplicates", "r-remove-duplicates-sm", "remove-duplicates", "A"),
                          B.small("Convert to Range", "r-convert-range-sm", "table-convert", "G")]])
    win.table_checks = {}
    cols = []
    for lab, key in OPTIONS:
        chk = Gtk.CheckButton(label=lab)
        chk.set_name("opt-" + key)
        chk.action = "table-option:" + key
        chk.keytip = lab[0] + lab.split()[-1][0]
        chk.press = (lambda c=chk: c.set_active(not c.get_active()))
        chk.connect("toggled", lambda c, key=key: None if win.syncing_checks else win.run("table-option:%s:%d" % (key, c.get_active())))
        t.controls.append(chk)
        win.table_checks[key] = chk
        cols.append(chk)
    B.group(t, "Table Style Options", [cols[0:3], cols[3:6], cols[6:7]])
    B.group(t, "Table Styles", [B.big("Quick\nStyles", "r-format-as-table", None, "S",
                                      menu=lambda: gallery(win, lambda n: win.run("table-style:%s" % (n or "none")), True))])
    r.add_tab(t)
    r.show_contextual("Table Design", False)
