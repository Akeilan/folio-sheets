"""Slicers and timelines (SH-20): Insert Slicer and Insert Timeline for a PivotTable, clicking
items and periods on the sheet (Ctrl adds, Shift extends a timeline), Clear Filter, the Slicer and
Timeline tabs, Report Connections and Slicer Settings."""
from gi.repository import Gdk, Gtk

from . import pivot_ui as P
from .whatif_ui import _dialog, _row, _entry

STYLES = ["Light%d" % n for n in range(1, 7)] + ["Dark%d" % n for n in range(1, 7)]
LEVELS = ["Years", "Quarters", "Months", "Days"]


def target_pivot(win):
    """The PivotTable to slice: the one at the active cell, the selected slicer's, or the only
    one on the sheet."""
    b = win.book
    i = P.current(win)
    if i is not None:
        return i
    k = selected(win)
    if k is not None:
        t = b.slicer_info(win.grid.sheet, k)["target"]
        if t is not None:
            return t[0]
    here = [i for i in range(b.pivot_count()) if b.pivot(i)["sheet"] == win.grid.sheet]
    return here[0] if len(here) == 1 else None


def selected(win):
    """The selected slicer or timeline (its object index), if one alone is selected."""
    g = win.grid
    if win.book is None or len(g.sel_objects) != 1:
        return None
    i = g.sel_objects[0]
    objs = win.book.objects(g.sheet)
    if i < len(objs) and objs[i]["kind"] in ("slicer", "timeline"):
        return i
    return None


def kind_of(win, i):
    objs = win.book.objects(win.grid.sheet)
    return objs[i]["kind"] if i < len(objs) else None


# ---------------------------------------------------------------- Insert Slicers / Insert Timelines
class InsertDialog:
    def __init__(self, win, pi, timeline=False):
        self.win = win
        self.pi = pi
        self.timeline = timeline
        b = win.book
        self.d, grid = _dialog(win, "Insert Timelines" if timeline else "Insert Slicers")
        p = b.pivot(pi)
        self.checks = []
        row = 0
        for f, fd in enumerate(p["fields"]):
            if timeline and not b.timeline_dates(pi, f):
                continue
            cb = Gtk.CheckButton(label=fd["name"])
            cb.field = f
            grid.attach(cb, 0, row, 2, 1)
            self.checks.append(cb)
            row += 1
        self.d.show_all()
        self.made = []

    def tick(self, name, on=True):
        for cb in self.checks:
            if cb.get_label() == name:
                cb.set_active(on)

    def apply(self):
        b = self.win.book
        want = [cb.field for cb in self.checks if cb.get_active()]
        if not want:
            return None
        with b.group("Insert Timeline" if self.timeline else "Insert Slicer"):
            for f in want:
                self.made.append(b.insert_slicer(self.pi, f, self.timeline))
        g = self.win.grid
        s = b.pivot(self.pi)["sheet"]
        if g.sheet != s:
            g.set_sheet(s)
            self.win.sheet_changed()
        g.sel_objects = list(self.made)
        g.refresh()
        self.win.on_object_selection(switch_tab=True)
        self.win.update_title()
        return None


def insert(win, timeline=False):
    title = "Insert Timelines" if timeline else "Insert Slicers"
    if win.book is None:
        return
    pi = target_pivot(win)
    if pi is None:
        win.dialogs.message("Select a cell in a PivotTable, then insert a %s for it." % ("timeline" if timeline else "slicer"), title)
        return None
    if timeline and not any(win.book.timeline_dates(pi, f) for f in range(len(win.book.pivot(pi)["fields"]))):
        win.dialogs.message("This PivotTable doesn't have any fields formatted as dates, so there's no timeline to insert.", title)
        return None
    dlg = InsertDialog(win, pi, timeline)
    P._run(win, dlg, title)
    return dlg


# ---------------------------------------------------------------- clicks on the sheet
def press(win, i, ev):
    """A click inside a slicer or timeline: True when it chose items (no move starts)."""
    g = win.grid
    b = win.book
    s = g.sheet
    rect = b.object_screen_rect(s, i, g.params())
    if rect is None:
        return False
    x0, y0, w, h = rect
    z = g.params()["zoom"]
    hit = b.slicer_hit(s, i, float(ev.x - x0), float(ev.y - y0), float(w), float(h), float(z))
    if hit is None or hit[0] == "header":
        return False
    state = ev.get_state()
    ctrl = bool(state & Gdk.ModifierType.CONTROL_MASK)
    shift = bool(state & Gdk.ModifierType.SHIFT_MASK)
    what, n = hit
    if what == "clear":
        clear(win, i)
    elif what == "item":
        with b.group("Slicer"):
            if shift and getattr(g, "_slicer_anchor", None) is not None and g._slicer_anchor[0] == i:
                # Shift: the run of items from the last one clicked
                a = g._slicer_anchor[1]
                lo, hi = min(a, n), max(a, n)
                b.slicer_choose(s, i, lo, False)
                for k in range(lo + 1, hi + 1):
                    b.slicer_choose(s, i, k, True)
            else:
                b.slicer_choose(s, i, n, ctrl)
                g._slicer_anchor = (i, n)
        after(win)
    elif what == "period":
        a = g._slicer_anchor[1] if shift and getattr(g, "_slicer_anchor", None) and g._slicer_anchor[0] == i else n
        g._slicer_anchor = (i, a)
        b.timeline_select(s, i, a, n)
        g._drag = ("timeline", i, a)
        after(win)
    elif what == "level":
        level_menu(win, i, ev)
    return True


def drag(win, d, x, y, done=False):
    """Dragging across a timeline's periods."""
    g = win.grid
    b = win.book
    s = g.sheet
    rect = b.object_screen_rect(s, d[1], g.params())
    if rect is None:
        return
    x0, y0, w, h = rect
    hit = b.slicer_hit(s, d[1], float(x - x0), float(y - y0), float(w), float(h), float(g.params()["zoom"]))
    if hit is not None and hit[0] == "period":
        info = b.slicer_info(s, d[1])
        per = b.timeline_periods(s, d[1])
        lo, hi = min(d[2], hit[1]), max(d[2], hit[1])
        span = (per[lo][1], per[hi][2]) if per else None
        if info["span"] != span:
            b.timeline_select(s, d[1], d[2], hit[1])
            after(win)


def level_menu(win, i, ev=None):
    m = Gtk.Menu()
    cur = win.book.slicer_info(win.grid.sheet, i)["level"]
    for k, name in enumerate(LEVELS):
        it = Gtk.CheckMenuItem(label=name)
        it.set_active(k == cur)
        it.connect("activate", lambda _w, k=k: set_level(win, i, k))
        m.append(it)
    m.show_all()
    m.attach_to_widget(win.grid.area, None)
    if ev is not None:
        m.popup_at_pointer(ev)
    return m


def after(win):
    win.grid.refresh()
    P.after(win)
    win.update_title()


# ---------------------------------------------------------------- the commands
def _need(win):
    i = selected(win)
    if i is None:
        win.dialogs.message("Select a slicer or timeline first.", "Slicer")
    return i


def clear(win, i=None):
    i = selected(win) if i is None else i
    if i is None:
        return _need(win)
    win.book.slicer_clear(win.grid.sheet, i)
    after(win)


def set_caption(win, text, i=None):
    i = selected(win) if i is None else i
    if i is None:
        return
    win.book.set_slicer(win.grid.sheet, i, caption=text, label="Slicer Caption")
    after(win)


def set_columns(win, n, i=None):
    i = selected(win) if i is None else i
    if i is None:
        return
    win.book.set_slicer(win.grid.sheet, i, columns=int(n), label="Columns")
    after(win)


def set_style(win, name, i=None):
    i = selected(win) if i is None else i
    if i is None:
        return _need(win)
    tl = kind_of(win, i) == "timeline"
    full = name if name.startswith(("Slicer", "TimeSlicer")) else ("TimeSlicerStyle" if tl else "SlicerStyle") + name
    win.book.set_slicer(win.grid.sheet, i, style=full, label="Slicer Style")
    after(win)


def set_level(win, i, k):
    win.book.set_slicer(win.grid.sheet, i, level=int(k), label="Time Level")
    after(win)


def toggle_header(win, i=None):
    i = selected(win) if i is None else i
    if i is None:
        return _need(win)
    on = win.book.slicer_info(win.grid.sheet, i)["header"]
    win.book.set_slicer(win.grid.sheet, i, header=not on, label="Header")
    after(win)


class SettingsDialog:
    """Slicer Settings / Timeline Settings: name, caption, header, sorting is the field's."""
    def __init__(self, win, i):
        self.win = win
        self.i = i
        info = win.book.slicer_info(win.grid.sheet, i)
        self.timeline = info["timeline"]
        self.d, grid = _dialog(win, "Timeline Settings" if self.timeline else "Slicer Settings")
        lab = Gtk.Label(xalign=0)
        lab.set_text("Source Name:   %s" % info["field"])
        grid.attach(lab, 0, 0, 2, 1)
        lab = Gtk.Label(xalign=0)
        lab.set_text("Name to use in formulas:   %s" % info["cache"])
        grid.attach(lab, 0, 1, 2, 1)
        self.caption = _entry("slicer-caption", info["caption"])
        _row(grid, 2, "Caption:", self.caption)
        self.header = Gtk.CheckButton(label="Display header")
        self.header.set_active(info["header"])
        grid.attach(self.header, 0, 3, 2, 1)
        self.columns = Gtk.SpinButton.new_with_range(1, 20, 1)
        self.columns.set_value(info["columns"])
        if not self.timeline:
            _row(grid, 4, "Number of columns:", self.columns)
        self.d.show_all()

    def apply(self):
        cap = self.caption.get_text().strip()
        kw = dict(caption=cap or None, header=self.header.get_active(), label=self.d.get_title())
        if not self.timeline:
            kw["columns"] = int(self.columns.get_value())
        self.win.book.set_slicer(self.win.grid.sheet, self.i, **kw)
        after(self.win)
        return None


def settings(win, i=None):
    i = selected(win) if i is None else i
    if i is None:
        return _need(win)
    dlg = SettingsDialog(win, i)
    P._run(win, dlg, dlg.d.get_title())
    return dlg


class ConnectionsDialog:
    """Report Connections / Filter Connections: the PivotTables a slicer filters (those built
    on the same data)."""
    def __init__(self, win, i):
        self.win = win
        self.i = i
        b = win.book
        s = win.grid.sheet
        info = b.slicer_info(s, i)
        self.d, grid = _dialog(win, "Report Connections (%s)" % info["name"])
        t = info["target"]
        base = b.pivot(t[0]) if t is not None else None
        self.checks = []
        row = 0
        for k in range(b.pivot_count()):
            p = b.pivot(k)
            if base is not None and (p["source_sheet"], p["source"], p["source_table"]) != (base["source_sheet"], base["source"], base["source_table"]):
                continue
            if info["field"] not in [f["name"] for f in p["fields"]]:
                continue
            cb = Gtk.CheckButton(label="%s  (%s)" % (p["name"], b.sheet_info(p["sheet"])["name"]))
            cb.pivot = p["name"]
            cb.set_active(p["name"] in info["pivots"])
            grid.attach(cb, 0, row, 2, 1)
            self.checks.append(cb)
            row += 1
        self.d.show_all()

    def apply(self):
        names = [cb.pivot for cb in self.checks if cb.get_active()]
        b = self.win.book
        s = self.win.grid.sheet
        with b.group("Report Connections"):
            # a pivot no longer connected shows everything again
            info = b.slicer_info(s, self.i)
            for n in info["pivots"]:
                if n not in names:
                    k = [j for j in range(b.pivot_count()) if b.pivot(j)["name"] == n]
                    if k:
                        f = [x["name"] for x in b.pivot(k[0])["fields"]].index(info["field"])
                        p = b.pivot(k[0])
                        p["fields"][f]["hidden"] = []
                        b.set_pivot(k[0], p, "Report Connections")
            b.set_slicer(s, self.i, pivots=names, label="Report Connections")
            # the newly connected follow the slicer's choice
            items = b.slicer_items(s, self.i)
            if names and items and not all(on for _t, on in items):
                chosen = [k for k, (_t, on) in enumerate(items) if on]
                b.slicer_choose(s, self.i, chosen[0], False)
                for k in chosen[1:]:
                    b.slicer_choose(s, self.i, k, True)
        after(self.win)
        return None


def connections(win, i=None):
    i = selected(win) if i is None else i
    if i is None:
        # from the PivotTable: its slicers
        pi = P.current(win)
        if pi is not None:
            name = win.book.pivot(pi)["name"]
            for k, o in enumerate(win.book.objects(win.grid.sheet)):
                if o["kind"] in ("slicer", "timeline") and name in win.book.slicer_info(win.grid.sheet, k)["pivots"]:
                    i = k
                    break
        if i is None:
            win.dialogs.message("Select a slicer or timeline to choose the PivotTables it filters.", "Filter Connections")
            return None
    dlg = ConnectionsDialog(win, i)
    P._run(win, dlg, "Report Connections")
    return dlg


# ---------------------------------------------------------------- the Slicer and Timeline tabs
def build_tabs(win, B):
    from .ribbon import Tab
    r = win.ribbon
    for kind in ("Slicer", "Timeline"):
        t = r.building(Tab(kind, "JS" if kind == "Slicer" else "JL", contextual=True))
        cap = Gtk.Entry()
        cap.set_width_chars(14)
        cap.set_name("%s-caption" % kind.lower())
        cap.connect("activate", lambda e: set_caption(win, e.get_text()))
        cb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        cb.pack_start(Gtk.Label(label="%s Caption:" % kind, xalign=0), False, False, 0)
        cb.pack_start(cap, False, False, 0)
        t.controls.append(cap)
        setattr(win, "%s_caption" % kind.lower(), cap)
        B.group(t, kind, [cb, [B.small("Report Connections", "r-filter-connections-sm", "filter-connections", "RC"),
                               B.small("%s Settings" % kind, "r-pivot-options-sm", "slicer-settings", "SS"),
                               B.small("Clear Filter", "r-pivot-clear-sm", "slicer-clear", "CF")]])
        B.group(t, "%s Styles" % kind, [B.big("Quick\nStyles", "r-slicer", None, "QS",
                                                 menu=[(n, "slicer-style:" + n) for n in STYLES])])
        B.group(t, "Arrange", [[B.small("Bring Forward", "r-bring-forward-sm", "bring-forward", "AF"),
                                B.small("Send Backward", "r-send-backward-sm", "send-backward", "AE")]])
        if kind == "Slicer":
            sp = Gtk.SpinButton.new_with_range(1, 20, 1)
            sp.set_name("slicer-columns")
            sp.connect("value-changed", lambda w: set_columns(win, w.get_value()) if not getattr(win, "_slicer_sync", False) else None)
            bb = Gtk.Box(spacing=4)
            bb.pack_start(Gtk.Label(label="Columns:"), False, False, 0)
            bb.pack_start(sp, False, False, 0)
            t.controls.append(sp)
            win.slicer_columns = sp
            B.group(t, "Buttons", [bb])
        else:
            B.group(t, "Show", [[B.small("Header", "r-pivot-field-headers-sm", "slicer-header", "H"),
                                 B.small("Time Level", "r-timeline-sm", None, "T",
                                         menu=[(n, "slicer-level:%d" % k) for k, n in enumerate(LEVELS)])]])
        r.add_tab(t)
        r.show_contextual(kind, False)


def sync(win):
    """Show the tab for the selected slicer or timeline, with its settings."""
    i = selected(win)
    k = kind_of(win, i) if i is not None else None
    win.ribbon.show_contextual("Slicer", k == "slicer")
    win.ribbon.show_contextual("Timeline", k == "timeline")
    if i is None:
        return None
    info = win.book.slicer_info(win.grid.sheet, i)
    win._slicer_sync = True
    try:
        e = win.slicer_caption if k == "slicer" else win.timeline_caption
        if not e.has_focus():
            e.set_text(info["caption"])
        if k == "slicer":
            win.slicer_columns.set_value(info["columns"])
    finally:
        win._slicer_sync = False
    return k
