"""Data tab: Advanced Filter and Consolidate."""
import math
import re

from gi.repository import Gtk

from .core import col_name

FUNCS = [("Sum", "SUM"), ("Count", "COUNTA"), ("Average", "AVERAGE"), ("Max", "MAX"), ("Min", "MIN"),
         ("Product", "PRODUCT"), ("Count Numbers", "COUNT"), ("StdDev", "STDEV"), ("StdDevp", "STDEVP"),
         ("Var", "VAR"), ("Varp", "VARP")]


def abs_ref(rg):
    r1, c1, r2, c2 = rg
    a = "$%s$%d" % (col_name(c1), r1 + 1)
    return a if (r1, c1) == (r2, c2) else a + ":$%s$%d" % (col_name(c2), r2 + 1)


def sheet_ref(name):
    return name if re.match(r"^[A-Za-z_][A-Za-z0-9_.]*$", name) else "'%s'" % name.replace("'", "''")


def full_ref(b, s, rg):
    return "%s!%s" % (sheet_ref(b.sheet_info(s)["name"]), abs_ref(rg))


def parse(win, text):
    from .pivot_ui import parse_source
    r = parse_source(win, text)
    return None if isinstance(r, str) else r


def _name_ref(b, s, name):
    for n in b.names():
        if n[0].lower() == name.lower() and n[2] == s:
            return n[1]
    return None


def _row(grid, k, label, widget):
    grid.attach(Gtk.Label(label=label, xalign=0), 0, k, 1, 1)
    widget.set_hexpand(True)
    grid.attach(widget, 1, k, 1, 1)


# ------------------------------------------------------------------ Advanced Filter
def advanced_filter(win):
    g, b = win.grid, win.book
    s = g.sheet
    r, c = g.sel.active
    rg = g.sel.ranges[-1]
    list_rg = tuple(rg) if rg[0:2] != rg[2:4] else tuple(b.current_region(s, r, c))
    d = Gtk.Dialog(title="Advanced Filter", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    box.pack_start(Gtk.Label(label="Action", xalign=0), False, False, 0)
    in_place = Gtk.RadioButton.new_with_label_from_widget(None, "Filter the list, in-place")
    copy = Gtk.RadioButton.new_with_label_from_widget(in_place, "Copy to another location")
    box.pack_start(in_place, False, False, 0)
    box.pack_start(copy, False, False, 0)
    grid = Gtk.Grid(column_spacing=8, row_spacing=6)
    list_e = Gtk.Entry(text=full_ref(b, s, list_rg), activates_default=True)
    crit_e = Gtk.Entry(text=_name_ref(b, s, "_xlnm.Criteria") or "", activates_default=True)
    copy_e = Gtk.Entry(activates_default=True)
    copy_e.set_sensitive(False)
    copy.connect("toggled", lambda w: copy_e.set_sensitive(w.get_active()))
    _row(grid, 0, "List range:", list_e)
    _row(grid, 1, "Criteria range:", crit_e)
    _row(grid, 2, "Copy to:", copy_e)
    box.pack_start(grid, False, False, 0)
    unique = Gtk.CheckButton(label="Unique records only")
    box.pack_start(unique, False, False, 0)
    d.list_e, d.crit_e, d.copy_e, d.copy, d.unique = list_e, crit_e, copy_e, copy, unique
    d.show_all()
    while True:
        resp = d.run()
        if resp != Gtk.ResponseType.OK:
            d.destroy()
            return None
        lst = parse(win, list_e.get_text())
        crit = parse(win, crit_e.get_text()) if crit_e.get_text().strip() else None
        dest = parse(win, copy_e.get_text()) if copy.get_active() else None
        msg = None
        if lst is None:
            msg = "The list range isn't valid."
        elif crit_e.get_text().strip() and crit is None:
            msg = "The criteria range isn't valid."
        elif copy.get_active() and dest is None:
            msg = "Enter a reference for the Copy to range."
        if msg is None:
            try:
                if dest is None:
                    n = b.advanced_filter(lst[0], lst[1], crit, unique.get_active())
                else:
                    n = b.advanced_filter(lst[0], lst[1], crit, unique.get_active(), dest)
            except ValueError as e:
                msg = str(e)
        if msg is not None:
            win.dialogs.message(msg, "Advanced Filter")
            if getattr(d, "_walk_runs", 0) > 1:
                d.destroy()
                return None
            continue
        d.destroy()
        break
    g.refresh()
    win.update_title()
    win.status.extra.set_text("%d record%s found." % (n, "" if n == 1 else "s"))
    return n


def show_all(win):
    """Data > Clear after an advanced filter."""
    try:
        done = win.book.advanced_show_all(win.grid.sheet)
    except ValueError:
        done = False
    if done:
        win.grid.refresh()
        win.update_title()
    return done


# ------------------------------------------------------------------ Consolidate
def _num(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool)


def _apply(fn, vals, cells):
    nums = [v for v in vals if _num(v)]
    if fn == "COUNTA":
        return float(len([v for v in cells if v not in (None, "")]))
    if fn == "COUNT":
        return float(len(nums))
    if not nums and fn not in ("SUM",):
        return None
    if fn == "SUM":
        return float(sum(nums))
    if fn == "AVERAGE":
        return sum(nums) / len(nums)
    if fn == "MAX":
        return max(nums)
    if fn == "MIN":
        return min(nums)
    if fn == "PRODUCT":
        p = 1.0
        for x in nums:
            p *= x
        return p
    n = len(nums)
    m = sum(nums) / n
    ss = sum((x - m) ** 2 for x in nums)
    if fn in ("STDEV", "VAR"):
        if n < 2:
            return None
        v = ss / (n - 1)
    else:
        v = ss / n
    return math.sqrt(v) if fn.startswith("STDEV") else v


def consolidate_ranges(win, refs, fn, top, left, links, dest_sheet, at):
    """Consolidate the (sheet, rect) references at `at` on dest_sheet. Returns the output rect."""
    b = win.book

    def label(v):
        return "" if v is None else str(v).strip()
    # (row key, col key) -> [(sheet, r, c, value)]
    acc = {}
    row_keys, col_keys = [], []
    clamped = []
    for s, (r1, c1, r2, c2) in refs:
        # whole rows or columns: as far as the sheet is used
        ext = b.used_extent(s) or (0, 0)
        clamped.append((s, (r1, c1, min(r2, max(ext[0], r1)), min(c2, max(ext[1], c1)))))
    for s, (r1, c1, r2, c2) in clamped:
        dr0 = r1 + (1 if top else 0)
        dc0 = c1 + (1 if left else 0)
        for r in range(dr0, r2 + 1):
            rk = label(b.value(s, r, c1)) if left else r - dr0
            if left and rk == "":
                continue
            if rk not in row_keys:
                row_keys.append(rk)
            for c in range(dc0, c2 + 1):
                ck = label(b.value(s, r1, c)) if top else c - dc0
                if top and ck == "":
                    continue
                if ck not in col_keys:
                    col_keys.append(ck)
                acc.setdefault((rk, ck), []).append((s, r, c, b.value(s, r, c)))
    ar, ac = at
    orow = ar + (1 if top else 0)
    ocol = ac + (1 if left else 0)
    b.begin_group("Consolidate")
    try:
        if top:
            for j, ck in enumerate(col_keys):
                b.set_input(dest_sheet, ar, ocol + j, str(ck))
        if left:
            for i, rk in enumerate(row_keys):
                b.set_input(dest_sheet, orow + i, ac, str(rk))
        for i, rk in enumerate(row_keys):
            for j, ck in enumerate(col_keys):
                items = acc.get((rk, ck), [])
                if not items:
                    continue
                if links:
                    refs_t = ",".join("%s!$%s$%d" % (sheet_ref(b.sheet_info(s)["name"]), col_name(c), r + 1) for s, r, c, _v in items)
                    b.set_input(dest_sheet, orow + i, ocol + j, "=%s(%s)" % (fn, refs_t))
                    continue
                v = _apply(fn, [x[3] for x in items], [x[3] for x in items])
                if v is None:
                    b.set_input(dest_sheet, orow + i, ocol + j, "#DIV/0!" if fn in ("AVERAGE", "STDEV", "VAR", "STDEVP", "VARP") else "")
                else:
                    b.set_input(dest_sheet, orow + i, ocol + j, "%.15g" % v)
    finally:
        b.end_group()
    return (ar, ac, orow + len(row_keys) - 1, ocol + len(col_keys) - 1)


def consolidate(win):
    g, b = win.grid, win.book
    st = win.__dict__.setdefault("consolidate_state", {"fn": "SUM", "refs": [], "top": False, "left": False, "links": False})
    d = Gtk.Dialog(title="Consolidate", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Close", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    box.pack_start(Gtk.Label(label="Function:", xalign=0), False, False, 0)
    fn = Gtk.ComboBoxText()
    for lab, f in FUNCS:
        fn.append(f, lab)
    fn.set_active_id(st["fn"])
    box.pack_start(fn, False, False, 0)
    box.pack_start(Gtk.Label(label="Reference:", xalign=0), False, False, 0)
    row = Gtk.Box(spacing=6)
    ref = Gtk.Entry(activates_default=False)
    rg = g.sel.ranges[-1]
    if rg[0:2] != rg[2:4]:
        ref.set_text(full_ref(b, g.sheet, tuple(rg)))
    row.pack_start(ref, True, True, 0)
    box.pack_start(row, False, False, 0)
    box.pack_start(Gtk.Label(label="All references:", xalign=0), False, False, 0)
    store = Gtk.ListStore(str)
    for t in st["refs"]:
        store.append([t])
    tv = Gtk.TreeView(model=store)
    tv.set_headers_visible(False)
    tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
    sw = Gtk.ScrolledWindow()
    sw.set_shadow_type(Gtk.ShadowType.IN)
    sw.set_size_request(340, 110)
    sw.add(tv)
    lrow = Gtk.Box(spacing=6)
    lrow.pack_start(sw, True, True, 0)
    side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    lrow.pack_start(side, False, False, 0)
    box.pack_start(lrow, True, True, 0)

    def add(*_):
        t = ref.get_text().strip()
        if not t:
            return False
        if parse(win, t) is None:
            win.dialogs.message("Reference isn't valid.", "Consolidate")
            return False
        if t not in [x[0] for x in store]:
            store.append([t])
        return True

    def delete(*_):
        m, it = tv.get_selection().get_selected()
        if it is not None:
            m.remove(it)
    for lab, f in (("Add", add), ("Delete", delete)):
        bt = Gtk.Button(label=lab)
        bt.connect("clicked", f)
        side.pack_start(bt, False, False, 0)
    box.pack_start(Gtk.Label(label="Use labels in", xalign=0), False, False, 0)
    top = Gtk.CheckButton(label="Top row")
    top.set_active(st["top"])
    left = Gtk.CheckButton(label="Left column")
    left.set_active(st["left"])
    links = Gtk.CheckButton(label="Create links to source data")
    links.set_active(st["links"])
    for w in (top, left, links):
        box.pack_start(w, False, False, 0)
    d.fn, d.ref, d.store, d.add, d.top, d.left, d.links = fn, ref, store, add, top, left, links
    d.show_all()
    resp = d.run()
    refs_t = [x[0] for x in store]
    if resp == Gtk.ResponseType.OK and not refs_t and ref.get_text().strip():
        if add():
            refs_t = [x[0] for x in store]
    st.update(fn=fn.get_active_id() or "SUM", refs=refs_t, top=top.get_active(), left=left.get_active(),
              links=links.get_active())
    d.destroy()
    if resp != Gtk.ResponseType.OK:
        return None
    refs = [parse(win, t) for t in refs_t]
    refs = [x for x in refs if x]
    if not refs:
        win.dialogs.message("Select a range to consolidate and click Add.", "Consolidate")
        return None
    at = tuple(g.sel.active)
    out = consolidate_ranges(win, refs, st["fn"], st["top"], st["left"], st["links"], g.sheet, at)
    g.sel.ranges = [out]
    g.refresh()
    win.update_title()
    return out
