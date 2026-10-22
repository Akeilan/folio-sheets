"""The Name Box and the formula bar."""
from gi.repository import Gdk, GLib, Gtk

from . import icons
from .core import parse_ref, range_a1


class NameBox(Gtk.ComboBoxText):
    """Shows the active cell (or the size of a selection being dragged);
    typing a reference or a name and pressing Enter goes there."""
    def __init__(self, win):
        Gtk.ComboBoxText.__init__(self, has_entry=True)
        self.win = win
        self.get_style_context().add_class("folio-namebox")
        self.entry = self.get_child()
        self.entry.set_width_chars(10)
        self.entry.connect("activate", self.on_activate)
        self.entry.connect("key-press-event", self.on_key)
        self.entry.connect("focus-in-event", lambda *_: GLib.idle_add(self.entry.select_region, 0, -1))
        self.connect("changed", self.on_changed)
        self.connect("notify::popup-shown", self.on_popup)
        self._quiet = False

    def show_text(self, text):
        self._quiet = True
        self.entry.set_text(text)
        self._quiet = False

    def on_popup(self, *_):
        if not self.get_property("popup-shown"):
            return
        self._quiet = True
        self.remove_all()
        for name, ref, local, hidden in self.win.book.names():
            if not hidden and not name.startswith("_xlnm"):
                self.append_text(name)
        self._quiet = False

    def on_changed(self, combo):
        if self._quiet or self.get_active() < 0:
            return
        self.goto(self.get_active_text())

    def on_key(self, w, ev):
        if ev.keyval == Gdk.KEY_Escape:
            self.win.grid.area.grab_focus()
            self.win.on_selection_changed()
            return True
        return False

    def on_activate(self, entry):
        self.goto(entry.get_text())

    def goto(self, text):
        text = (text or "").strip()
        if not text:
            return
        if not self.win.goto_reference(text):
            self.win.dialogs.message("Reference isn't valid.")
            self.win.on_selection_changed()
            return
        self.win.grid.area.grab_focus()


class FormulaBar(Gtk.Box):
    def __init__(self, win):
        Gtk.Box.__init__(self, spacing=0)
        self.win = win
        self.get_style_context().add_class("folio-formulabar")
        self.set_valign(Gtk.Align.START)
        self.namebox = NameBox(win)
        self.namebox.set_valign(Gtk.Align.CENTER)
        self.pack_start(self.namebox, False, False, 0)
        grip = Gtk.Label(label="⋮")
        grip.get_style_context().add_class("dim-label")
        grip.set_margin_start(4)
        grip.set_margin_end(4)
        self.pack_start(grip, False, False, 0)
        self.cancel_btn = Gtk.Button(label="✕")
        self.enter_btn = Gtk.Button(label="✓")
        self.fx_btn = Gtk.Button()
        fx = Gtk.Label()
        fx.set_markup("<i>fx</i>")
        self.fx_btn.add(fx)
        for b, tip in ((self.cancel_btn, "Cancel"), (self.enter_btn, "Enter"), (self.fx_btn, "Insert Function")):
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_focus_on_click(False)
            b.set_tooltip_text(tip)
            b.get_style_context().add_class("folio-fbtn")
            self.pack_start(b, False, False, 0)
        self.cancel_btn.connect("clicked", lambda *_: self.win.grid.cancel_edit())
        self.enter_btn.connect("clicked", lambda *_: self.win.grid.commit_edit())
        self.fx_btn.connect("clicked", lambda *_: self.win.run("insert-function"))
        self.cancel_btn.set_sensitive(False)
        self.enter_btn.set_sensitive(False)
        self.view = Gtk.TextView()
        self.view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self.view.get_style_context().add_class("folio-formula")
        self.view.set_left_margin(4)
        self.view.set_top_margin(1)
        self.view.set_bottom_margin(0)
        self.view.set_accepts_tab(False)
        self.view.connect("key-press-event", self.on_key)
        self.view.connect("focus-in-event", self.on_focus)
        self.buf = self.view.get_buffer()
        self.buf.connect("changed", self.on_buf_changed)
        self.scroll = Gtk.ScrolledWindow()
        self.scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.EXTERNAL)
        self.scroll.add(self.view)
        self.scroll.set_propagate_natural_height(False)
        self.scroll.set_min_content_height(22)
        self.scroll.set_max_content_height(22)
        self.scroll.set_valign(Gtk.Align.CENTER)
        self.pack_start(self.scroll, True, True, 0)
        self.expanded = False
        self.expand_btn = Gtk.Button()
        self.expand_btn.add(icons.image("chevron", 12))
        self.expand_btn.set_relief(Gtk.ReliefStyle.NONE)
        self.expand_btn.set_tooltip_text("Expand Formula Bar (Ctrl+Shift+U)")
        self.expand_btn.connect("clicked", lambda *_: self.toggle_expand())
        self.pack_start(self.expand_btn, False, False, 0)
        self._quiet = False

    def toggle_expand(self):
        self.expanded = not self.expanded
        h = 66 if self.expanded else 22
        self.scroll.set_policy(Gtk.PolicyType.NEVER,
                               Gtk.PolicyType.AUTOMATIC if self.expanded else Gtk.PolicyType.EXTERNAL)
        if self.expanded:
            self.scroll.set_max_content_height(h)
            self.scroll.set_min_content_height(h)
        else:
            self.scroll.set_min_content_height(h)
            self.scroll.set_max_content_height(h)
        self.expand_btn.set_tooltip_text("Collapse Formula Bar (Ctrl+Shift+U)" if self.expanded
                                         else "Expand Formula Bar (Ctrl+Shift+U)")

    def text(self):
        return self.buf.get_text(self.buf.get_start_iter(), self.buf.get_end_iter(), True)

    def show_text(self, t):
        self._quiet = True
        self.buf.set_text(t)
        self._quiet = False

    def set_editing(self, on):
        self.cancel_btn.set_sensitive(on)
        self.enter_btn.set_sensitive(on)

    def on_focus(self, *_):
        g = self.win.grid
        if not g.editing():
            # editing in the bar: the cell shows the text too
            g.start_edit(mode="edit")
            GLib.idle_add(self.view.grab_focus)
        return False

    def on_buf_changed(self, buf):
        if self._quiet:
            return
        g = self.win.grid
        if g.editing():
            g.editor.set_text(self.text(), quiet=True)
            g.place_editor()

    def on_key(self, w, ev):
        g = self.win.grid
        state = ev.get_state()
        shift = bool(state & Gdk.ModifierType.SHIFT_MASK)
        alt = bool(state & Gdk.ModifierType.MOD1_MASK)
        if ev.keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            if alt:
                self.buf.insert_at_cursor("\n")
                return True
            g.editor.set_text(self.text(), quiet=True)
            g.commit_edit(move=(-1 if shift else 1, 0))
            return True
        if ev.keyval in (Gdk.KEY_Tab, Gdk.KEY_ISO_Left_Tab):
            g.editor.set_text(self.text(), quiet=True)
            g.commit_edit(move=(0, -1 if shift else 1))
            return True
        if ev.keyval == Gdk.KEY_Escape:
            g.cancel_edit()
            self.win.on_selection_changed()
            return True
        return False


def reference_of(text):
    """A typed Name Box reference as a range tuple, or None."""
    return parse_ref(text.replace("$", ""))


__all__ = ["FormulaBar", "NameBox", "reference_of", "range_a1"]
