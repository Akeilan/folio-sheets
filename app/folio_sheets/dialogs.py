"""Small dialogs: messages, Go To, Zoom, rename prompts."""
from gi.repository import Gtk

from . import APP_NAME


class Dialogs:
    def __init__(self, win):
        self.win = win

    def _dialog(self, title):
        d = Gtk.Dialog(title=title, transient_for=self.win, modal=True)
        d.get_style_context().add_class("folio-dialog")
        d.set_resizable(False)
        return d

    def message(self, text, title=APP_NAME):
        d = Gtk.MessageDialog(transient_for=self.win, modal=True, message_type=Gtk.MessageType.WARNING,
                              buttons=Gtk.ButtonsType.OK, text=text)
        d.set_title(title)
        d.run()
        d.destroy()

    def info(self, text, title=APP_NAME):
        d = Gtk.MessageDialog(transient_for=self.win, modal=True, message_type=Gtk.MessageType.INFO,
                              buttons=Gtk.ButtonsType.OK, text=text)
        d.set_title(title)
        d.run()
        d.destroy()

    def confirm_save(self, name):
        """Save / Don't Save / Cancel → 'save' | 'discard' | 'cancel'."""
        d = Gtk.MessageDialog(transient_for=self.win, modal=True, message_type=Gtk.MessageType.QUESTION,
                              text="Want to save your changes to '%s'?" % name)
        d.set_title(APP_NAME)
        d.add_button("Save", 1)
        d.add_button("Don't Save", 2)
        d.add_button("Cancel", 3)
        d.set_default_response(1)
        r = d.run()
        d.destroy()
        return {1: "save", 2: "discard"}.get(r, "cancel")

    def ask_text(self, title, label, value=""):
        d = self._dialog(title)
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_response(Gtk.ResponseType.OK)
        box = d.get_content_area()
        box.set_border_width(10)
        box.set_spacing(6)
        box.pack_start(Gtk.Label(label=label, xalign=0), False, False, 0)
        e = Gtk.Entry()
        e.set_text(value)
        e.set_activates_default(True)
        box.pack_start(e, False, False, 0)
        d.show_all()
        r = d.run()
        t = e.get_text()
        d.destroy()
        return t if r == Gtk.ResponseType.OK else None

    def goto(self):
        """Go To (F5 / Ctrl+G): names and earlier places, or a typed reference."""
        d = self._dialog("Go To")
        d.add_button("Special...", 5)
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_response(Gtk.ResponseType.OK)
        box = d.get_content_area()
        box.set_border_width(10)
        box.set_spacing(6)
        box.pack_start(Gtk.Label(label="Go to:", xalign=0), False, False, 0)
        store = Gtk.ListStore(str)
        for item in self.win.goto_history[::-1]:
            store.append([item])
        for name, ref, local, hidden in self.win.book.names():
            if not hidden and not name.startswith("_xlnm"):
                store.append([name])
        tv = Gtk.TreeView(model=store)
        tv.set_headers_visible(False)
        tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(260, 160)
        sw.add(tv)
        box.pack_start(sw, True, True, 0)
        box.pack_start(Gtk.Label(label="Reference:", xalign=0), False, False, 0)
        e = Gtk.Entry()
        e.set_activates_default(True)
        box.pack_start(e, False, False, 0)
        tv.get_selection().connect("changed", lambda s: e.set_text(
            s.get_selected()[0][s.get_selected()[1]][0]) if s.get_selected()[1] else None)
        tv.connect("row-activated", lambda *_: d.response(Gtk.ResponseType.OK))
        d.show_all()
        e.grab_focus()
        r = d.run()
        t = e.get_text().strip()
        d.destroy()
        if r == 5:
            self.win.run("goto-special")
            return
        if r == Gtk.ResponseType.OK and t:
            if not self.win.goto_reference(t):
                self.message("Reference isn't valid.")

    def zoom(self):
        d = self._dialog("Zoom")
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_response(Gtk.ResponseType.OK)
        box = d.get_content_area()
        box.set_border_width(10)
        box.set_spacing(4)
        frame = Gtk.Frame(label="Magnification")
        inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        inner.set_border_width(8)
        frame.add(inner)
        cur = round(self.win.grid.state().zoom * 100)
        group = None
        radios = {}
        for v in (200, 100, 75, 50, 25, "fit", "custom"):
            label = {"fit": "Fit selection", "custom": "Custom:"}.get(v, "%d%%" % v if isinstance(v, int) else v)
            rb = Gtk.RadioButton.new_with_label_from_widget(group, label)
            group = group or rb
            radios[v] = rb
            if v == "custom":
                row = Gtk.Box(spacing=6)
                row.pack_start(rb, False, False, 0)
                spin = Gtk.SpinButton.new_with_range(10, 400, 1)
                spin.set_value(cur)
                spin.connect("changed", lambda *_: radios["custom"].set_active(True))
                row.pack_start(spin, False, False, 0)
                row.pack_start(Gtk.Label(label="%"), False, False, 0)
                inner.pack_start(row, False, False, 0)
            else:
                inner.pack_start(rb, False, False, 0)
        (radios.get(cur) or radios["custom"]).set_active(True)
        box.pack_start(frame, False, False, 0)
        d.show_all()
        r = d.run()
        choice = next((k for k, rb in radios.items() if rb.get_active()), "custom")
        val = spin.get_value()
        d.destroy()
        if r != Gtk.ResponseType.OK:
            return
        if choice == "fit":
            self.win.run("zoom-selection")
        elif choice == "custom":
            self.win.set_zoom(val)
        else:
            self.win.set_zoom(choice)
