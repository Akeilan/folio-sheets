"""View > Window: New Window on the same workbook, Arrange All, View Side by Side with
Synchronous Scrolling, Reset Window Position, Hide and Unhide, Switch Windows."""
from gi.repository import Gdk, GLib, Gtk

ARRANGE = [("Tiled", "tiled"), ("Horizontal", "horizontal"), ("Vertical", "vertical"), ("Cascade", "cascade")]


def views(win):
    """The windows showing the same workbook, in the order they were opened."""
    return [w for w in win.app.windows if w.book is win.book]


def view_number(win):
    vs = views(win)
    return vs.index(win) + 1 if len(vs) > 1 and win in vs else 0


def new_window(win):
    """A second window on the same workbook (Book1:2), at the same place."""
    if win.grid.editing() and win.grid.commit_edit() is False:
        return None
    from .window import SheetWindow
    w = SheetWindow(win.app, share=win)
    win.app.windows.append(w)
    w.present()
    for v in views(win):
        v.update_title()
    return w


def watch(win):
    """Windows sharing a book see each other's edits (polled: cheap, and nothing to wire)."""
    win._seen = (win.book.version, win.book.sheet_count)

    def tick():
        if win.get_window() is None or win not in win.app.windows:
            return False
        if len(views(win)) < 2:
            win._seen = (win.book.version, win.book.sheet_count)
            return True
        now = (win.book.version, win.book.sheet_count)
        if now != win._seen:
            sheets_changed = now[1] != win._seen[1]
            win._seen = now
            g = win.grid
            if g.sheet >= win.book.sheet_count:
                g.sheet = win.book.sheet_count - 1
            if sheets_changed:
                win.tabs.rebuild()
            g.area.queue_draw()
            g.update_scrollbars()
            if not g.editing():
                win.on_selection_changed()
            win.update_title()
        return True
    GLib.timeout_add(200, tick)


def book_switched(win):
    """Undo crossed an engine Book swap: every window on the workbook follows."""
    for v in views(win):
        v._book_switched()


# ------------------------------------------------------------------ arranging
def _workarea(win):
    disp = Gdk.Display.get_default()
    gw = win.get_window()
    mon = disp.get_monitor_at_window(gw) if gw is not None else disp.get_primary_monitor() or disp.get_monitor(0)
    wa = mon.get_workarea()
    return wa.x, wa.y, wa.width, wa.height


def place(w, x, y, width, height):
    if w.is_maximized():
        w.unmaximize()
    w.move(int(x), int(y))
    w.resize(max(int(width), 320), max(int(height), 200))
    w.arranged = (int(x), int(y), max(int(width), 320), max(int(height), 200))


def arrange(win, how, only_this_book=False):
    ws = [w for w in (views(win) if only_this_book else win.app.windows) if w.get_visible()]
    if not ws:
        return []
    # the active window first
    ws.sort(key=lambda w: 0 if w is win else 1)
    x0, y0, W, H = _workarea(win)
    n = len(ws)
    if how == "horizontal":
        h = H // n
        for i, w in enumerate(ws):
            place(w, x0, y0 + i * h, W, h)
    elif how == "vertical":
        wd = W // n
        for i, w in enumerate(ws):
            place(w, x0 + i * wd, y0, wd, H)
    elif how == "cascade":
        step = 28
        for i, w in enumerate(ws):
            place(w, x0 + i * step, y0 + i * step, W - (n - 1) * step, H - (n - 1) * step)
    else:
        cols = 1
        while cols * cols < n:
            cols += 1
        rows = (n + cols - 1) // cols
        wd, h = W // cols, H // rows
        for i, w in enumerate(ws):
            place(w, x0 + (i % cols) * wd, y0 + (i // cols) * h, wd, h)
    return ws


def arrange_dialog(win):
    d = Gtk.Dialog(title="Arrange Windows", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(4)
    box.pack_start(Gtk.Label(label="Arrange", xalign=0), False, False, 0)
    group = None
    radios = []
    last = win.app.settings.get("arrange", "tiled")
    for label, key in ARRANGE:
        rb = Gtk.RadioButton.new_with_label_from_widget(group, label)
        group = group or rb
        rb.set_active(key == last)
        radios.append((rb, key))
        box.pack_start(rb, False, False, 0)
    only = Gtk.CheckButton(label="Windows of active workbook")
    box.pack_start(only, False, False, 6)
    d.radios, d.only = radios, only
    d.show_all()
    r = d.run()
    how = next(k for rb, k in radios if rb.get_active())
    mine = only.get_active()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return None
    win.app.settings["arrange"] = how
    return arrange(win, how, mine)


# ------------------------------------------------------------------ side by side
def side_by_side(win):
    """Two windows one above the other, scrolling together. A second press ends it."""
    if win.side_partner is not None:
        end_side_by_side(win)
        return None
    others = [w for w in win.app.windows if w is not win and w.get_visible()]
    if not others:
        win.dialogs.message("Open another workbook or a new window on this one to compare side by side.")
        return None
    other = others[0]
    if len(others) > 1:
        other = _pick_window(win, others)
        if other is None:
            return None
    win.side_partner, other.side_partner = other, win
    win.sync_scroll = other.sync_scroll = True
    reset_position(win)
    for w in (win, other):
        w.ribbon.update("side-by-side", "true")
        w.ribbon.update("sync-scroll", "true")
        w.grid._scroll_seen = None
    return other


def _pick_window(win, others):
    d = Gtk.Dialog(title="Compare Side by Side", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    box.pack_start(Gtk.Label(label="Compare Side by Side with:", xalign=0), False, False, 4)
    lb = Gtk.ListBox()
    for w in others:
        row = Gtk.ListBoxRow()
        row.add(Gtk.Label(label=w.doc_title(), xalign=0))
        row.win = w
        lb.add(row)
    lb.select_row(lb.get_row_at_index(0))
    box.pack_start(lb, True, True, 0)
    d.list = lb
    d.show_all()
    r = d.run()
    row = lb.get_selected_row()
    d.destroy()
    return row.win if r == Gtk.ResponseType.OK and row is not None else None


def end_side_by_side(win):
    other = win.side_partner
    for w in (win, other):
        if w is None:
            continue
        w.side_partner = None
        w.sync_scroll = False
        w.ribbon.update("side-by-side", "false")
        w.ribbon.update("sync-scroll", "false")


def toggle_sync(win):
    if win.side_partner is None:
        return
    on = not win.sync_scroll
    for w in (win, win.side_partner):
        w.sync_scroll = on
        w.ribbon.update("sync-scroll", "true" if on else "false")
        w.grid._scroll_seen = None


def reset_position(win):
    """Side by side: the two windows share the screen equally again."""
    other = win.side_partner
    if other is None:
        return
    x0, y0, W, H = _workarea(win)
    h = H // 2
    place(win, x0, y0, W, h)
    place(other, x0, y0 + h, W, H - h)


def scrolled(win):
    """Called as the grid draws: a scroll in one window moves its partner as far."""
    g = win.grid
    st = g.state()
    now = (g.sheet, st.top_row, st.left_col)
    seen, g._scroll_seen = getattr(g, "_scroll_seen", None), now
    other = win.side_partner
    if not win.sync_scroll or other is None or seen is None or seen[0] != now[0]:
        return
    dr, dc = now[1] - seen[1], now[2] - seen[2]
    if not dr and not dc:
        return
    og = other.grid
    ost = og.state()
    ost.top_row = max(ost.freeze_rows, ost.top_row + dr)
    ost.left_col = max(ost.freeze_cols, ost.left_col + dc)
    og._scroll_seen = (og.sheet, ost.top_row, ost.left_col)
    og.area.queue_draw()
    og.update_scrollbars()


# ------------------------------------------------------------------ hide / unhide
def hide(win):
    shown = [w for w in win.app.windows if w.get_visible()]
    if len(shown) <= 1:
        win.dialogs.message("This is the only window that is showing; it can't be hidden.")
        return False
    if win.side_partner is not None:
        end_side_by_side(win)
    win.hide()
    nxt = next(w for w in shown if w is not win)
    nxt.present()
    return True


def unhide(win):
    hidden = [w for w in win.app.windows if not w.get_visible()]
    if not hidden:
        win.dialogs.message("There are no hidden windows.")
        return None
    d = Gtk.Dialog(title="Unhide", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    box = d.get_content_area()
    box.set_border_width(12)
    box.pack_start(Gtk.Label(label="Unhide workbook:", xalign=0), False, False, 4)
    lb = Gtk.ListBox()
    for w in hidden:
        row = Gtk.ListBoxRow()
        row.add(Gtk.Label(label=w.doc_title(), xalign=0))
        row.win = w
        lb.add(row)
    lb.select_row(lb.get_row_at_index(0))
    box.pack_start(lb, True, True, 0)
    d.list = lb
    d.show_all()
    r = d.run()
    row = lb.get_selected_row()
    d.destroy()
    if r != Gtk.ResponseType.OK or row is None:
        return None
    row.win.show()
    row.win.present()
    return row.win


def switch_menu(win):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    box.set_border_width(4)
    for i, w in enumerate([w for w in win.app.windows if w.get_visible()], start=1):
        b = Gtk.ModelButton(text="%d %s" % (i, w.doc_title()))
        b.set_property("role", Gtk.ButtonRole.CHECK)
        b.set_property("active", w is win)
        b.connect("clicked", lambda _b, w=w: w.present())
        box.pack_start(b, False, False, 0)
    return box

