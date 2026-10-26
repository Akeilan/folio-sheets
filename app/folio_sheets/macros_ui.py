"""Macro-enabled workbooks (SH-24): the macros are kept in the file but never run.

As in Excel with macros disabled: a SECURITY WARNING bar when such a workbook opens, the Macro
dialog listing its macros (Alt+F8), the macro-free warning when it is saved as .xlsx, and the
macro-enabled file types in Save As. Running, recording and editing VBA say they aren't available.
"""
from gi.repository import Gtk

from . import APP_NAME

NOT_RUN = ("%s doesn't run macros. The macros stay in this workbook, and saving it as an Excel "
           "Macro-Enabled Workbook (*.xlsm) keeps them for Excel.") % APP_NAME


class MacroBar(Gtk.Box):
    """The yellow message bar under the ribbon."""

    def __init__(self, win):
        super().__init__(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.win = win
        self.get_style_context().add_class("folio-msgbar")
        from . import icons
        self.pack_start(icons.image("shield-warning", 16), False, False, 0)
        title = Gtk.Label(xalign=0)
        title.set_markup("<b>SECURITY WARNING</b>")
        self.pack_start(title, False, False, 0)
        self.text = Gtk.Label(label="Macros have been disabled.", xalign=0)
        self.pack_start(self.text, False, False, 0)
        self.enable = Gtk.Button(label="Enable Content")
        self.enable.set_tooltip_text("Macros can't run in %s; this explains what happens to them." % APP_NAME)
        self.enable.connect("clicked", lambda *_: self.enable_content())
        self.pack_start(self.enable, False, False, 0)
        close = Gtk.Button(label="✕")
        close.set_relief(Gtk.ReliefStyle.NONE)
        close.get_style_context().add_class("flat")
        close.set_tooltip_text("Close")
        close.connect("clicked", lambda *_: self.hide())
        self.pack_end(close, False, False, 0)
        self.set_no_show_all(True)

    def show_for(self, book):
        if book is not None and book.has_macros():
            for c in self.get_children():      # (no_show_all keeps show_all() off the bar itself)
                c.show_all()
            self.show()
        else:
            self.hide()

    def enable_content(self):
        self.win.dialogs.info(NOT_RUN, "Macros")
        self.hide()


class MacroDialog(Gtk.Dialog):
    """View Macros (Alt+F8): the workbook's macros; Run, Step Into, Edit and Create are off."""

    def __init__(self, win):
        super().__init__(title="Macro", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.set_default_size(420, 380)
        self.win = win
        area = self.get_content_area()
        area.set_border_width(10)
        area.set_spacing(6)
        area.pack_start(Gtk.Label(label="Macro name:", xalign=0), False, False, 0)
        self.name = Gtk.Entry()
        area.pack_start(self.name, False, False, 0)
        row = Gtk.Box(spacing=8)
        self.store = Gtk.ListStore(str)
        self.view = Gtk.TreeView(model=self.store, headers_visible=False)
        self.view.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        self.view.get_selection().connect("changed", self._picked)
        sw = Gtk.ScrolledWindow()
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.set_size_request(260, 200)
        sw.add(self.view)
        row.pack_start(sw, True, True, 0)
        btns = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self.buttons = {}
        for label in ("Run", "Step Into", "Edit", "Create", "Delete", "Options..."):
            b = Gtk.Button(label=label)
            b.set_sensitive(False)
            b.set_tooltip_text("Not available: %s doesn't run or edit macros." % APP_NAME)
            btns.pack_start(b, False, False, 0)
            self.buttons[label] = b
        cancel = Gtk.Button(label="Cancel")
        cancel.connect("clicked", lambda *_: self.response(Gtk.ResponseType.CANCEL))
        btns.pack_start(cancel, False, False, 6)
        row.pack_start(btns, False, False, 0)
        area.pack_start(row, True, True, 0)
        h = Gtk.Box(spacing=6)
        h.pack_start(Gtk.Label(label="Macros in:"), False, False, 0)
        self.where = Gtk.ComboBoxText()
        self.where.append_text("All Open Workbooks")
        self.where.append_text("This Workbook")
        self.where.set_active(0)
        self.where.connect("changed", lambda *_: self.fill())
        h.pack_start(self.where, True, True, 0)
        area.pack_start(h, False, False, 0)
        self.note = Gtk.Label(xalign=0)
        self.note.set_line_wrap(True)
        self.note.set_max_width_chars(56)
        area.pack_start(self.note, False, False, 0)
        self.fill()
        self.show_all()

    def names(self):
        wins = [self.win]
        if self.where.get_active() == 0:
            wins = list(getattr(self.win.app, "windows", [self.win]))
        out = []
        seen = set()
        for w in wins:
            if w.book is None or id(w.book) in seen:
                continue
            seen.add(id(w.book))
            prefix = "" if w is self.win or len(wins) == 1 else "'%s'!" % w.doc_name()
            for shown, _module, _std in w.book.macros():
                out.append(prefix + shown)
        return out

    def fill(self):
        self.store.clear()
        names = self.names()
        for n in names:
            self.store.append([n])
        if names:
            self.view.get_selection().select_path(Gtk.TreePath.new_first())
            self.note.set_text(NOT_RUN)
        elif self.win.book is not None and self.win.book.has_macros():
            self.note.set_text("This workbook has a VBA project, but no macros that can be run from here.")
        else:
            self.note.set_text("This workbook has no macros.")

    def _picked(self, sel):
        m, it = sel.get_selected()
        if it is not None:
            self.name.set_text(m[it][0])


def show_macros(win):
    d = MacroDialog(win)
    d.run()
    d.destroy()


def check_macro_free(win, path):
    """Before saving a workbook with macros in a macro-free type: Excel's question.
    True = go ahead (macros left out), False = cancelled, 'xlsm' = choose a macro-enabled type."""
    import os
    ext = os.path.splitext(path)[1].lower()
    if ext in (".xlsm", ".xltm") or win.book is None or not win.book.has_macros():
        return True
    d = Gtk.MessageDialog(transient_for=win, modal=True, message_type=Gtk.MessageType.WARNING,
                          text="The following features cannot be saved in macro-free workbooks:")
    d.format_secondary_text(
        "• VB project\n\n"
        "To save a file with these features, click No, and then choose a macro-enabled file type in the "
        "File Type list.\nTo continue saving as a macro-free workbook, click Yes.")
    d.set_title(APP_NAME)
    d.add_button("Yes", Gtk.ResponseType.YES)
    d.add_button("No", Gtk.ResponseType.NO)
    d.set_default_response(Gtk.ResponseType.NO)
    r = d.run()
    d.destroy()
    if r == Gtk.ResponseType.YES:
        return True
    if r == Gtk.ResponseType.NO:
        return "xlsm"
    return False
