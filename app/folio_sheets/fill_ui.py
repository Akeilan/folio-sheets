"""Home ▸ Fill ▸ Series."""
from gi.repository import Gtk


def _radios(labels):
    group = None
    out = []
    for lab in labels:
        rb = Gtk.RadioButton.new_with_label_from_widget(group, lab)
        group = group or rb
        out.append(rb)
    return out


def series_dialog(win):
    g = win.grid
    rg = tuple(g.sel.ranges[-1])
    d = Gtk.Dialog(title="Series", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    grid = Gtk.Grid(row_spacing=4, column_spacing=24)
    grid.set_border_width(12)
    d.get_content_area().pack_start(grid, True, True, 0)
    by = _radios(["Rows", "Columns"])
    kinds = _radios(["Linear", "Growth", "Date", "AutoFill"])
    units = _radios(["Day", "Weekday", "Month", "Year"])
    for col, (title, rbs) in enumerate((("Series in", by), ("Type", kinds), ("Date unit", units))):
        grid.attach(Gtk.Label(label=title, xalign=0), col, 0, 1, 1)
        for i, rb in enumerate(rbs):
            grid.attach(rb, col, i + 1, 1, 1)
    # rows when the selection is wider than tall
    (by[0] if (rg[3] - rg[1]) > (rg[2] - rg[0]) else by[1]).set_active(True)
    trend = Gtk.CheckButton(label="Trend")
    grid.attach(trend, 0, 5, 1, 1)
    step = Gtk.Entry(text="1")
    step.set_name("step")
    stop = Gtk.Entry()
    stop.set_name("stop")
    step.set_activates_default(True)
    grid.attach(Gtk.Label(label="Step value:", xalign=0), 0, 6, 1, 1)
    grid.attach(step, 1, 6, 1, 1)
    grid.attach(Gtk.Label(label="Stop value:", xalign=0), 2, 6, 1, 1)
    grid.attach(stop, 2, 7, 1, 1)

    def sync(*_):
        date = kinds[2].get_active()
        for u in units:
            u.set_sensitive(date)
        step.set_sensitive(not kinds[3].get_active())
    for k in kinds:
        k.connect("toggled", sync)
    sync()
    d.show_all()
    while True:
        r = d.run()
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        try:
            st = float(step.get_text() or "1")
            sp = float(stop.get_text()) if stop.get_text().strip() else None
        except ValueError:
            win.dialogs.message("Type a number for the step and stop values.", "Series")
            continue
        break
    kind = ["linear", "growth", "date", "autofill"][[k.get_active() for k in kinds].index(True)]
    unit = ["day", "weekday", "month", "year"][[u.get_active() for u in units].index(True)]
    by_rows = by[0].get_active()
    tr = trend.get_active()
    d.destroy()
    try:
        win.book.fill_series(g.sheet, rg, by_rows, kind, unit, st, sp, tr)
    except ValueError as e:
        win.dialogs.message(str(e), "Series")
        return False
    g.refresh()
    win.update_title()
    return True
