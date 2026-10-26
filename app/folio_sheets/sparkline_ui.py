"""Sparklines (SH-21): Insert › Sparklines puts a small line, column or win/loss chart in each cell
of a row or column, one per row (or column) of the data; the Sparkline tab changes the selected
cell's group. Kept as Excel keeps them, so they show and edit there too."""
from gi.repository import Gtk

from . import APP_NAME
from .core import a1

KINDS = (("line", "Line"), ("column", "Column"), ("winloss", "Win/Loss"))
SHOW = (("high", "High Point"), ("low", "Low Point"), ("negative", "Negative Points"),
        ("first", "First Point"), ("last", "Last Point"), ("markers", "Markers"))
MARKERS = (("negative_color", "Negative Points"), ("markers_color", "Markers"), ("high_color", "High Point"),
           ("low_color", "Low Point"), ("first_color", "First Point"), ("last_color", "Last Point"))
COLORS = (("Dark Blue", 0x376092), ("Blue", 0x4472C4), ("Orange", 0xED7D31), ("Gray", 0xA5A5A5), ("Gold", 0xFFC000),
          ("Green", 0x70AD47), ("Red", 0xD00000), ("Dark Red", 0xC00000), ("Black", 0x000000), ("Purple", 0x7030A0))
WEIGHTS = (0.25, 0.5, 0.75, 1, 1.5, 2.25, 3, 4.5, 6)
TITLE = "Create Sparklines"


def _ref(r1, c1, r2, c2):
    return a1(r1, c1) if (r1, c1) == (r2, c2) else "%s:%s" % (a1(r1, c1), a1(r2, c2))


class CreateDialog(Gtk.Dialog):
    """Choose the data that you want: Data Range; Choose where you want the sparklines to be placed:
    Location Range."""
    def __init__(self, win, kind, data="", location=""):
        super().__init__(title=TITLE, transient_for=win, modal=True)
        self.win = win
        self.kind = kind
        self.add_button("OK", Gtk.ResponseType.OK)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.set_default_response(Gtk.ResponseType.OK)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        box.set_border_width(12)
        self.get_content_area().pack_start(box, True, True, 0)
        box.pack_start(Gtk.Label(label="Choose the data that you want", xalign=0), False, False, 0)
        self.data = self._row(box, "Data Range:", data, "data")
        box.pack_start(Gtk.Label(label="Choose where you want the sparklines to be placed", xalign=0), False, False, 6)
        self.loc = self._row(box, "Location Range:", location, "location")
        self.show_all()

    def _row(self, box, label, text, name):
        row = Gtk.Box(spacing=8)
        lab = Gtk.Label(label=label, xalign=0)
        lab.set_width_chars(15)
        e = Gtk.Entry(text=text)
        e.set_name(name)
        e.set_width_chars(28)
        e.set_activates_default(True)
        row.pack_start(lab, False, False, 0)
        row.pack_start(e, True, True, 0)
        box.pack_start(row, False, False, 0)
        return e

    def apply(self):
        """Make the sparklines; an error message, or None."""
        data = self.data.get_text().strip().lstrip("=")
        loc = self.loc.get_text().strip().lstrip("=")
        if not data:
            return "The reference for the data range isn't valid."
        if not loc:
            return "The location reference isn't valid."
        w = self.win
        try:
            w.book.add_sparklines(w.grid.sheet, self.kind, data, loc)
        except ValueError as e:
            return str(e)
        return None


def create(win, kind, data=None, location=None):
    """Insert › Sparklines › Line / Column / Win/Loss."""
    g = win.grid
    rg = tuple(g.sel.ranges[-1])
    if data is None and location is None:
        # the selected numbers are the data; an empty selection is where they go
        empty = all(win.book.input(g.sheet, r, c) == "" for r in range(rg[0], min(rg[2], rg[0] + 50) + 1)
                    for c in range(rg[1], min(rg[3], rg[1] + 50) + 1))
        if empty:
            data, location = "", _ref(*rg)
        else:
            data, location = _ref(*rg), ""
    d = CreateDialog(win, kind, data or "", location or "")
    ok = False
    while True:
        if d.run() != Gtk.ResponseType.OK:
            break
        err = d.apply()
        if err is None:
            ok = True
            break
        win.dialogs.message(err, TITLE)
    d.destroy()
    if ok:
        done(win)
    return ok


def done(win):
    win.grid.refresh()
    win.update_title()
    win.on_selection_changed()


def current(win):
    """(group index, its settings) of the active cell's sparkline, or None."""
    g = win.grid
    if win.book is None:
        return None
    at = win.book.sparkline_at(g.sheet, *g.sel.active)
    if at is None:
        return None
    return at[0], win.book.sparklines(g.sheet)[at[0]]


def set_options(win, **opts):
    cur = current(win)
    if cur is None:
        return False
    win.book.sparkline_set(win.grid.sheet, cur[0], opts)
    done(win)
    return True


def _sel_rect(win):
    g = win.grid
    r1, c1, r2, c2 = g.sel.ranges[-1]
    return min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2)


def clear(win, group=False):
    if current(win) is None:
        return False
    win.book.clear_sparklines(win.grid.sheet, *_sel_rect(win), group)
    done(win)
    return True


def regroup(win, group):
    if current(win) is None:
        return False
    win.book.regroup_sparklines(win.grid.sheet, *_sel_rect(win), group)
    done(win)
    return True


def edit_data(win, data=None, location=None):
    """Edit Group Location & Data: the group made again over the new ranges, its look kept."""
    cur = current(win)
    if cur is None:
        return False
    gi, info = cur
    items = info["items"]
    s = win.grid.sheet
    if data is None:
        refs = [f for f, _ in items]
        sheet = refs[0].rsplit("!", 1)[0] if "!" in refs[0] else ""
        rs = []
        from . import core
        for f in refs:
            x = core.parse_ref(f.rsplit("!", 1)[-1].replace("$", ""))
            if x:
                rs.append(tuple(x))
        if rs:
            box = (min(r[0] for r in rs), min(r[1] for r in rs), max(r[2] for r in rs), max(r[3] for r in rs))
            data = (sheet + "!" if sheet else "") + _ref(*box)
        else:
            data = refs[0]
        cells = [l for _, l in items]
        location = _ref(min(r for r, _ in cells), min(c for _, c in cells), max(r for r, _ in cells), max(c for _, c in cells))
        d = CreateDialog(win, info["kind"], data, location)
        d.set_title("Edit Sparklines")
        resp = d.run()
        data, location = d.data.get_text().strip().lstrip("="), d.loc.get_text().strip().lstrip("=")
        d.destroy()
        if resp != Gtk.ResponseType.OK:
            return False
    keep = {k: v for k, v in info.items() if k != "items"}
    try:
        with win.book.group("Edit Sparklines"):
            win.book.clear_sparklines(s, items[0][1][0], items[0][1][1], items[0][1][0], items[0][1][1], True)
            g2 = win.book.add_sparklines(s, keep["kind"], data, location)
            win.book.sparkline_set(s, g2, keep)
    except ValueError as e:
        win.book.undo()
        win.dialogs.message(str(e), "Edit Sparklines")
        return False
    done(win)
    return True


# ---------------------------------------------------------------- the Sparkline tab
def build_tab(win, B):
    from .ribbon import RibbonButton, Tab
    from .ribbon_defs import color_palette
    r = win.ribbon
    t = r.building(Tab("Sparkline", "JS", contextual=True))
    B.group(t, "Sparkline", [B.big("Edit\nData", "r-spark-edit-data", "spark-edit-data", "D",
                                   tip="Edit Data\nChange the data and the cells of this group of sparklines.")])
    B.group(t, "Type", [
        RibbonButton(B.r, label.replace("/", "/\n") if "/" in label else label, "r-sparkline-" + k, "spark-type:" + k,
                     size="large", state="spark:" + k, keytip=label[0] if k != "winloss" else "W",
                     tooltip="Convert to %s Sparkline" % label)
        for k, label in KINDS])
    win.spark_checks = {}
    cols = []
    for key, lab in SHOW:
        chk = Gtk.CheckButton(label=lab)
        chk.set_name("spark-" + key)
        chk.action = "spark-show:" + key
        chk.keytip = lab[0] + lab.split()[-1][0]
        chk.press = (lambda c=chk: c.set_active(not c.get_active()))
        chk.connect("toggled", lambda c, key=key: None if win.syncing_checks else win.run(
            "spark-show:%s:%d" % (key, c.get_active())))
        t.controls.append(chk)
        win.spark_checks[key] = chk
        cols.append(chk)
    B.group(t, "Show", [cols[0:3], cols[3:6]])
    ref = [None]

    def series_color():
        pop = Gtk.Popover()
        ref[0] = pop
        box = color_palette(lambda c: win.run("spark-color:%06X" % (c or 0)), popref=ref)
        ex = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        ex.set_border_width(6)
        ex.pack_start(Gtk.Label(label="Weight", xalign=0), False, False, 0)
        row = Gtk.Box(spacing=2)
        for pt in WEIGHTS:
            b = Gtk.Button(label="%g pt" % pt)
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.connect("clicked", lambda _b, pt=pt: (pop.popdown(), win.run("spark-weight:%g" % pt)))
            row.pack_start(b, False, False, 0)
        ex.pack_start(row, False, False, 0)
        box.pack_start(ex, False, False, 0)
        pop.add(box)
        return pop

    marker_menu = [(label, [(name, "spark-marker:%s:%06X" % (key, c)) for name, c in COLORS]) for key, label in MARKERS]
    B.group(t, "Style", [[B.small("Sparkline Color", "r-spark-color-sm", None, "SC", menu=series_color),
                          B.small("Marker Color", "r-marker-color-sm", None, "M", menu=marker_menu)]])
    B.group(t, "Group", [[B.small("Axis", "r-spark-axis-sm", "spark-axis", "A"),
                          B.small("Group", "r-group-sm", "spark-group", "G"),
                          B.small("Ungroup", "r-group-sm", "spark-ungroup", "U")],
                         [B.small("Clear", "r-clear-sm", None, "C", menu=[
                             ("Clear Selected Sparklines", "spark-clear"),
                             ("Clear Selected Sparkline Groups", "spark-clear-group")])]])
    r.add_tab(t)
    r.show_contextual("Sparkline", False)


def sync_tab(win):
    cur = current(win) if hasattr(win, "spark_checks") else None
    win.ribbon.show_contextual("Sparkline", cur is not None)
    if cur is None:
        return None
    info = cur[1]
    for k, _ in KINDS:
        win.ribbon.update("spark:" + k, "true" if info["kind"] == k else "false")
    win.syncing_checks = True
    try:
        for key, chk in win.spark_checks.items():
            chk.set_active(bool(info.get(key)))
    finally:
        win.syncing_checks = False
    return cur


def axis_not_available(win):
    win.dialogs.info("Sparkline axis options aren't available in %s yet. Sparklines are drawn "
                     "with an automatic axis for each sparkline." % APP_NAME, "Axis")
