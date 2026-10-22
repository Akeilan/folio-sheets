"""Notes and threaded comments: the note editor over the grid, the comment card, the
Comments pane, and the Review tab's commands."""
from gi.repository import Gdk, GLib, Gtk, Pango

from .core import a1


def author():
    """The name notes and comments are signed with (the computer's user)."""
    name = GLib.get_real_name() or ""
    if not name or name == "Unknown":
        name = GLib.get_user_name() or "Author"
    return name


def _active(win):
    return win.grid.sel.active


# ------------------------------------------------------------------ notes
class NoteEditor(Gtk.Frame):
    """A yellow box typed into in place, like the note it becomes."""

    def __init__(self, win, r, c):
        super().__init__()
        self.win, self.r, self.c = win, r, c
        self.get_style_context().add_class("folio-note")
        self.view = Gtk.TextView()
        self.view.set_name("note-text")
        self.view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self.view.set_left_margin(4)
        self.view.set_right_margin(4)
        self.view.set_top_margin(3)
        css = Gtk.CssProvider()
        css.load_from_data(b"textview, textview text { background-color: #ffffe1; color: #000; font-size: 9pt; } frame { border: 1px solid #000; }")
        for w in (self, self.view):
            w.get_style_context().add_provider(css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self.add(self.view)
        self.view.connect("key-press-event", self.on_key)
        self.view.connect("focus-out-event", lambda *_: (GLib.idle_add(self.finish), False)[1])
        self.done = False

    def on_key(self, w, ev):
        if ev.keyval == Gdk.KEY_Escape:
            self.finish()
            return True
        return False

    def text(self):
        buf = self.view.get_buffer()
        return buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False)

    def finish(self):
        if self.done:
            return False
        self.done = True
        g = self.win.grid
        b = self.win.book
        t = self.text()
        try:
            b.set_note(g.sheet, self.r, self.c, author(), t)
        except ValueError as e:
            self.win.dialogs.message(str(e))
        g.edit_note = None
        g.fixed.remove(self)
        g.area.grab_focus()
        g.refresh()
        self.win.update_title()
        self.win.note_editor = None
        return False


def edit_note(win, r=None, c=None):
    g = win.grid
    b = win.book
    if r is None:
        r, c = _active(win)
    if b.thread(g.sheet, r, c):
        win.dialogs.message("This cell has a comment. A cell can have a comment or a note, not both.", "New Note")
        return None
    if getattr(win, "note_editor", None) is not None:
        win.note_editor.finish()
    n = b.note(g.sheet, r, c)
    g.ensure_visible(r, c)
    ed = NoteEditor(win, r, c)
    text = n["text"] if n else "%s:\n" % author()
    ed.view.get_buffer().set_text(text)
    x, y, w, h = b.note_box(g.sheet, r, c, g.params())
    ed.set_size_request(int(w), int(h))
    g.edit_note = (r, c)
    g.fixed.put(ed, int(x), int(y))
    ed.show_all()
    ed.view.grab_focus()
    buf = ed.view.get_buffer()
    buf.place_cursor(buf.get_end_iter())
    win.note_editor = ed
    g.refresh()
    return ed


def delete(win):
    g = win.grid
    win.book.clear_comments(g.sheet, [tuple(x) for x in g.sel.ranges])
    g.refresh()
    win.update_title()


def show_hide(win):
    g = win.grid
    cells = []
    for r1, c1, r2, c2 in g.sel.ranges:
        for n in win.book.notes(g.sheet):
            r, c = n["cell"]
            if r1 <= r <= r2 and c1 <= c <= c2:
                cells.append((r, c))
    if not cells:
        win.status.extra.set_text("There's no note in the selection.")
        return
    win.book.show_notes(g.sheet, cells, None)
    g.refresh()
    win.update_title()


def show_all(win):
    win.book.show_notes(win.grid.sheet, None, None)
    win.grid.refresh()
    win.update_title()


def go(win, back, notes):
    g = win.grid
    r, c = _active(win)
    nxt = win.book.next_comment(g.sheet, r, c, back, notes)
    if nxt is None:
        win.status.extra.set_text("There are no %s on this sheet." % ("notes" if notes else "comments"))
        return
    g.select_cell(*nxt)
    g.ensure_visible(*nxt)
    if not notes:
        open_card(win, *nxt)


def convert(win):
    n = win.book.notes_to_comments(win.grid.sheet)
    win.status.extra.set_text("%d notes converted to comments." % n)
    win.grid.refresh()
    win.update_title()
    refresh_pane(win)


# ------------------------------------------------------------------ comments
def _local(date):
    """A comment's time (kept in UTC) in local time."""
    if not date:
        return ""
    import datetime
    try:
        d = datetime.datetime.fromisoformat(date[:19]).replace(tzinfo=datetime.timezone.utc).astimezone()
        return d.strftime("%Y-%m-%d %H:%M")
    except ValueError:
        return date.replace("T", " ")[:16]


def _entry_box(name, date, text):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    head = Gtk.Label(xalign=0)
    when = _local(date)
    head.set_markup("<b>%s</b>  <small>%s</small>" % (GLib.markup_escape_text(name), GLib.markup_escape_text(when)))
    box.pack_start(head, False, False, 0)
    body = Gtk.Label(label=text, xalign=0)
    body.set_line_wrap(True)
    body.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
    body.set_max_width_chars(40)
    body.set_selectable(False)
    box.pack_start(body, False, False, 0)
    return box


def card(win, r, c, on_done=None):
    """A thread's card: its entries, a reply box, Resolve and Delete."""
    b = win.book
    s = win.grid.sheet
    t = b.thread(s, r, c)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    box.set_border_width(10)
    box.set_size_request(300, -1)
    title = Gtk.Label(xalign=0)
    title.set_markup("<small>%s%s</small>" % (a1(r, c), "  (resolved)" if t and t["resolved"] else ""))
    box.pack_start(title, False, False, 0)
    if t:
        for name, date, text in t["entries"]:
            box.pack_start(_entry_box(name, date, text), False, False, 0)
            box.pack_start(Gtk.Separator(), False, False, 0)
    view = Gtk.TextView()
    view.set_name("comment-text")
    view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
    view.set_size_request(-1, 54)
    frame = Gtk.Frame()
    frame.add(view)
    box.pack_start(frame, False, False, 0)
    row = Gtk.Box(spacing=6)
    post = Gtk.Button(label="Reply" if t else "Post")
    post.set_name("post")
    post.get_style_context().add_class("folio-default")
    row.pack_end(post, False, False, 0)
    if t:
        res = Gtk.Button(label="Reopen" if t["resolved"] else "Resolve")
        res.set_name("resolve")
        row.pack_start(res, False, False, 0)
        dele = Gtk.Button(label="Delete Thread")
        dele.set_name("delete")
        row.pack_start(dele, False, False, 0)

        def on_res(_b):
            b.resolve_comment(s, r, c, not t["resolved"])
            _after(win, on_done)
        res.connect("clicked", on_res)

        def on_del(_b):
            b.edit_comment(s, r, c, 0, None)
            _after(win, on_done)
        dele.connect("clicked", on_del)
    box.pack_start(row, False, False, 0)

    def on_post(_b):
        buf = view.get_buffer()
        text = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), False).strip()
        if not text:
            return
        try:
            b.add_comment(s, r, c, author(), text)
        except ValueError as e:
            win.dialogs.message(str(e), "Comment")
            return
        _after(win, on_done)
    post.connect("clicked", on_post)

    def on_key(w, ev):
        if ev.keyval == Gdk.KEY_Return and ev.state & Gdk.ModifierType.CONTROL_MASK:
            on_post(None)
            return True
        return False
    view.connect("key-press-event", on_key)
    box.view = view
    box.post = post
    box.show_all()
    return box


def _after(win, on_done):
    win.grid.refresh()
    win.update_title()
    refresh_pane(win)
    if on_done:
        on_done()


def open_card(win, r=None, c=None, focus=True):
    """New Comment / the card of the active cell's thread, beside the cell."""
    g = win.grid
    b = win.book
    if r is None:
        r, c = _active(win)
    if b.note(g.sheet, r, c):
        win.dialogs.message("This cell has a note. A cell can have a note or a comment, not both.", "New Comment")
        return None
    close_card(win)
    g.ensure_visible(r, c)
    rect = b.cell_rect(g.sheet, r, c, g.params())
    pop = Gtk.Popover()
    pop.set_relative_to(g.area)
    if rect:
        rr = Gdk.Rectangle()
        rr.x, rr.y, rr.width, rr.height = int(rect[0] + rect[2]), int(rect[1]), 1, int(rect[3])
        pop.set_pointing_to(rr)
    pop.set_position(Gtk.PositionType.RIGHT)
    box = card(win, r, c, on_done=lambda: close_card(win))
    pop.add(box)
    pop.card = box
    pop.cell = (r, c)
    pop.show_all()
    pop.popup()
    if focus:
        box.view.grab_focus()
    win.comment_card = pop
    return pop


def close_card(win):
    pop = getattr(win, "comment_card", None)
    if pop is not None:
        pop.popdown()
        pop.destroy()
        win.comment_card = None


# ------------------------------------------------------------------ the Comments pane
def toggle_pane(win):
    pane = getattr(win, "comments_pane", None)
    if pane is None:
        return
    pane.set_visible(not pane.get_visible())
    refresh_pane(win)


def make_pane(win):
    outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    outer.set_size_request(320, -1)
    outer.set_name("comments-pane")
    head = Gtk.Box(spacing=6)
    head.set_border_width(6)
    lab = Gtk.Label(xalign=0)
    lab.set_markup("<b>Comments</b>")
    head.pack_start(lab, True, True, 0)
    new = Gtk.Button(label="New")
    new.connect("clicked", lambda _b: open_card(win))
    head.pack_end(new, False, False, 0)
    close = Gtk.Button.new_from_icon_name("window-close-symbolic", Gtk.IconSize.MENU)
    close.set_relief(Gtk.ReliefStyle.NONE)
    close.connect("clicked", lambda _b: outer.hide())
    head.pack_end(close, False, False, 0)
    outer.pack_start(head, False, False, 0)
    sw = Gtk.ScrolledWindow()
    sw.set_vexpand(True)
    sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    outer.list = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    outer.list.set_border_width(6)
    sw.add(outer.list)
    outer.pack_start(sw, True, True, 0)
    outer.set_no_show_all(True)
    head.show_all()
    sw.show_all()
    return outer


def refresh_pane(win):
    pane = getattr(win, "comments_pane", None)
    if pane is None or not pane.get_visible():
        return
    for ch in pane.list.get_children():
        pane.list.remove(ch)
    b = win.book
    s = win.grid.sheet
    cells = b.threads(s)
    if not cells:
        lab = Gtk.Label(label="There are no comments on this sheet.", xalign=0)
        pane.list.pack_start(lab, False, False, 0)
    for (r, c) in cells:
        fr = Gtk.Frame()
        fr.get_style_context().add_class("folio-comment-card")
        box = card(win, r, c)
        ev = Gtk.EventBox()
        ev.add(box)
        ev.connect("button-press-event", lambda _w, _e, r=r, c=c: (win.grid.select_cell(r, c), win.grid.ensure_visible(r, c), False)[2])
        fr.add(ev)
        pane.list.pack_start(fr, False, False, 0)
    pane.list.show_all()
