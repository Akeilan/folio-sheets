"""Waterfall, Funnel, Histogram, Pareto, Box and Whisker, Treemap and Sunburst charts:
inserting them, changing their kind, data and title."""
from gi.repository import Gtk

from .core import parse_ref, range_a1

KINDS = [("Waterfall", "waterfall"), ("Funnel", "funnel"), ("Histogram", "histogram"), ("Pareto", "pareto"),
         ("Box and Whisker", "boxwhisker"), ("Treemap", "treemap"), ("Sunburst", "sunburst")]
LABEL = dict((k, lab) for lab, k in KINDS)
GROUPS = {"hierarchy": ["treemap", "sunburst"], "statistic": ["histogram", "pareto", "boxwhisker"],
          "waterfall": ["waterfall", "funnel"]}


def _range(win):
    g = win.grid
    rg = tuple(g.sel.ranges[-1])
    if rg[0:2] == rg[2:4]:
        rg = tuple(win.book.current_region(g.sheet, rg[0], rg[1]))
    return rg


def insert(win, kind, rg=None):
    from .chart_ui import _visible_center
    g, b = win.grid, win.book
    rg = rg or _range(win)
    x, y = _visible_center(win)
    try:
        i = b.insert_chartex(g.sheet, rg, kind, x, y)
    except ValueError as e:
        win.dialogs.message(str(e), "Insert Chart")
        return None
    g.sel_objects = [i]
    g.refresh()
    win.update_title()
    win.on_object_selection()
    return i


class KindDialog:
    """All of the newer charts with a preview: insert one, or change a chart's kind."""

    def __init__(self, win, kinds=None, current=None, rg=None, title="Insert Chart"):
        self.win, self.rg = win, rg
        self.kind = current or (kinds or [k for _, k in KINDS])[0]
        d = self.d = Gtk.Dialog(title=title, transient_for=win, modal=True)
        d.get_style_context().add_class("folio-dialog")
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_size(700, 420)
        box = Gtk.Box(spacing=12)
        box.set_border_width(12)
        d.get_content_area().pack_start(box, True, True, 0)
        side = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        group = None
        self.radios = {}
        for lab, k in KINDS:
            if kinds and k not in kinds:
                continue
            rb = Gtk.RadioButton.new_with_label_from_widget(group, lab)
            group = group or rb
            rb.set_active(k == self.kind)
            rb.connect("toggled", lambda w, k=k: (w.get_active() and self._pick(k)))
            self.radios[k] = rb
            side.pack_start(rb, False, False, 0)
        box.pack_start(side, False, False, 0)
        self.preview = Gtk.DrawingArea()
        self.preview.set_size_request(480, 288)
        self.preview.connect("draw", self._draw)
        box.pack_start(self.preview, True, True, 0)
        d.show_all()

    def _pick(self, k):
        self.kind = k
        self.preview.queue_draw()

    def _draw(self, w, cr):
        if self.rg is None:
            return
        a = w.get_allocation()
        try:
            self.win.book.render_chartex_preview(self.win.grid.sheet, cr, float(a.width), float(a.height), self.rg, self.kind)
        except ValueError:
            pass

    def run(self):
        r = self.d.run()
        k = self.kind
        self.d.destroy()
        return k if r == Gtk.ResponseType.OK else None


def more(win, group):
    rg = _range(win)
    k = KindDialog(win, GROUPS.get(group), rg=rg).run()
    return insert(win, k, rg) if k else None


def change_type(win, i):
    b, g = win.book, win.grid
    info = b.chartex_info(g.sheet, i)
    rg = None
    if info and info["values"]:
        rg = _source_rect(info)
    k = KindDialog(win, current=info["kind"], rg=rg, title="Change Chart Type").run()
    if not k:
        return False
    b.set_chartex(g.sheet, i, kind=k)
    g.refresh()
    win.update_title()
    return True


def _source_rect(info):
    refs = info["cats"] + info["values"]
    rects = [parse_ref(f.rsplit("!", 1)[-1].replace("$", "")) for f in refs]
    rects = [r for r in rects if r]
    if not rects:
        return None
    r1 = min(r[0] for r in rects) - 1
    return (max(0, r1), min(r[1] for r in rects), max(r[2] for r in rects), max(r[3] for r in rects))


def select_data(win, i):
    b, g = win.book, win.grid
    info = b.chartex_info(g.sheet, i)
    cur = _source_rect(info)
    t = win.dialogs.ask_text("Select Data Source", "Chart data range:", range_a1(cur) if cur else "")
    if not t:
        return False
    rg = parse_ref(t.rsplit("!", 1)[-1].replace("$", "").lstrip("="))
    if rg is None:
        win.dialogs.message("Reference isn't valid.", "Select Data Source")
        return False
    try:
        b.set_chartex(g.sheet, i, rg=tuple(rg))
    except ValueError as e:
        win.dialogs.message(str(e), "Select Data Source")
        return False
    g.refresh()
    win.update_title()
    return True


def edit_title(win, i):
    b, g = win.book, win.grid
    info = b.chartex_info(g.sheet, i)
    t = win.dialogs.ask_text("Chart Title", "Title (empty: the series name):", info["own_title"] or "")
    if t is None:
        return False
    if t.strip():
        b.set_chartex(g.sheet, i, title=t)
    else:
        b.set_chartex(g.sheet, i, clear_title=True)
    g.refresh()
    win.update_title()
    return True
