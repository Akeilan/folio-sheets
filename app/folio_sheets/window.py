"""The workbook window."""
import csv
import io
import os
import subprocess
import tempfile

from gi.repository import Gdk, GLib, Gtk

from . import APP_NAME, commands, icons
from .backstage import Backstage
from .core import MAX_COLS, MAX_ROWS, Book, a1, parse_ref, range_a1
from .dialogs import Dialogs
from . import fn_dialogs
from .core import check_formula
from .formula_edit import FormulaAssist
from .formatting import FormattingMixin
from .formulabar import FormulaBar
from .history import HistoryBook
from .grid import GridView
from .ribbon import Ribbon
from .sheettabs import SheetTabs
from .statusbar import StatusBar

NATIVE = (".xlsx", ".xlsm", ".xltx", ".xltm")
IMPORT = (".xls", ".ods", ".xlsb", ".fods", ".sxc")


class SheetWindow(FormattingMixin, Gtk.ApplicationWindow):
    def __init__(self, app, path=None, share=None):
        Gtk.ApplicationWindow.__init__(self, application=app)
        self.app = app
        self.side_partner = None        # View Side by Side
        self.sync_scroll = False
        self.path = None
        self.untitled = None
        self.imported_from = None
        self.book = None
        self.syncing_checks = False
        self.goto_history = []
        self.missing_log = []
        self._alt_alone = False
        geo = app.settings.get("window", {})
        self.set_default_size(geo.get("w", 1280), geo.get("h", 820))
        if geo.get("max", True):
            self.maximize()
        self.set_icon_name("folio-sheets")
        self.dialogs = Dialogs(self)
        self.assist = FormulaAssist(self)
        self.show_formulas = set()      # sheets showing formulas
        self.arrows = {}                # sheet -> tracer arrows
        self.watch = None

        # ---- title bar
        self.header = Gtk.HeaderBar()
        self.header.set_show_close_button(True)
        self.header.get_style_context().add_class("folio-titlebar")
        self.title_label = Gtk.Label(label=APP_NAME)
        self.title_label.get_style_context().add_class("title")
        self.header.set_custom_title(self.title_label)
        self.qat = Gtk.Box(spacing=0)
        self.build_qat()
        self.header.pack_start(self.qat)
        self.search = Gtk.SearchEntry()
        self.search.set_placeholder_text("Search (Alt+Q)")
        self.search.get_style_context().add_class("folio-search")
        self.search.set_width_chars(28)
        self.search.connect("activate", self.on_search)
        self.header.pack_end(self.search)
        self.set_titlebar(self.header)

        # ---- layout
        self.overlay = Gtk.Overlay()
        self.add(self.overlay)
        self.outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.overlay.add(self.outer)
        self.ribbon = Ribbon(self.run, self.focus_document)
        self.ribbon.connect("file-clicked", lambda *_: self.show_backstage())
        self.outer.pack_start(self.ribbon, False, False, 0)
        from .ink_ui import InkTools
        self.ink = InkTools(self)
        from .macros_ui import MacroBar
        self.macro_bar = MacroBar(self)
        self.outer.pack_start(self.macro_bar, False, False, 0)
        self.grid = GridView(self)
        from .validation_ui import DvUi
        self.dv = DvUi(self)
        self.dv.attach(self.grid)
        from .links_ui import LinksUi
        self.links = LinksUi(self)
        self.show_last_colors()
        self.formula_bar = FormulaBar(self)
        self.outer.pack_start(self.formula_bar, False, False, 0)
        mid = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        mid.pack_start(self.grid, True, True, 0)
        from .notes_ui import make_pane
        self.comments_pane = make_pane(self)
        mid.pack_start(self.comments_pane, False, False, 0)
        from . import getdata_ui
        self.queries_pane = getdata_ui.make_pane(self)
        mid.pack_start(self.queries_pane, False, False, 0)
        self.outer.pack_start(mid, True, True, 0)
        self.tabs = SheetTabs(self)
        self.outer.pack_start(self.tabs, False, False, 0)
        self.status = StatusBar(self)
        self.outer.pack_start(self.status, False, False, 0)

        from .ribbon_defs import build_ribbon
        build_ribbon(self)
        self.ribbon.update("ink:select", "true")
        self.backstage = Backstage(self)
        self.overlay.add_overlay(self.backstage)
        from .keytips import KeyTips
        self.keytips = KeyTips(self)

        self.connect("key-press-event", self.on_key)
        self.connect("key-release-event", self.on_key_release)
        self.connect("delete-event", self.on_delete)
        self.connect("configure-event", self._remember_geometry)
        self.show_all()
        self.backstage.hide()
        if share is not None:
            self._share_from(share)
        elif path:
            self.load(path)
        else:
            self.new_book()
        from . import windows_ui
        windows_ui.watch(self)
        GLib.idle_add(self.grid.area.grab_focus)

    def _share_from(self, other):
        """New Window: the same workbook, seen from where the other window is."""
        self.book = other.book
        self.path, self.untitled, self.imported_from = other.path, other.untitled, other.imported_from
        self.grid.set_book(self.book)
        og = other.grid
        self.grid.sheet = og.sheet
        st, ost = self.grid.state(), og.state()
        for k in ("top_row", "left_col", "zoom", "freeze_rows", "freeze_cols", "show_grid", "show_headers"):
            setattr(st, k, getattr(ost, k))
        st.sel.ranges = [tuple(r) for r in ost.sel.ranges]
        st.sel.active = tuple(ost.sel.active)
        self.tabs.rebuild()
        self.sheet_changed()
        self.update_title()

    # ------------------------------------------------------------ QAT
    # every command the Quick Access Toolbar can hold, in its menu's order
    QAT_ALL = [("new", "qat-new", "New (Ctrl+N)", "New"), ("open", "r-browse", "Open (Ctrl+O)", "Open"),
               ("save", "qat-save", "Save (Ctrl+S)", "Save"), ("undo", "r-undo-sm", "Undo (Ctrl+Z)", "Undo"),
               ("redo", "r-redo-sm", "Redo (Ctrl+Y)", "Redo"), ("print", "qat-print", "Print (Ctrl+P)", "Print")]
    QAT_DEFAULT = ["save", "undo", "redo"]

    @property
    def QAT(self):
        on = self.app.settings.get("qat", self.QAT_DEFAULT)
        return [(k, i, t) for k, i, t, _n in self.QAT_ALL if k in on]

    def build_qat(self):
        for c in self.qat.get_children():
            self.qat.remove(c)
        for key, icon, tip in self.QAT:
            b = Gtk.Button()
            b.add(icons.image(icon, 16) if not icon.startswith(("cmd/", "r-")) else icons.white_image(icon, 16))
            b.set_tooltip_text(tip)
            b.set_focus_on_click(False)
            b.connect("clicked", lambda _b, k=key: self.run(k))
            self.qat.pack_start(b, False, False, 0)
        # the customise menu (suite spec): tick commands on or off
        mb = Gtk.MenuButton()
        mb.set_relief(Gtk.ReliefStyle.NONE)
        mb.set_focus_on_click(False)
        mb.set_tooltip_text("Customize Quick Access Toolbar")
        mb.add(Gtk.Label(label="\u25be"))
        menu = Gtk.Menu()
        head = Gtk.MenuItem(label="Customize Quick Access Toolbar")
        head.set_sensitive(False)
        menu.append(head)
        on = self.app.settings.get("qat", self.QAT_DEFAULT)
        for k, _i, _t, name in self.QAT_ALL:
            mi = Gtk.CheckMenuItem(label=name)
            mi.set_active(k in on)
            mi.connect("toggled", lambda m, k=k: GLib.idle_add(lambda: (self.set_qat(k, m.get_active()), False)[1]))
            menu.append(mi)
        menu.show_all()
        mb.set_popup(menu)
        self.qat_menu_button = mb
        self.qat.pack_start(mb, False, False, 0)
        self.qat.show_all()

    def set_qat(self, key, on):
        cur = [k for k in self.app.settings.get("qat", self.QAT_DEFAULT) if k != key]
        if on:
            cur.append(key)
        self.app.settings["qat"] = [k for k, *_ in self.QAT_ALL if k in cur]
        try:
            self.app.save_settings()
        except OSError:
            pass
        for w in self.app.windows:
            w.build_qat()

    # ------------------------------------------------------------ books
    def new_book(self):
        tpl = self.app.blank_template()
        if tpl is None:
            raise RuntimeError("the blank workbook template is missing")
        self.book = self._wrap(Book.open(tpl))
        self.path = None
        self.untitled = self.app.next_untitled()
        self._after_load()

    def load(self, path):
        ext = os.path.splitext(path)[1].lower()
        try:
            if ext in NATIVE:
                book = Book.open(path)
                self.imported_from = None
                self.path = os.path.abspath(path)
            elif ext in (".txt", ".prn"):
                # a text file opens through the Text Import Wizard (CSV opens directly)
                book = self._import_text_wizard(path)
                if book is None:
                    if self.book is None:
                        self.new_book()
                    return
                self.imported_from = os.path.abspath(path)
                self.path = None
            elif ext in (".csv", ".tsv"):
                book = self._import_text(path)
                self.imported_from = os.path.abspath(path)
                self.path = None
            elif ext in IMPORT:
                book = self._import_legacy(path)
                self.imported_from = os.path.abspath(path)
                self.path = None
            else:
                book = Book.open(path)
                self.path = os.path.abspath(path)
        except Exception as e:
            self.dialogs.message("%s can't open '%s'.\n\n%s" % (APP_NAME, os.path.basename(path), e))
            if self.book is None:
                self.new_book()
            return
        self.book = self._wrap(book)
        if self.path is None:
            self.untitled = os.path.splitext(os.path.basename(path))[0]
        self.app.add_recent(path)
        self._after_load()

    def _wrap(self, book):
        hb = HistoryBook(book)
        from . import windows_ui
        hb.on_switch = lambda: windows_ui.book_switched(self)
        return hb

    def _book_switched(self):
        """Undo/redo stepped across a formatting command: the engine Book changed."""
        g = self.grid
        g.copy_range = None
        if g.sheet >= self.book.sheet_count:
            g.sheet = self.book.sheet_count - 1
        self.tabs.rebuild()
        self.update_title()

    def _import_text(self, path):
        """CSV/TSV: values typed into a blank workbook (the Text Import
        Wizard with its column types comes later)."""
        book = Book.open(self.app.blank_template())
        with open(path, newline="", encoding="utf-8-sig", errors="replace") as f:
            sample = f.read(65536)
            f.seek(0)
            delim = "\t" if path.lower().endswith(".tsv") else ","
            try:
                delim = csv.Sniffer().sniff(sample, delimiters=",;\t").delimiter
            except csv.Error:
                pass
            for r, row in enumerate(csv.reader(f, delimiter=delim)):
                for c, v in enumerate(row):
                    if v != "" and r < MAX_ROWS and c < MAX_COLS:
                        book.set_input(0, r, c, v if not v.startswith("=") else "'" + v)
        book.mark_saved()
        return book

    def _import_text_wizard(self, path):
        from . import text_ui
        res = text_ui.import_wizard(self, path)
        if res is None:
            return None
        opts, lines = res
        book = Book.open(self.app.blank_template())
        cells = opts.cells(lines)
        text_cols = {j for row in cells for j, (_t, f) in enumerate(row) if f == "text"}
        for j in text_cols:
            book.set_number_format(0, [(0, j, max(len(cells), 1) - 1, j)], "@")
        for i, row in enumerate(cells[:MAX_ROWS]):
            for j, (t, _f) in enumerate(row[:MAX_COLS]):
                if t != "":
                    book.set_input(0, i, j, t)
        book.mark_saved()
        return book

    def _import_legacy(self, path):
        """Legacy formats go through LibreOffice as a converter only."""
        out = tempfile.mkdtemp(prefix="folio-sheets-")
        cmd = ["soffice", "--headless", "--norestore", "--convert-to", "xlsx:Calc MS Excel 2007 XML",
               "--outdir", out, path]
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=180)
        conv = os.path.join(out, os.path.splitext(os.path.basename(path))[0] + ".xlsx")
        if not os.path.exists(conv):
            raise RuntimeError("the converter could not read this file")
        return Book.open(conv)

    def _after_load(self):
        self.macro_bar.show_for(self.book)
        self.grid.set_book(self.book)
        self.tabs.rebuild()
        self.sheet_changed()
        self.update_title()

    def doc_name(self):
        return os.path.basename(self.path) if self.path else (self.untitled or "Book1")

    def doc_title(self):
        """The name in the title bar: Book1, or Book1:2 for a second window on it."""
        from .windows_ui import view_number
        name = self.doc_name()
        if self.path:
            name = os.path.splitext(name)[0]
        n = view_number(self) if self.book is not None else 0
        return "%s:%d" % (name, n) if n else name

    def update_title(self):
        name = self.doc_title()
        self.title_label.set_text("%s - %s" % (name, APP_NAME))
        self.set_title("%s - %s" % (name, APP_NAME))

    # ------------------------------------------------------------ state
    def backstage_open(self):
        return self.backstage.get_visible()

    def show_backstage(self, page="home"):
        self.keytips.leave()
        if self.grid.editing():
            self.grid.commit_edit()
        self.backstage.open(page)

    def focus_document(self):
        self.grid.area.grab_focus()

    def sheet_changed(self):
        self.tabs.mark_active()
        st = self.grid.state()
        self.status.show_zoom(st.zoom * 100)
        self.sync_checks()
        self.ribbon.update("split", "true" if getattr(st, "split", False) else "false")
        from . import protect_ui
        protect_ui.after(self)
        self.grid.refresh()

    def sync_checks(self):
        st = self.grid.state()
        self.syncing_checks = True
        try:
            self.chk_gridlines.set_active(st.show_grid)
            self.chk_headings.set_active(st.show_headers)
            self.chk_formula_bar.set_active(self.formula_bar.get_visible())
        finally:
            self.syncing_checks = False

    def on_selection_changed(self):
        g = self.grid
        if self.book is None:
            return
        if not g.editing():
            r, c = g.sel.active
            self.formula_bar.namebox.show_text(a1(r, c))
            from .protect_ui import hides_formula
            self.formula_bar.show_text("" if hides_formula(self, r, c) else self.book.input(g.sheet, r, c))
            st = self.book.cell_style(g.sheet, r, c)
            self.update_ribbon_state(st)
        self.dv.update_message()
        self.status.update_aggregates()
        self.update_calc_status()
        self.sync_table_tab()
        title_dirty = self.book.modified
        if getattr(self, "_last_dirty", None) != title_dirty:
            self._last_dirty = title_dirty

    def update_calc_status(self):
        b = self.book
        circ = b.circular()
        if circ:
            s, r, c = circ[0]
            name = self.book.sheet_info(s)["name"]
            txt = "Circular References: %s" % (a1(r, c) if s == self.grid.sheet else "%s!%s" % (name, a1(r, c)))
        elif b.calc_manual and b.calc_pending:
            txt = "Calculate"
        else:
            txt = ""
        self.status.set_calc(txt)

    def update_ribbon_state(self, st):
        r = self.ribbon
        r.update("bold", "true" if st.get("bold") else "false")
        r.update("italic", "true" if st.get("italic") else "false")
        r.update("underline", "true" if st.get("underline") else "false")
        r.update("wrap-text", "true" if st.get("wrap") else "false")
        try:
            ar, ac = self.grid.sel.active
            merged = any(m[0] <= ar <= m[2] and m[1] <= ac <= m[3] for m in (self.book.merges(self.grid.sheet) or []))
        except Exception:
            merged = False
        r.update("merge-center", "true" if merged else "false")
        h = st.get("halign") or ""
        v = st.get("valign") or "bottom"
        for k in ("left", "center", "right"):
            r.update("align-" + k, "true" if h == k else "false")
        r.update("align-top", "true" if v == "top" else "false")
        r.update("align-middle", "true" if v == "center" else "false")
        r.update("align-bottom", "true" if v in ("bottom", "") else "false")
        self.font_combo.apply_state(st.get("font", ""))
        size = st.get("size", 11)
        self.size_combo.apply_state(("%g" % size))
        from .ribbon_defs import NUMBER_FORMATS
        code = st.get("numfmt", "General")
        name = next((n for n, c in NUMBER_FORMATS if c == code), "Custom" if code != "General" else "General")
        self.numfmt_combo.apply_state(name)

    def show_drag_size(self):
        g = self.grid
        rg = g.sel.last
        rows, cols = rg[2] - rg[0] + 1, rg[3] - rg[1] + 1
        if rows > 1 or cols > 1:
            self.formula_bar.namebox.show_text("%dR x %dC" % (rows, cols))

    # ------------------------------------------------------------ editing hooks
    def edit_started(self, text):
        self.formula_bar.show_text(text)
        self.formula_bar.set_editing(True)
        self.status.set_mode("Edit" if self.grid.editor.mode == "edit" else "Enter")
        GLib.idle_add(lambda: (self.assist.update() if self.grid.editing() else None, False)[1])

    def edit_changed(self, text):
        self.formula_bar.show_text(text)

    def edit_mode_changed(self, mode):
        self.status.set_mode("Edit" if mode == "edit" else "Enter")

    def edit_ended(self):
        self.assist.end_point()
        self.assist.hide_popups()
        self.assist.refs = []
        self.formula_bar.set_editing(False)
        self.status.set_mode("Ready")
        self.update_title()

    def formula_mode(self):
        return self.assist.active()

    def point_ready(self):
        return self.assist.point_ready()

    def point_move(self, dr, dc, extend=False):
        self.assist.point_move(dr, dc, extend)

    def insert_reference(self, r, c, extend=False):
        self.assist.insert_reference(r, c, extend)

    def validate_entry(self, text):
        """Before a typed entry is stored: formulas must parse (open
        parentheses are closed for you), and the cell's data validation must
        let it in. None keeps the edit going; False cancels it."""
        t = text
        if t.startswith("=") and len(t) > 1:
            body = t[1:]
        elif len(t) > 1 and t[0] in "+-" and not t[1:].replace(".", "", 1).replace(",", "").isdigit() \
                and any(ch.isalpha() for ch in t):
            body = t
        else:
            return self.dv.check_entry(text)
        err = check_formula(body)
        if err is None:
            return self.dv.check_entry(text)
        self.formula_error(text, err)
        return None

    def formula_error(self, text, err):
        self.dialogs.message("There's a problem with this formula.\n\nNot trying to type a formula? When the "
                             "first character is an equal (=) or minus (-) sign, it is read as a formula. To "
                             "enter it as text, type an apostrophe (') first.", "Formula")

    def insert_into_formula(self, s, call=True):
        """Put a function call or a name at the cursor of the formula being
        typed; outside editing, the cell gets =s."""
        g = self.grid
        if g.editing():
            t = self.assist.text()
            pos = self.assist.cursor()
            if not t:
                s = "=" + s
            self.assist._replace(pos, pos, s)
            self.assist.update()
            (self.formula_bar.view if self.formula_bar.view.has_focus() else g.editor).grab_focus()
            return
        r, c = g.sel.active
        self.book.set_input(g.sheet, r, c, "=" + s)
        g.refresh()

    def fill_selection_with_edit(self):
        """Ctrl+Enter: the typed entry goes into every selected cell."""
        g = self.grid
        text = g.editor.text()
        from .protect_ui import may_change_cells
        if not may_change_cells(self):
            return
        checked = self.validate_entry(text)
        if checked is None:
            return
        if checked is False:
            g.cancel_edit()
            return
        text = checked
        g.cancel_edit()
        cells = []
        for rg in g.sel.ranges:
            if (rg[2] - rg[0] + 1) * (rg[3] - rg[1] + 1) > 1_000_000:
                self.dialogs.message("The selection is too large to fill.")
                return
            for r in range(rg[0], rg[2] + 1):
                for c in range(rg[1], rg[3] + 1):
                    cells.append((r, c))
        with self.book.group("Typing"):
            for r, c in cells:
                self.book.set_input(g.sheet, r, c, text)
        g.refresh()

    # ------------------------------------------------------------ commands
    def run(self, cmd):
        if cmd is None:
            return
        self.keytips.leave()
        name = commands.method_name(cmd)
        arg = cmd.split(":", 1)[1] if ":" in cmd else None
        fn = getattr(self, name, None)
        if fn is None:
            self.not_yet(cmd)
            return
        keep = ("undo", "redo", "insert-function", "fn-insert", "use-in-formula", "autosum", "autosum-average",
                "autosum-count", "autosum-max", "autosum-min")
        if self.grid.editing() and cmd.split(":", 1)[0] not in keep:
            if self.grid.commit_edit() is False:
                return
        base = cmd.split(":", 1)[0]
        from . import protect_ui
        if self.book is not None and not protect_ui.gate(self, cmd):
            return
        guard = self.book is not None and protect_ui.protected(self) and base not in protect_ui.FREE
        if guard:
            h0 = self.book.history_state
            self._approved = protect_ui.need_of(base) is not None
        grouping = self.book is not None and base not in ("undo", "redo")
        try:
            if grouping:
                # whatever engine edits one command makes, one undo step takes back
                with self.book.group(commands.label(cmd)):
                    fn(arg) if arg is not None else fn()
            else:
                fn(arg) if arg is not None else fn()
        except NotImplementedError:
            self.not_yet(cmd)
        if guard:
            protect_ui.after_command(self, base, h0)

    # ------------------------------------------------------------ Draw tab
    def act_draw_select(self):
        self.ink.set_tool(None)

    def act_draw_pen(self):
        self.ink.set_tool("pen")

    def act_draw_pencil(self):
        self.ink.set_tool("pencil")

    def act_draw_highlighter(self):
        self.ink.set_tool("highlighter")

    def act_draw_eraser(self):
        self.ink.set_tool("eraser")

    def act_draw_lasso(self):
        self.ink.set_tool("lasso")

    def act_ink_width(self, arg):
        tool, mm = arg.split(":")
        self.ink.set_width(tool, float(mm))

    def act_ink_color(self, arg):
        tool, c = arg.split(":")
        self.ink.set_color(tool, int(c, 16))

    def act_draw_ruler(self):
        from . import ink_ui
        ink_ui.not_available(self, "The ruler")

    def act_ink_to_shape(self):
        from . import ink_ui
        ink_ui.not_available(self, "Ink to Shape")

    def act_ink_to_math(self):
        from . import ink_ui
        ink_ui.not_available(self, "Ink to Math")

    def act_ink_replay(self):
        from . import ink_ui
        ink_ui.not_available(self, "Ink Replay")

    def act_macros(self):
        from . import macros_ui
        macros_ui.show_macros(self)

    def act_record_macro(self):
        self.dialogs.info("Recording macros isn't available: %s doesn't run or record macros." % APP_NAME, "Record Macro")

    def act_relative_refs(self):
        self.dialogs.info("Recording macros isn't available: %s doesn't run or record macros." % APP_NAME, "Use Relative References")

    def act_visual_basic(self):
        self.dialogs.info("Visual Basic isn't available in %s. The macros in a workbook are kept when it is saved "
                          "as an Excel Macro-Enabled Workbook (*.xlsm)." % APP_NAME, "Visual Basic")

    def not_yet(self, cmd):
        label = commands.label(cmd)
        self.missing_log.append(cmd)
        self.status.extra.set_text("“%s” is not available yet." % label)
        GLib.timeout_add(4000, lambda: (self.status.extra.set_text(""), False)[1])

    # file
    def act_new(self):
        self.app.new_window()

    def choose_open_file(self):
        d = Gtk.FileChooserNative(title="Open", transient_for=self, action=Gtk.FileChooserAction.OPEN)
        f = Gtk.FileFilter()
        f.set_name("All Spreadsheets")
        for pat in ("*.xlsx", "*.xlsm", "*.xltx", "*.xls", "*.ods", "*.csv", "*.txt"):
            f.add_pattern(pat)
            f.add_pattern(pat.upper())
        d.add_filter(f)
        a = Gtk.FileFilter()
        a.set_name("All Files")
        a.add_pattern("*")
        d.add_filter(a)
        if self.path:
            d.set_current_folder(os.path.dirname(self.path))
        r = d.run()
        p = d.get_filename() if r == Gtk.ResponseType.ACCEPT else None
        d.destroy()
        return p

    def act_open(self):
        p = self.choose_open_file()
        if p:
            self.app.open_path(p, self)

    def act_save(self):
        if self.path is None or os.path.splitext(self.path)[1].lower() not in NATIVE:
            return self.act_save_as()
        return self.save_to(self.path)

    def act_save_as(self, fmt=None):
        d = Gtk.FileChooserNative(title="Save As", transient_for=self, action=Gtk.FileChooserAction.SAVE)
        d.set_do_overwrite_confirmation(True)
        filters = [("Excel Workbook (*.xlsx)", "*.xlsx", "xlsx"), ("Excel Macro-Enabled Workbook (*.xlsm)", "*.xlsm", "xlsm"),
                   ("Excel Template (*.xltx)", "*.xltx", "xltx"), ("Excel Macro-Enabled Template (*.xltm)", "*.xltm", "xltm"),
                   ("CSV (Comma delimited) (*.csv)", "*.csv", "csv")]
        fobjs = {}
        for name, pat, key in filters:
            f = Gtk.FileFilter()
            f.set_name(name)
            f.add_pattern(pat)
            d.add_filter(f)
            fobjs[key] = f
        cur = os.path.splitext(self.path)[1].lower().lstrip(".") if self.path else ""
        want = fmt or (cur if cur in ("xlsm", "xltx", "xltm") else "xlsx")
        self._save_filters = fobjs
        d.set_filter(fobjs[want])
        base = os.path.splitext(self.doc_name())[0]
        d.set_current_name(base + "." + want)
        if self.path:
            d.set_current_folder(os.path.dirname(self.path))
        r = d.run()
        p = d.get_filename() if r == Gtk.ResponseType.ACCEPT else None
        chosen = d.get_filter()
        d.destroy()
        if not p:
            return False
        kind = next((k for k, f in fobjs.items() if f is chosen), "xlsx")
        if p.lower().endswith(".csv"):
            kind = "csv"
        if kind == "csv":
            if not p.lower().endswith(".csv"):
                p += ".csv"
            return self.save_csv(p)
        ext = os.path.splitext(p)[1].lower()
        if ext not in NATIVE:
            p += "." + kind
        elif ext.lstrip(".") != kind and chosen is not None:
            # the type chosen in the list wins over a different extension typed
            p = os.path.splitext(p)[0] + "." + kind
        return self.save_to(p)

    def save_to(self, path):
        if self.grid.editing():
            self.grid.commit_edit()
        from . import macros_ui
        ok = macros_ui.check_macro_free(self, path)
        if ok == "xlsm":
            return self.act_save_as("xlsm")
        if not ok:
            return False
        macro_free = os.path.splitext(path)[1].lower() not in (".xlsm", ".xltm")
        try:
            self.store_view_state()
            self.book.save(path)
            if macro_free and self.book.has_macros():
                # as in Excel: the open workbook no longer has the macros it was saved without
                self.book.drop_macros()
                self.macro_bar.hide()
        except Exception as e:
            self.dialogs.message("The workbook couldn't be saved.\n\n%s" % e)
            return False
        self.path = os.path.abspath(path)
        self.imported_from = None
        self.book.mark_saved()
        self.app.add_recent(self.path)
        from .windows_ui import views
        for w in views(self):
            w.path, w.imported_from = self.path, None
            w.update_title()
        return True

    def save_csv(self, path):
        """The active sheet's displayed values, as the original writes CSV."""
        g = self.grid
        ext = self.book.used_extent(g.sheet)
        buf = io.StringIO()
        w = csv.writer(buf, lineterminator="\r\n")
        if ext:
            for r in range(ext[0] + 1):
                w.writerow([self.book.display(g.sheet, r, c) for c in range(ext[1] + 1)])
        try:
            with open(path, "w", encoding="utf-8", newline="") as f:
                f.write(buf.getvalue())
        except OSError as e:
            self.dialogs.message("The file couldn't be saved.\n\n%s" % e)
            return False
        if self.book.sheet_count > 1:
            self.status.extra.set_text("Only the active sheet was saved to the CSV file.")
        return True

    def store_view_state(self):
        """Selections, scroll positions, zoom and panes go back into the
        file (only where they changed)."""
        for i, st in list(self.grid.states.items()):
            if i >= self.book.sheet_count:
                # a sheet that undo took away again
                self.grid.states.pop(i, None)
                continue
            try:
                self.book.set_view(i, {
                    "top_row": st.top_row, "left_col": st.left_col, "zoom": int(round(st.zoom * 100)),
                    # a split isn't kept in the file as frozen panes
                    "freeze_rows": 0 if getattr(st, "split", False) else st.freeze_rows,
                    "freeze_cols": 0 if getattr(st, "split", False) else st.freeze_cols,
                    "show_grid": st.show_grid, "show_headers": st.show_headers,
                    "active": st.sel.active, "sel": st.sel.ranges, "selected": i == self.grid.sheet,
                    "view": getattr(st, "view_mode", "normal"),
                })
            except AttributeError:
                pass
        try:
            self.book.active_sheet = self.grid.sheet
        except Exception:
            pass

    def act_close(self):
        if self.confirm_close():
            self._closing()
            self.destroy()
            self.app.window_closed(self)

    def _closing(self):
        from . import windows_ui
        if self.side_partner is not None:
            windows_ui.end_side_by_side(self)
        others = [w for w in windows_ui.views(self) if w is not self]
        if others:
            self.store_view_state()
            GLib.idle_add(lambda: ([w.update_title() for w in others if w in self.app.windows], False)[1])

    def confirm_close(self):
        from .windows_ui import views
        if self.book is None or not self.book.modified:
            return True
        if len(views(self)) > 1:
            return True        # the workbook stays open in its other window
        ans = self.dialogs.confirm_save(self.doc_name())
        if ans == "save":
            return bool(self.act_save())
        return ans == "discard"

    def on_delete(self, *a):
        if self.grid.editing():
            self.grid.commit_edit()
        if not self.confirm_close():
            return True
        self._closing()
        self.app.window_closed(self)
        return False

    def _remember_geometry(self, w, ev):
        if not self.is_maximized():
            self.app.settings["window"] = {"w": ev.width, "h": ev.height, "max": False}
        else:
            self.app.settings.setdefault("window", {})["max"] = True
        return False

    def act_options(self):
        self.dialogs.info("Options will be available in a later version.", "Options")

    def act_help(self):
        p = None
        for d in ("/usr/share/folio-sheets", os.path.join(os.path.dirname(__file__), "..", "..", "share")):
            q = os.path.join(d, "SHORTCUTS.txt")
            if os.path.exists(q):
                p = q
                break
        if p:
            subprocess.Popen(["xdg-open", p], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    # undo
    def act_undo(self):
        if self.grid.editing():
            self.grid.cancel_edit()
            return
        s = self.book.undo_sheet()
        if self.book.undo() is None:
            return
        self._back_to(s)

    def act_redo(self):
        s = self.book.redo_sheet()
        if self.book.redo() is None:
            return
        self._back_to(s)

    def _back_to(self, s):
        """After undo/redo: show the sheet the step changed."""
        g = self.grid
        if s is not None and s < self.book.sheet_count and s != g.sheet:
            g.set_sheet(s)
            self.tabs.rebuild()
        self.on_object_selection()
        g.refresh()
        self.update_title()

    # clipboard (text values for now)
    def _clipboard(self):
        return Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)

    def act_copy(self, cut=False):
        from . import clip_ui
        clip_ui.copy(self, cut)

    def act_cut(self):
        self.act_copy(cut=True)

    def _start_ants(self):
        g = self.grid
        if g._ants_id:
            return

        def tick():
            if g.copy_range is None:
                g._ants_id = 0
                return False
            g.ants = (g.ants + 1) % 8
            g.area.queue_draw()
            return True
        g._ants_id = GLib.timeout_add(120, tick)

    def cancel_copy(self):
        from . import clip_ui
        clip_ui.cancel(self)

    def act_paste(self):
        from . import clip_ui
        clip_ui.paste(self)

    def _paste_as(self, **opts):
        from . import clip_ui
        clip_ui.paste(self, opts)

    def act_paste_values(self):
        self._paste_as(what="values")

    def act_paste_formulas(self):
        self._paste_as(what="formulas")

    def act_paste_keep_widths(self):
        self._paste_as(what="all_widths")

    def act_paste_link(self):
        self._paste_as(what="formulas", link=True)

    def act_paste_no_borders(self):
        self._paste_as(what="noborders")

    def act_paste_transpose(self):
        self._paste_as(what="all", transpose=True)

    def act_paste_special(self):
        from . import clip_ui
        clip_ui.paste_special(self)

    def act_paste_picture(self):
        from . import clip_ui
        clip_ui.paste_picture(self)

    def act_copy_picture(self):
        from . import clip_ui
        if not clip_ui._copy_picture_object(self):
            raise NotImplementedError

    def act_pictures(self):
        from . import clip_ui
        d = Gtk.FileChooserNative(title="Insert Picture", transient_for=self, action=Gtk.FileChooserAction.OPEN)
        f = Gtk.FileFilter()
        f.set_name("Pictures")
        for pat in ("*.png", "*.jpg", "*.jpeg", "*.gif", "*.bmp", "*.webp", "*.tif", "*.tiff", "*.ico"):
            f.add_pattern(pat)
            f.add_pattern(pat.upper())
        d.add_filter(f)
        path = d.get_filename() if d.run() == Gtk.ResponseType.ACCEPT else None
        d.destroy()
        if path:
            clip_ui.insert_picture_file(self, path)

    # editing
    def act_clear_contents(self):
        g = self.grid
        self.book.clear_contents(g.sheet, g.sel.ranges)
        g.refresh()

    def _fill(self, dr, dc):
        g = self.grid
        ext = self.book.used_extent(g.sheet) or (0, 0)
        for rg in g.sel.ranges:
            r1, c1, r2, c2 = rg
            # whole rows or columns: only as far as the sheet has data
            r2 = min(r2, max(ext[0], r1 + 1)) if r2 - r1 > 100000 else r2
            c2 = min(c2, max(ext[1], c1 + 1)) if c2 - c1 > 1000 else c2
            if (r2 - r1 + 1) * (c2 - c1 + 1) > 200000:
                self.dialogs.message("The selection is too large to fill.")
                return
            if dr:
                src_r = r1 if dr > 0 else r2
                rows = range(r1 + 1, r2 + 1) if dr > 0 else range(r1, r2)
                if r1 == r2:
                    if r1 == 0:
                        continue
                    src_r, rows = (r1 - 1, [r1]) if dr > 0 else (r1 + 1, [r1])
                for c in range(c1, c2 + 1):
                    v = self.book.input(g.sheet, src_r, c)
                    for r in rows:
                        t = v
                        if v.startswith("="):
                            t = "=" + self.book.shift_formula(v[1:], r - src_r, 0)
                        self.book.set_input(g.sheet, r, c, t)
            else:
                src_c = c1 if dc > 0 else c2
                cols = range(c1 + 1, c2 + 1) if dc > 0 else range(c1, c2)
                if c1 == c2:
                    if c1 == 0:
                        continue
                    src_c, cols = (c1 - 1, [c1]) if dc > 0 else (c1 + 1, [c1])
                for r in range(r1, r2 + 1):
                    v = self.book.input(g.sheet, r, src_c)
                    for c in cols:
                        t = v
                        if v.startswith("="):
                            t = "=" + self.book.shift_formula(v[1:], 0, c - src_c)
                        self.book.set_input(g.sheet, r, c, t)
        g.refresh()

    def act_fill_down(self):
        self._fill(1, 0)

    def act_fill_right(self):
        self._fill(0, 1)

    def act_fill_up(self):
        self._fill(-1, 0)

    def act_fill_left(self):
        self._fill(0, -1)

    # ------------------------------------------------------------ formulas tab
    def act_insert_function(self):
        fn_dialogs.insert_function(self)

    def act_fn_insert(self, name):
        fn_dialogs.function_arguments(self, name)

    def _autosum_range(self, r, c):
        """The numbers AutoSum adds for a cell: the block above, else the
        block to the left."""
        b, s = self.book, self.grid.sheet

        def is_num(rr, cc):
            v = b.value(s, rr, cc)
            return isinstance(v, (int, float)) and not isinstance(v, bool)
        if r > 0 and (is_num(r - 1, c) or (b.has_content(s, r - 1, c) and b.is_formula(s, r - 1, c))):
            top = r - 1
            while top > 0 and (is_num(top - 1, c) or (b.is_formula(s, top - 1, c) and not isinstance(
                    b.value(s, top - 1, c), str))):
                top -= 1
            return (top, c, r - 1, c)
        if c > 0 and is_num(r, c - 1):
            left = c - 1
            while left > 0 and is_num(r, left - 1):
                left -= 1
            return (r, left, r, c - 1)
        if r > 0:
            # numbers further up, past blank cells
            top = r - 1
            while top >= 0 and not b.has_content(s, top, c):
                top -= 1
            if top >= 0 and is_num(top, c):
                bottom = top
                while top > 0 and is_num(top - 1, c):
                    top -= 1
                return (top, c, r - 1, c)
        return None

    def act_autosum(self, fn="SUM"):
        g = self.grid
        fn = {"average": "AVERAGE", "count": "COUNT", "max": "MAX", "min": "MIN"}.get(fn, fn)
        if g.editing():
            self.insert_into_formula(fn + "(")
            return
        rg = g.sel.last
        from .formula_edit import ref_text
        if rg[0] == rg[2] and rg[1] == rg[3]:
            r, c = rg[0], rg[1]
            src = self._autosum_range(r, c)
            g.start_edit(initial="=%s(" % fn)
            if src:
                self.assist.point = {"start": len(fn) + 2, "end": len(fn) + 2, "anchor": (src[0], src[1]),
                                     "cursor": (src[2], src[3]), "sheet": g.sheet}
                self.assist._put()
            buf = g.editor.get_buffer()
            buf.insert(buf.get_end_iter(), ")")
            pos = len(g.editor.text()) - 1
            buf.place_cursor(buf.get_iter_at_offset(pos))
            if self.assist.point:
                self.assist.point["end"] = pos
            self.assist.update()
            return
        # a range: totals below each column (or right of each row) of numbers
        r1, c1, r2, c2 = rg
        b, s = self.book, g.sheet
        last_empty_row = not any(b.has_content(s, r2, c) for c in range(c1, c2 + 1))
        if last_empty_row and r2 > r1:
            for c in range(c1, c2 + 1):
                b.set_input(s, r2, c, "=%s(%s)" % (fn, ref_text(r1, c, r2 - 1, c)))
        elif r2 > r1 or c1 == c2:
            row = r2 + 1
            if row >= MAX_ROWS:
                self.dialogs.message("There's no room below the selection for the totals.")
                return
            for c in range(c1, c2 + 1):
                b.set_input(s, row, c, "=%s(%s)" % (fn, ref_text(r1, c, r2, c)))
        else:
            col = c2 + 1
            if col >= MAX_COLS:
                self.dialogs.message("There's no room to the right of the selection for the totals.")
                return
            for r in range(r1, r2 + 1):
                b.set_input(s, r, col, "=%s(%s)" % (fn, ref_text(r, c1, r, c2)))
        g.refresh()

    def act_autosum_average(self):
        self.act_autosum("average")

    def act_autosum_count(self):
        self.act_autosum("count")

    def act_autosum_max(self):
        self.act_autosum("max")

    def act_autosum_min(self):
        self.act_autosum("min")

    def act_name_manager(self):
        fn_dialogs.name_manager(self)

    def act_define_name(self):
        fn_dialogs.edit_name(self)

    def act_use_in_formula(self):
        fn_dialogs.paste_names(self)

    def act_names_from_selection(self):
        fn_dialogs.create_from_selection(self)

    TRACE = 0x2F6DB5

    def act_trace_precedents(self):
        g = self.grid
        r, c = g.sel.active
        if not self.book.is_formula(g.sheet, r, c):
            self.dialogs.message("The active cell must contain a formula to trace its precedents.",
                                 "Trace Precedents")
            return
        arrows = self.arrows.setdefault(g.sheet, [])
        for (s, r1, c1, r2, c2) in self.book.precedents(g.sheet, r, c):
            if s != g.sheet:
                continue
            a = ((r1, c1, min(r2, r1 + 100000), min(c2, c1 + 16383)), (r, c), self.TRACE)
            if a not in arrows:
                arrows.append(a)
        g.refresh()

    def act_trace_dependents(self):
        g = self.grid
        r, c = g.sel.active
        deps = self.book.dependents(g.sheet, r, c)
        if not deps:
            self.dialogs.message("No formulas in this workbook refer to the active cell.", "Trace Dependents")
            return
        arrows = self.arrows.setdefault(g.sheet, [])
        for (s, rr, cc) in deps:
            if s == g.sheet:
                a = ((r, c, r, c), (rr, cc), self.TRACE)
                if a not in arrows:
                    arrows.append(a)
        g.refresh()

    def act_remove_arrows(self):
        self.arrows.pop(self.grid.sheet, None)
        self.grid.refresh()

    def act_show_formulas(self):
        s = self.grid.sheet
        if s in self.show_formulas:
            self.show_formulas.discard(s)
        else:
            self.show_formulas.add(s)
        self.ribbon.update("show-formulas", "true" if s in self.show_formulas else "false")
        self.grid.refresh()

    def act_error_checking(self):
        fn_dialogs.error_checking(self)

    def act_evaluate_formula(self):
        fn_dialogs.evaluate_formula(self)

    def act_watch_window(self):
        if self.watch is None:
            self.watch = fn_dialogs.WatchWindow(self)
        self.watch.show_all()
        self.watch.reload()
        self.watch.present()

    def act_calc_options(self, mode):
        self.book.calc_manual = mode == "manual"
        for m in ("auto", "auto-no-tables", "manual"):
            self.ribbon.update("calc-options:" + m, "true" if m == mode else "false")
        self.grid.refresh()

    def act_calc_now(self):
        self.book.calculate()
        self.grid.refresh()

    def act_calc_sheet(self):
        self.book.calculate(False, self.grid.sheet)
        self.grid.refresh()

    def act_calc_full(self):
        self.book.calculate(True)
        self.grid.refresh()

    # navigation
    def goto_reference(self, text):
        g = self.grid
        t = text.strip()
        sheet = g.sheet
        if "!" in t:
            sname, t = t.rsplit("!", 1)
            sname = sname.strip("'").replace("''", "'")
            idx = next((i for i in range(self.book.sheet_count) if self.book.sheet_info(i)["name"].lower()
                        == sname.lower()), None)
            if idx is None:
                return False
            sheet = idx
        rg = parse_ref(t.replace("$", ""))
        if rg is None:
            # a defined name
            for name, ref, local, hidden in self.book.names():
                if name.lower() == t.lower() and (local is None or local == g.sheet):
                    return self.goto_reference(ref.lstrip("="))
            return False
        if sheet != g.sheet:
            g.set_sheet(sheet)
            self.sheet_changed()
        prev = range_a1(g.sel.last)
        self.goto_history = [h for h in self.goto_history if h != prev][-9:] + [prev]
        if rg[0] == rg[2] and rg[1] == rg[3]:
            g.select_cell(rg[0], rg[1])
        else:
            g.select_range(rg)
        return True

    def act_goto(self):
        self.dialogs.goto()

    def act_select_all(self):
        g = self.grid
        r, c = g.sel.active
        region = self.book.current_region(g.sheet, r, c)
        cur = g.sel.ranges[0] if len(g.sel.ranges) == 1 else None
        if region[0] == region[2] and region[1] == region[3] or cur == region:
            g.sel.select_all()
        else:
            g.sel.set_range(region, (r, c))
        g.refresh()

    # view
    def set_zoom(self, pct):
        pct = max(10, min(400, round(pct)))
        st = self.grid.state()
        st.zoom = pct / 100.0
        self.status.show_zoom(pct)
        self.grid.ensure_visible(*self.grid.sel.active)
        self.grid.refresh()
        self.grid.place_editor()

    def act_zoom_dialog(self):
        self.dialogs.zoom()

    def act_zoom_100(self):
        self.set_zoom(100)

    def act_zoom_selection(self):
        g = self.grid
        rg = g.sel.last
        ext = self.book.used_extent(g.sheet) or (0, 0)
        r2 = min(rg[2], max(ext[0], rg[0]))
        c2 = min(rg[3], max(ext[1], rg[1]))
        w = sum(self.book.col_px(g.sheet, c) for c in range(rg[1], c2 + 1))
        h = sum(self.book.row_px(g.sheet, r) for r in range(rg[0], r2 + 1))
        a = g.area.get_allocation()
        geo = g.geometry()
        z = min((a.width - geo["hw"] - 4) / max(w, 1), (a.height - geo["hh"] - 4) / max(h, 1))
        st = g.state()
        st.top_row, st.left_col = max(st.freeze_rows, rg[0]), max(st.freeze_cols, rg[1])
        self.set_zoom(z * 100)

    def act_view_gridlines(self):
        st = self.grid.state()
        st.show_grid = not st.show_grid
        self.sync_checks()
        self.grid.refresh()

    def act_view_headings(self):
        st = self.grid.state()
        st.show_headers = not st.show_headers
        self.sync_checks()
        self.grid.refresh()

    def act_view_formula_bar(self):
        self.formula_bar.set_visible(not self.formula_bar.get_visible())
        self.sync_checks()

    def act_view_normal(self):
        self.grid.state().view_mode = "normal"
        self.grid.refresh()

    def _freeze(self, rows, cols):
        g = self.grid
        st = g.state()
        st.split, st.pane_top, st.pane_left = False, 0, 0
        self.ribbon.update("split", "false")
        st.freeze_rows, st.freeze_cols = rows, cols
        st.top_row = max(st.top_row, rows)
        st.left_col = max(st.left_col, cols)
        g.refresh()

    def act_freeze_panes(self):
        st = self.grid.state()
        if st.split:
            # a split becomes frozen panes where its bars are
            return self._freeze(st.freeze_rows, st.freeze_cols)
        if st.freeze_rows or st.freeze_cols:
            return self._freeze(0, 0)
        r, c = self.grid.sel.active
        # freezes the rows above and columns left of the active cell, as
        # scrolled: what is on screen above/left stays
        self._freeze(r, c)

    def act_unfreeze(self):
        self._freeze(0, 0)

    def act_freeze_top_row(self):
        st = self.grid.state()
        st.top_row = max(st.top_row, 1)
        self._freeze(1, 0)

    def act_freeze_first_column(self):
        self._freeze(0, 1)

    def act_split(self):
        """Split the window into panes at the active cell (in the middle when it is the
        top-left cell); again to take the split away."""
        g = self.grid
        st = g.state()
        if st.split:
            st.split = False
            st.freeze_rows = st.freeze_cols = st.pane_top = st.pane_left = 0
            g.refresh()
            self.ribbon.update("split", "false")
            return
        st.freeze_rows = st.freeze_cols = 0
        g.area.queue_draw()
        r, c = g.sel.active
        first_r, last_r = g.visible_rows()
        first_c, last_c = g.visible_cols()
        top, left = st.top_row, st.left_col
        if r <= first_r and c <= first_c:
            r = first_r + max(1, (last_r - first_r) // 2)
            c = first_c + max(1, (last_c - first_c) // 2)
        st.split = True
        st.pane_top, st.pane_left = top, left
        st.freeze_rows = r if first_r < r <= last_r else 0
        st.freeze_cols = c if first_c < c <= last_c else 0
        if not st.freeze_rows:
            st.pane_top = 0
        if not st.freeze_cols:
            st.pane_left = 0
        st.top_row = max(top, st.freeze_rows)
        st.left_col = max(left, st.freeze_cols)
        if not st.freeze_rows and not st.freeze_cols:
            st.split = False
        g.refresh()
        self.ribbon.update("split", "true" if st.split else "false")

    def act_new_window(self):
        from . import windows_ui
        return windows_ui.new_window(self)

    def act_arrange_all(self):
        from . import windows_ui
        return windows_ui.arrange_dialog(self)

    def act_side_by_side(self):
        from . import windows_ui
        return windows_ui.side_by_side(self)

    def act_sync_scroll(self):
        from . import windows_ui
        windows_ui.toggle_sync(self)

    def act_reset_window(self):
        from . import windows_ui
        windows_ui.reset_position(self)

    def act_hide_window(self):
        from . import windows_ui
        return windows_ui.hide(self)

    def act_unhide_window(self):
        from . import windows_ui
        return windows_ui.unhide(self)

    def window_list_menu(self):
        from . import windows_ui
        return windows_ui.switch_menu(self)

    def act_switch_windows(self):
        pass

    # sheets
    def sheet_tab_menu(self, widget, ev):
        items = [("Insert...", "insert-sheet"), ("Delete", "delete-sheet"), ("Rename", "rename-sheet"),
                 ("Move or Copy...", "move-copy-sheet"), ("Protect Sheet...", "protect-sheet"),
                 ("Tab Color", "tab-color"), None, ("Hide", "hide-sheet"), ("Unhide...", "unhide-sheet"),
                 None, ("Select All Sheets", "select-all-sheets")]
        self._popup_menu(items, ev)

    def context_menu(self, ev, hit):
        kind = hit[0]
        common = [("Cut", "cut"), ("Copy", "copy"), ("Paste Options:", "paste"), ("Paste Special...", "paste-special"),
                  None]
        if kind == "colhdr":
            items = common + [("Insert", "insert-columns"), ("Delete", "delete-columns"),
                              ("Clear Contents", "clear-contents"), None, ("Format Cells...", "format-cells"),
                              ("Column Width...", "column-width"), ("Hide", "hide-columns"), ("Unhide", "unhide-columns")]
        elif kind == "rowhdr":
            items = common + [("Insert", "insert-rows"), ("Delete", "delete-rows"),
                              ("Clear Contents", "clear-contents"), None, ("Format Cells...", "format-cells"),
                              ("Row Height...", "row-height"), ("Hide", "hide-rows"), ("Unhide", "unhide-rows")]
        else:
            items = common + [("Smart Lookup", None), ("Insert...", "insert-cells"), ("Delete...", "delete-cells"),
                              ("Clear Contents", "clear-contents"), None, ("Quick Analysis", None),
                              ("Filter", "filter"), ("Sort", "sort-custom"), ("Get Data from Table/Range...", None),
                              None, ("New Comment", "new-comment")] + self._note_items() + [None,
                              ("Format Cells...", "format-cells"), ("Pick From Drop-down List...", "pick-from-list"),
                              ("Define Name...", "define-name")] + self.links.menu_items() + self._query_items()
            # cloud / AI lookups are not part of Folio
            items = [it for it in items if it is None or it[1] is not None]
        self._popup_menu(items, ev)

    def _query_items(self):
        """Refresh, in a table a query loaded (as Excel's cell menu has it)."""
        from . import getdata_ui
        g = self.grid
        r, c = g.sel.active
        i = self.book.table_at(g.sheet, r, c) if self.book is not None else None
        if i is None:
            return []
        name = self.book.tables(g.sheet)[i]["name"].lower()
        if any((q.get("table") or "").lower() == name for q in getdata_ui.queries(self.book)):
            return [None, ("Refresh", "refresh")]
        return []

    def _note_items(self):
        g = self.grid
        r, c = g.sel.active
        if self.book is not None and self.book.note(g.sheet, r, c):
            return [("Edit Note", "edit-note"), ("Delete Note", "delete-note"), ("Show/Hide Note", "show-hide-note")]
        if self.book is not None and self.book.thread(g.sheet, r, c):
            return [("Delete Comment", "delete-comment")]
        return [("New Note", "new-note")]

    def _popup_menu(self, items, ev):
        menu = Gtk.Menu()
        menu.get_style_context().add_class("folio-menu")
        for it in items:
            if it is None:
                menu.append(Gtk.SeparatorMenuItem())
                continue
            mi = Gtk.MenuItem(label=it[0])
            mi.connect("activate", lambda _m, c=it[1]: GLib.idle_add(lambda: (self.run(c), False)[1]))
            menu.append(mi)
        menu.show_all()
        menu.attach_to_widget(self, None)
        menu.popup_at_pointer(ev)
        self._last_menu = items

    # ------------------------------------------------------------ number formats
    ACCOUNTING = {
        "$": '_("$"* #,##0.00_);_("$"* \\(#,##0.00\\);_("$"* "-"??_);_(@_)',
        "£": '_-[$£-809]* #,##0.00_-;\\-[$£-809]* #,##0.00_-;_-[$£-809]* "-"??_-;_-@_-',
        "€": '_-[$€-x-euro2] * #,##0.00_-;\\-[$€-x-euro2] * #,##0.00_-;_-[$€-x-euro2] * "-"??_-;_-@_-',
    }
    COMMA = '_(* #,##0.00_);_(* \\(#,##0.00\\);_(* "-"??_);_(@_)'

    def apply_number_format(self, code):
        g = self.grid
        if g.editing():
            g.commit_edit()
        self.book.set_number_format(g.sheet, g.sel.ranges, code)
        g.refresh()

    def act_number_format(self, name):
        from .ribbon_defs import NUMBER_FORMATS
        if name == "More Number Formats...":
            return self.run("number-dialog")
        code = dict(NUMBER_FORMATS).get(name)
        if code is None:
            return
        self.apply_number_format(code)

    def act_accounting(self, cur="$"):
        self.apply_number_format(self.ACCOUNTING.get(cur, self.ACCOUNTING["$"]))

    def act_percent(self):
        self.apply_number_format("0%")

    def act_comma(self):
        self.apply_number_format(self.COMMA)

    def _decimals(self, d):
        g = self.grid
        r, c = g.sel.active
        self.book.adjust_decimals(g.sheet, g.sel.ranges, r, c, d)
        g.refresh()

    def act_decimal_increase(self):
        self._decimals(1)

    def act_decimal_decrease(self):
        self._decimals(-1)

    # ------------------------------------------------------------ sizes
    def _sel_cols(self):
        """Columns of the selection (whole selected columns, or the columns
        the selected cells are in)."""
        out = []
        for r1, c1, r2, c2 in self.grid.sel.ranges:
            out.append((c1, c2))
        return out

    def _sel_rows(self):
        return [(r1, r2) for r1, c1, r2, c2 in self.grid.sel.ranges]

    def _whole_cols_containing(self, c):
        for r1, c1, r2, c2 in self.grid.sel.ranges:
            if r1 == 0 and r2 == MAX_ROWS - 1 and c1 <= c <= c2:
                return [(a, b) for (q1, a, q2, b) in self.grid.sel.ranges if q1 == 0 and q2 == MAX_ROWS - 1]
        return [(c, c)]

    def _whole_rows_containing(self, r):
        for r1, c1, r2, c2 in self.grid.sel.ranges:
            if c1 == 0 and c2 == MAX_COLS - 1 and r1 <= r <= r2:
                return [(a, b) for (a, q1, b, q2) in self.grid.sel.ranges if q1 == 0 and q2 == MAX_COLS - 1]
        return [(r, r)]

    def resize_header(self, drag, x, y):
        """End of a drag on a column or row border."""
        g = self.grid
        kind, idx, x0, y0 = drag[:4]
        from .protect_ui import protected, allows, refuse
        if protected(self) and not allows(self, "formatColumns" if kind == "colresize" else "formatRows"):
            self.size_tip(None)
            refuse(self)
            return
        z = g.state().zoom
        self.size_tip(None)
        if kind == "colresize":
            px = self.book.col_px(g.sheet, idx) + (x - x0) / z
            for a, b in self._whole_cols_containing(idx):
                self.book.set_col_px(g.sheet, a, b, max(0, round(px)))
        else:
            px = self.book.row_px(g.sheet, idx) + (y - y0) / z
            for a, b in self._whole_rows_containing(idx):
                self.book.set_row_px(g.sheet, a, b, max(0, round(px)))
        g.refresh()
        g.place_editor()

    def resize_feedback(self, drag, x, y):
        """While dragging a border: the size as a tip by the pointer."""
        g = self.grid
        kind, idx, x0, y0 = drag[:4]
        z = g.state().zoom
        if kind == "colresize":
            px = max(0, round(self.book.col_px(g.sheet, idx) + (x - x0) / z))
            w = self.book.px_to_width(g.sheet, px)
            self.size_tip("Width: %.2f (%d pixels)" % (w, px))
        else:
            px = max(0, round(self.book.row_px(g.sheet, idx) + (y - y0) / z))
            self.size_tip("Height: %.2f (%d pixels)" % (px * 0.75, px))

    def size_tip(self, text):
        a = self.assist
        if text is None:
            a.tip.hide()
            return
        a.tip_label.set_text(text)
        _screen, px, py = self.get_display().get_default_seat().get_pointer().get_position()
        a.tip.move(px + 14, py - 30)
        a.tip.show_all()

    def _ask_number(self, title, label, value, lo, hi):
        d = Gtk.Dialog(title=title, transient_for=self, modal=True)
        d.get_style_context().add_class("folio-dialog")
        d.set_resizable(False)
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_response(Gtk.ResponseType.OK)
        box = d.get_content_area()
        box.set_border_width(10)
        row = Gtk.Box(spacing=8)
        row.pack_start(Gtk.Label(label=label), False, False, 0)
        e = Gtk.Entry()
        e.set_width_chars(10)
        e.set_text(("%.2f" % value).rstrip("0").rstrip(".") if value is not None else "")
        e.set_activates_default(True)
        row.pack_start(e, False, False, 0)
        box.pack_start(row, False, False, 0)
        d.show_all()
        while True:
            r = d.run()
            if r != Gtk.ResponseType.OK:
                d.destroy()
                return None
            try:
                v = float(e.get_text().strip())
                if lo <= v <= hi:
                    d.destroy()
                    return v
            except ValueError:
                pass
            self.dialogs.message("Enter a number between %g and %g." % (lo, hi), title)

    def act_column_width(self):
        g = self.grid
        cols = self._sel_cols()
        widths = {self.book.col_width(g.sheet, c) for a, b in cols for c in range(a, min(b, a + 200) + 1)}
        v = self._ask_number("Column Width", "Column width:", widths.pop() if len(widths) == 1 else None, 0, 255)
        if v is None:
            return
        for a, b in cols:
            self.book.set_col_width(g.sheet, a, b, v)
        g.refresh()

    def act_row_height(self):
        g = self.grid
        rows = self._sel_rows()
        hs = {self.book.row_height(g.sheet, r) for a, b in rows for r in range(a, min(b, a + 200) + 1)}
        v = self._ask_number("Row Height", "Row height:", hs.pop() if len(hs) == 1 else None, 0, 409)
        if v is None:
            return
        for a, b in rows:
            self.book.set_row_height(g.sheet, a, b, v)
        g.refresh()

    def act_default_width(self):
        g = self.grid
        v = self._ask_number("Standard Width", "Standard column width:", self.book.default_col_width(g.sheet), 0, 255)
        if v is not None:
            self.book.set_default_col_width(g.sheet, v)
            g.refresh()

    def act_autofit_columns(self):
        g = self.grid
        for r1, c1, r2, c2 in g.sel.ranges:
            whole = r1 == 0 and r2 == MAX_ROWS - 1
            self.book.autofit_cols(g.sheet, c1, c2, None if whole else (r1, r2))
        g.refresh()

    def act_autofit_rows(self):
        g = self.grid
        for r1, c1, r2, c2 in g.sel.ranges:
            self.book.autofit_rows(g.sheet, r1, r2)
        g.refresh()

    def act_autofit_column_at(self, c):
        g = self.grid
        for a, b in self._whole_cols_containing(int(c)):
            self.book.autofit_cols(g.sheet, a, b)
        g.refresh()

    def act_autofit_row_at(self, r):
        g = self.grid
        for a, b in self._whole_rows_containing(int(r)):
            self.book.autofit_rows(g.sheet, a, b)
        g.refresh()

    def act_hide_columns(self):
        for a, b in self._sel_cols():
            self.book.set_cols_hidden(self.grid.sheet, a, b, True)
        self.grid.refresh()

    def act_unhide_columns(self):
        for a, b in self._sel_cols():
            self.book.set_cols_hidden(self.grid.sheet, a, b, False)
        self.grid.refresh()

    def act_hide_rows(self):
        for a, b in self._sel_rows():
            self.book.set_rows_hidden(self.grid.sheet, a, b, True)
        self.grid.refresh()

    def act_unhide_rows(self):
        for a, b in self._sel_rows():
            self.book.set_rows_hidden(self.grid.sheet, a, b, False)
        self.grid.refresh()

    # ------------------------------------------------------------ sheets
    def _remap_states(self, mapping):
        """Per-sheet window state follows sheets to their new indexes;
        mapping: old index -> new index (None: gone)."""
        g = self.grid
        g.states = {mapping[i]: st for i, st in g.states.items() if mapping.get(i) is not None}
        self.show_formulas = {mapping[i] for i in self.show_formulas if mapping.get(i) is not None}
        self.arrows = {mapping[i]: a for i, a in self.arrows.items() if mapping.get(i) is not None}
        self.dv.circles = {mapping[i]: a for i, a in self.dv.circles.items() if mapping.get(i) is not None}

    def _after_structure(self, select):
        g = self.grid
        g.sheet = min(select, self.book.sheet_count - 1)
        self.book.active_sheet = g.sheet
        self.tabs.rebuild()
        self.sheet_changed()
        self.update_title()

    # ------------------------------------------------------------ fill
    def fill_handle(self, src, target, ctrl=False):
        """The fill handle was dragged from src over target (Ctrl: copy instead of a
        series; a single number counts up instead)."""
        from .protect_ui import may_change_cells
        if not may_change_cells(self, [tuple(target)]):
            return
        g = self.grid
        b = self.book
        src, target = tuple(src), tuple(target)
        try:
            inside = target[0] >= src[0] and target[1] >= src[1] and target[2] <= src[2] and target[3] <= src[3]
            if inside:
                cut = (target[2] + 1, src[1], src[2], src[3]) if target[2] < src[2] else (src[0], target[3] + 1, src[2], src[3])
                b.clear_contents(g.sheet, [cut], "AutoFill")
            else:
                single = src[0:2] == src[2:4]
                v = b.value(g.sheet, src[0], src[1])
                if ctrl and single and isinstance(v, float) and not b.is_formula(g.sheet, src[0], src[1]):
                    vertical = target[0] != src[0] or target[2] != src[2]
                    if target[0] < src[0] or target[1] < src[1]:
                        b.autofill(g.sheet, src, target, False)
                    else:
                        b.fill_series(g.sheet, target, not vertical, "linear", "day", 1.0)
                else:
                    b.autofill(g.sheet, src, target, not ctrl)
        except ValueError as e:
            self.dialogs.message(str(e))
            return
        g.sel.ranges = [target]
        g.refresh()
        self.update_title()

    def act_fill_series(self):
        from . import fill_ui
        fill_ui.series_dialog(self)

    def act_fill_justify(self):
        """Fill ▸ Justify: the text of the selection's first column, rewrapped to the
        selection's width, one line per cell."""
        import textwrap
        g = self.grid
        b = self.book
        r1, c1, r2, c2 = g.sel.ranges[-1]
        ext = b.used_extent(g.sheet) or (r1, c1)
        r2 = min(r2, max(ext[0], r1))
        words = " ".join(b.display(g.sheet, r, c1) for r in range(r1, r2 + 1) if b.display(g.sheet, r, c1)).split()
        if not words:
            return
        if any(b.is_formula(g.sheet, r, c1) or isinstance(b.value(g.sheet, r, c1), float) for r in range(r1, r2 + 1)):
            self.dialogs.message("Cannot justify numbers or formulas.")
            return
        width_px = sum(b.col_px(g.sheet, c) for c in range(c1, c2 + 1))
        chars = max(1, int((width_px - 5) / b.mdw(g.sheet) * 1.1))
        lines = textwrap.wrap(" ".join(words), chars)
        with b.group("Fill Justify"):
            for i, r in enumerate(range(r1, max(r2, r1 + len(lines) - 1) + 1)):
                b.set_input(g.sheet, r, c1, lines[i] if i < len(lines) else "")
        g.refresh()
        self.update_title()

    # ------------------------------------------------------------ page layout and printing
    def act_print(self):
        self.show_backstage("print")

    def act_export_pdf(self):
        from . import print_ui
        print_ui.export_pdf(self)

    def act_margins(self, name=None):
        from . import print_ui
        if name in print_ui.MARGINS:
            print_ui.margins(self, name)
        else:
            print_ui.page_setup_dialog(self, "margins")

    def act_page_orientation(self, which=None):
        from . import print_ui
        print_ui.orientation(self, which or "portrait")

    def act_page_size(self, name=None):
        from . import print_ui
        if name:
            print_ui.size(self, name)
        else:
            print_ui.page_setup_dialog(self, "page")

    def act_print_area(self, how=None):
        from . import print_ui
        print_ui.print_area(self, how or "set")

    def act_breaks(self, how=None):
        from . import print_ui
        print_ui.breaks(self, how or "insert")

    def act_print_titles(self):
        from . import print_ui
        print_ui.page_setup_dialog(self, "sheet")

    def act_scale_width(self, n=None):
        from . import print_ui
        print_ui.scale_fit(self, "width", int(n or 0))

    def act_scale_height(self, n=None):
        from . import print_ui
        print_ui.scale_fit(self, "height", int(n or 0))

    def act_scale(self):
        from . import print_ui
        print_ui.page_setup_dialog(self, "page")

    def act_print_gridlines(self):
        from . import print_ui
        print_ui.toggle_print_option(self, "gridlines")

    def act_print_headings(self):
        from . import print_ui
        print_ui.toggle_print_option(self, "headings")

    def act_header_footer(self):
        from . import print_ui
        print_ui.page_setup_dialog(self, "header")

    def act_page_setup_dialog(self):
        from . import print_ui
        print_ui.page_setup_dialog(self)

    def act_view_page_break(self):
        st = self.grid.state()
        st.view_mode = "normal" if st.view_mode == "pageBreakPreview" else "pageBreakPreview"
        self.grid.refresh()

    # ------------------------------------------------------------ notes and comments
    def act_new_note(self):
        from . import notes_ui
        notes_ui.edit_note(self)

    def act_edit_note(self):
        from . import notes_ui
        notes_ui.edit_note(self)

    def act_delete_note(self):
        from . import notes_ui
        notes_ui.delete(self)

    def act_show_hide_note(self):
        from . import notes_ui
        notes_ui.show_hide(self)

    def act_notes(self):
        self.act_new_note()

    def act_show_all_notes(self):
        from . import notes_ui
        notes_ui.show_all(self)

    def act_next_note(self):
        from . import notes_ui
        notes_ui.go(self, False, True)

    def act_prev_note(self):
        from . import notes_ui
        notes_ui.go(self, True, True)

    def act_convert_notes(self):
        from . import notes_ui
        notes_ui.convert(self)

    def act_new_comment(self):
        from . import notes_ui
        notes_ui.open_card(self)

    def act_comment(self):
        self.act_new_comment()

    def act_delete_comment(self):
        from . import notes_ui
        g = self.grid
        r, c = g.sel.active
        if self.book.thread(g.sheet, r, c) is None:
            self.status.extra.set_text("There's no comment in this cell.")
            return
        self.book.edit_comment(g.sheet, r, c, 0, None)
        g.refresh()
        self.update_title()
        notes_ui.refresh_pane(self)

    def act_next_comment(self):
        from . import notes_ui
        notes_ui.go(self, False, False)

    def act_prev_comment(self):
        from . import notes_ui
        notes_ui.go(self, True, False)

    def act_show_comments(self):
        from . import notes_ui
        notes_ui.toggle_pane(self)

    def act_clear_comments(self):
        from . import notes_ui
        notes_ui.delete(self)

    # ------------------------------------------------------------ tables
    def act_format_as_table(self, style=None):
        from . import table_ui
        table_ui.create(self, style or table_ui.DEFAULT_STYLE)

    def act_table(self):
        from . import table_ui
        table_ui.create(self, table_ui.DEFAULT_STYLE)

    def act_table_convert(self):
        from . import table_ui
        i = table_ui.current(self)
        if i is None:
            return
        self.book.convert_table(self.grid.sheet, i)
        self.grid.refresh()
        self.update_title()
        self.on_selection_changed()

    def act_table_resize(self):
        from . import table_ui
        i = table_ui.current(self)
        if i is not None:
            table_ui.resize(self, i)
            self.on_selection_changed()

    def act_table_option(self, arg=None):
        from . import table_ui
        i = table_ui.current(self)
        if i is None or not arg:
            return
        key, _, val = arg.partition(":")
        t = self.book.tables(self.grid.sheet)[i]
        on = (val == "1") if val else not t.get(key, False)
        try:
            self.book.set_table_option(self.grid.sheet, i, key, on)
        except ValueError as e:
            self.dialogs.message(str(e))
        self.grid.refresh()
        self.update_title()
        self.sync_table_tab()

    def act_table_style(self, name=None):
        from . import table_ui
        i = table_ui.current(self)
        if i is None:
            return
        self.book.set_table_style(self.grid.sheet, i, None if name in (None, "none") else name)
        self.grid.refresh()
        self.update_title()

    def sync_table_tab(self):
        g = self.grid
        if self.book is None or not hasattr(self, "table_checks"):
            return
        i = self.book.table_at(g.sheet, *g.sel.active)
        self.ribbon.show_contextual("Table Design", i is not None)
        if i is None:
            return
        t = self.book.tables(g.sheet)[i]
        self.syncing_checks = True
        try:
            for key, chk in self.table_checks.items():
                chk.set_active(bool(t[key]))
            if not self.table_name.has_focus():
                self.table_name.set_text(t["name"])
        finally:
            self.syncing_checks = False

    # ------------------------------------------------------------ conditional formatting
    def act_conditional_formatting(self, arg=None):
        from . import cf_ui
        arg = arg or "manage"
        key, _, rest = arg.partition(":")
        if key in cf_ui.HIGHLIGHT:
            cf_ui.highlight(self, key)
        elif key in cf_ui.TOPBOTTOM:
            cf_ui.top_bottom(self, key)
        elif key == "bar":
            cf_ui.data_bar(self, rest)
        elif key == "scale":
            cf_ui.color_scale(self, rest)
        elif key == "icons":
            cf_ui.icon_set(self, rest)
        elif key == "new":
            cf_ui.new_rule(self)
        elif key in ("clear-sel", "clear"):
            cf_ui.clear(self, False)
        elif key == "clear-sheet":
            cf_ui.clear(self, True)
        elif key == "manage":
            cf_ui.manage(self)

    # ------------------------------------------------------------ charts
    def on_object_selection(self, switch_tab=False):
        g = self.grid
        n = len(self.book.objects(g.sheet)) if self.book is not None else 0
        g.sel_objects = [i for i in g.sel_objects if i < n]
        chart = g.selected_chart() is not None if self.book is not None else False
        self.ribbon.show_contextual("Chart Design", chart)
        if chart and switch_tab:
            t = self.ribbon.tab_by_name("Chart Design")
            if t is not None:
                self.ribbon.select(t)
        if chart:
            name = self.book.objects(g.sheet)[g.sel_objects[0]]["name"]
            self.formula_bar.namebox.show_text(name)
        g.area.queue_draw()

    def object_menu(self, ev, i):
        items = [("Cut", "chart-cut"), ("Copy", "chart-copy"), None, ("Delete", "chart-delete"), None,
                 ("Change Chart Type...", "chart-change-type"), ("Select Data...", "chart-select-data"),
                 ("Switch Row/Column", "chart-switch-row-col"), None, ("Format Chart...", "chart-format")]
        kind = self.book.objects(self.grid.sheet)[i]["kind"]
        if kind != "chart":
            items = [("Delete", "chart-delete")]
        items = [it for it in items if it is None or getattr(self, commands.method_name(it[1]), None)]
        self._popup_menu(items, ev)

    def _chart_index(self):
        i = self.grid.selected_chart()
        if i is None:
            self.status.extra.set_text("Select a chart first.")
        return i

    def _insert_chart(self, kind, page):
        from . import chart_ui
        chart_ui.insert_chart(self, kind, page)

    def act_chart_column(self, kind=None):
        self._insert_chart(kind, "Column")

    def act_chart_line(self, kind=None):
        self._insert_chart(kind, "Line")

    def act_chart_pie(self, kind=None):
        self._insert_chart(kind, "Pie")

    def act_chart_scatter(self, kind=None):
        self._insert_chart(kind, "X Y (Scatter)")

    def act_chart_combo(self, kind=None):
        self._insert_chart(kind, "Combo")

    def act_recommended_charts(self):
        self._insert_chart(None, None)

    def act_charts_dialog(self):
        self._insert_chart(None, None)

    def act_chart_change_type(self):
        from . import chart_ui
        i = self._chart_index()
        if i is not None:
            chart_ui.change_type(self, i)

    def act_chart_select_data(self):
        from . import chart_ui
        i = self._chart_index()
        if i is not None:
            chart_ui.select_data(self, i)

    def act_chart_switch_row_col(self):
        i = self._chart_index()
        if i is None:
            return
        try:
            self.book.switch_chart_rows(self.grid.sheet, i)
        except ValueError as e:
            self.dialogs.message(str(e), "Switch Row/Column")
            return
        self.grid.refresh()
        self.update_title()

    def act_chart_format(self):
        from . import chart_ui
        i = self._chart_index()
        if i is not None:
            chart_ui.format_chart(self, i)

    def act_chart_add_element(self, what=None):
        from . import chart_ui
        i = self._chart_index()
        if i is not None and what:
            chart_ui.add_element(self, i, what)

    def act_chart_colors(self, n=None):
        from . import chart_ui
        i = self._chart_index()
        if i is not None:
            chart_ui.set_colors(self, i, int(n or 0))

    def act_chart_delete(self):
        g = self.grid
        if not g.sel_objects:
            return
        self.book.delete_objects(g.sheet, list(g.sel_objects))
        g.sel_objects = []
        self.on_object_selection()
        g.refresh()
        self.update_title()

    def act_flash_fill(self):
        g = self.grid
        r, c = g.sel.active
        try:
            n = self.book.flash_fill(g.sheet, r, c)
        except ValueError as e:
            self.dialogs.message(str(e), "Flash Fill")
            return
        self.status.extra.set_text("Flash Fill changed %d cells." % n)
        g.refresh()
        self.update_title()

    # ------------------------------------------------------------ find
    def act_find(self):
        from . import find_ui
        find_ui.open_find(self, False)

    def act_replace(self):
        from . import find_ui
        find_ui.open_find(self, True)

    def act_goto_special(self):
        from . import find_ui
        find_ui.goto_special(self)

    def act_select_constants(self):
        from . import find_ui
        find_ui.goto_special(self, "constants")

    def act_select_formulas(self):
        from . import find_ui
        find_ui.goto_special(self, "formulas")

    def act_select_cf(self):
        from . import find_ui
        find_ui.goto_special(self, "cf")

    def act_select_validation(self):
        from . import find_ui
        find_ui.goto_special(self, "validation")

    # ------------------------------------------------------------ sort & filter
    def act_sort_asc(self):
        from . import data_ui
        data_ui.sort_quick(self, True)

    def act_sort_desc(self):
        from . import data_ui
        data_ui.sort_quick(self, False)

    def act_sort_custom(self):
        from . import data_ui
        data_ui.sort_custom(self)

    def act_filter(self):
        from . import data_ui
        data_ui.toggle_filter(self)

    def act_filter_clear(self):
        from . import data_ui
        data_ui.clear_filter(self)

    def act_filter_reapply(self):
        from . import data_ui
        data_ui.reapply_filter(self)

    def act_group(self, arg=None):
        from . import outline_ui
        outline_ui.group(self, 1, arg)

    def act_ungroup(self, arg=None):
        from . import outline_ui
        outline_ui.group(self, -1, arg)

    def act_show_detail(self):
        from . import outline_ui
        outline_ui.detail(self, True)

    def act_hide_detail(self):
        from . import outline_ui
        outline_ui.detail(self, False)

    def act_subtotal(self):
        from . import outline_ui
        outline_ui.subtotal(self)

    def act_spelling(self):
        from . import proofing
        proofing.spelling(self)

    def act_thesaurus(self):
        from . import proofing
        proofing.thesaurus(self)

    def act_text_to_columns(self):
        from . import text_ui
        text_ui.text_to_columns(self)

    def act_from_text_csv(self):
        from . import getdata_ui
        getdata_ui.from_text_csv(self)

    def act_from_workbook(self):
        from . import getdata_ui
        getdata_ui.from_workbook(self)

    def act_from_folder(self):
        from . import getdata_ui
        getdata_ui.from_folder(self)

    def act_get_data_unavailable(self, what="This source"):
        from . import getdata_ui
        getdata_ui.not_available(self, what)

    def act_refresh_all(self):
        from . import getdata_ui
        getdata_ui.refresh_all(self)

    def act_refresh(self):
        from . import getdata_ui
        getdata_ui.refresh_current(self)

    def act_queries_connections(self):
        from . import getdata_ui
        getdata_ui.toggle_pane(self)

    def act_existing_connections(self):
        from . import getdata_ui
        getdata_ui.toggle_pane(self, True)

    def act_from_text_legacy(self):
        d = Gtk.FileChooserNative.new("Import Text File", self, Gtk.FileChooserAction.OPEN, "Import", "Cancel")
        f = Gtk.FileFilter()
        f.set_name("Text Files (*.prn; *.txt; *.csv; *.tsv)")
        for p in ("*.prn", "*.txt", "*.csv", "*.tsv", "*.PRN", "*.TXT", "*.CSV", "*.TSV"):
            f.add_pattern(p)
        d.add_filter(f)
        r = d.run()
        path = d.get_filename()
        d.destroy()
        if r != Gtk.ResponseType.ACCEPT or not path:
            return
        from . import text_ui
        text_ui.import_into(self, path)

    def act_remove_duplicates(self):
        from . import data_ui
        data_ui.remove_duplicates(self)

    def act_data_validation(self, arg=None):
        if arg == "circle":
            self.dv.circle()
        elif arg == "clear-circles":
            self.dv.clear_circles()
        else:
            self.dv.dialog()

    def act_pick_from_list(self):
        self.dv.pick_from_list()

    # ------------------------------------------------------------ insert / delete cells
    def _struct_ranges(self, rows):
        """The selection's row (or column) spans, last first so earlier ones don't move."""
        g = self.grid
        spans = sorted({(rg[0], rg[2]) if rows else (rg[1], rg[3]) for rg in g.sel.ranges}, reverse=True)
        return spans

    def _struct(self, label, fn):
        g = self.grid
        if g.editing() and g.commit_edit() is False:
            return False
        if self.book.sheet_info(g.sheet)["kind"] != "work":
            self.dialogs.message("This command works on worksheets only.")
            return False
        try:
            with self.book.group(label):
                fn()
        except ValueError as e:
            self.dialogs.message(str(e))
            return False
        g.copy_range = None
        g.refresh()
        self.update_title()
        return True

    def act_insert_rows(self):
        g = self.grid
        self._struct("Insert Rows", lambda: [self.book.insert_rows(g.sheet, a, b - a + 1)
                                               for a, b in self._struct_ranges(True)])

    def act_delete_rows(self):
        g = self.grid
        self._struct("Delete Rows", lambda: [self.book.delete_rows(g.sheet, a, b - a + 1)
                                               for a, b in self._struct_ranges(True)])

    def act_insert_columns(self):
        g = self.grid
        self._struct("Insert Columns", lambda: [self.book.insert_cols(g.sheet, a, b - a + 1)
                                                  for a, b in self._struct_ranges(False)])

    def act_delete_columns(self):
        g = self.grid
        self._struct("Delete Columns", lambda: [self.book.delete_cols(g.sheet, a, b - a + 1)
                                                  for a, b in self._struct_ranges(False)])

    def _cells_choice(self, title, options):
        """Insert/Delete dialog: one of options [(label, key)]; None when cancelled."""
        d = Gtk.Dialog(title=title, transient_for=self, modal=True)
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        d.set_default_response(Gtk.ResponseType.OK)
        box = d.get_content_area()
        box.set_border_width(12)
        box.set_spacing(6)
        box.pack_start(Gtk.Label(label=title.rstrip(".") + ":", xalign=0), False, False, 0)
        group = None
        buttons = []
        for label, key in options:
            b = Gtk.RadioButton.new_with_mnemonic_from_widget(group, label)
            group = group or b
            buttons.append((b, key))
            box.pack_start(b, False, False, 0)
        d.show_all()
        r = d.run()
        choice = next((k for b, k in buttons if b.get_active()), None)
        d.destroy()
        return choice if r == Gtk.ResponseType.OK else None

    @staticmethod
    def _whole_rows(rg):
        return rg[1] == 0 and rg[3] >= MAX_COLS - 1

    @staticmethod
    def _whole_cols(rg):
        return rg[0] == 0 and rg[2] >= MAX_ROWS - 1

    def act_insert_cells(self):
        g = self.grid
        rgs = list(g.sel.ranges)
        if all(self._whole_rows(r) for r in rgs):
            return self.act_insert_rows()
        if all(self._whole_cols(r) for r in rgs):
            return self.act_insert_columns()
        if len(rgs) > 1:
            self.dialogs.message("This command cannot be used on multiple selections.")
            return
        how = self._cells_choice("Insert", [("Shift cells _right", "right"), ("Shift cells _down", "down"),
                                            ("Entire _row", "row"), ("Entire _column", "col")])
        if how == "row":
            return self.act_insert_rows()
        if how == "col":
            return self.act_insert_columns()
        if how:
            self._struct("Insert Cells", lambda: self.book.insert_cells(g.sheet, tuple(rgs[0]), how == "down"))

    def act_delete_cells(self):
        g = self.grid
        rgs = list(g.sel.ranges)
        if all(self._whole_rows(r) for r in rgs):
            return self.act_delete_rows()
        if all(self._whole_cols(r) for r in rgs):
            return self.act_delete_columns()
        if len(rgs) > 1:
            self.dialogs.message("This command cannot be used on multiple selections.")
            return
        how = self._cells_choice("Delete", [("Shift cells _left", "left"), ("Shift cells _up", "up"),
                                            ("Entire _row", "row"), ("Entire _column", "col")])
        if how == "row":
            return self.act_delete_rows()
        if how == "col":
            return self.act_delete_columns()
        if how:
            self._struct("Delete Cells", lambda: self.book.delete_cells(g.sheet, tuple(rgs[0]), how == "up"))

    def act_insert_sheet(self):
        g = self.grid
        at = g.sheet if self.app.settings.get("insert_sheet_before", False) else g.sheet + 1
        n = self.book.sheet_count
        i = self.book.insert_sheet(at)
        self._remap_states({k: (k + 1 if k >= at else k) for k in range(n)})
        self._after_structure(i)

    def act_rename_sheet(self):
        g = self.grid
        cur = self.book.sheet_info(g.sheet)["name"]
        while True:
            t = self.dialogs.ask_text("Rename Sheet", "Sheet name:", cur)
            if t is None or t == cur:
                return
            err = self.book.check_sheet_name(t, g.sheet)
            if err:
                self.dialogs.message(err)
                cur = t
                continue
            self.book.rename_sheet(g.sheet, t)
            self._after_structure(g.sheet)
            return

    def act_delete_sheet(self):
        g = self.grid
        i = g.sheet
        if self.book.has_data(i):
            d = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.WARNING,
                                  buttons=Gtk.ButtonsType.NONE,
                                  text="This sheet will be permanently deleted. Do you want to continue?")
            d.add_button("Delete", Gtk.ResponseType.OK)
            d.add_button("Cancel", Gtk.ResponseType.CANCEL)
            r = d.run()
            d.destroy()
            if r != Gtk.ResponseType.OK:
                return
        n = self.book.sheet_count
        try:
            self.book.delete_sheet(i)
        except ValueError as e:
            self.dialogs.message(str(e))
            return
        self._remap_states({k: (None if k == i else (k - 1 if k > i else k)) for k in range(n)})
        vis = [k for k in range(self.book.sheet_count) if self.book.sheet_info(k)["state"] == "visible"]
        nxt = min((k for k in vis if k >= i), default=vis[-1] if vis else 0)
        self._after_structure(nxt)

    def act_hide_sheet(self):
        g = self.grid
        try:
            self.book.set_sheet_state(g.sheet, "hidden")
        except ValueError as e:
            self.dialogs.message(str(e))
            return
        vis = [k for k in range(self.book.sheet_count) if self.book.sheet_info(k)["state"] == "visible"]
        nxt = min((k for k in vis if k > g.sheet), default=vis[-1])
        self._after_structure(nxt)

    def act_unhide_sheet(self):
        hidden = [(k, self.book.sheet_info(k)["name"]) for k in range(self.book.sheet_count)
                  if self.book.sheet_info(k)["state"] == "hidden"]
        if not hidden:
            return
        d = Gtk.Dialog(title="Unhide", transient_for=self, modal=True)
        d.get_style_context().add_class("folio-dialog")
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        box = d.get_content_area()
        box.set_border_width(10)
        box.pack_start(Gtk.Label(label="Unhide sheet:", xalign=0), False, False, 0)
        store = Gtk.ListStore(int, str)
        for k, n in hidden:
            store.append([k, n])
        tv = Gtk.TreeView(model=store)
        tv.set_headers_visible(False)
        tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=1))
        tv.get_selection().set_mode(Gtk.SelectionMode.MULTIPLE)
        tv.get_selection().select_path(Gtk.TreePath(0))
        tv.connect("row-activated", lambda *_: d.response(Gtk.ResponseType.OK))
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(240, 140)
        sw.add(tv)
        box.pack_start(sw, True, True, 0)
        d.show_all()
        r = d.run()
        m, paths = tv.get_selection().get_selected_rows()
        picked = [m[p][0] for p in paths]
        d.destroy()
        if r != Gtk.ResponseType.OK or not picked:
            return
        for k in picked:
            self.book.set_sheet_state(k, "visible")
        self._after_structure(picked[0])

    def act_tab_color(self, rgb=None):
        if rgb is not None:
            self.book.set_tab_color(self.grid.sheet, None if rgb == "none" else int(rgb, 16))
            self.tabs.rebuild()
            return
        from .ribbon_defs import color_palette
        pop = Gtk.Popover.new(self.tabs.buttons[self.grid.sheet] or self.tabs)
        ref = [pop]

        def pick(c):
            self.book.set_tab_color(self.grid.sheet, c)
            self.tabs.rebuild()
        pop.add(color_palette(pick, none_label="No Color", popref=ref))
        pop.show_all()
        pop.popup()

    def act_move_copy_sheet(self):
        g = self.grid
        n = self.book.sheet_count
        d = Gtk.Dialog(title="Move or Copy", transient_for=self, modal=True)
        d.get_style_context().add_class("folio-dialog")
        d.add_button("OK", Gtk.ResponseType.OK)
        d.add_button("Cancel", Gtk.ResponseType.CANCEL)
        box = d.get_content_area()
        box.set_border_width(10)
        box.set_spacing(6)
        box.pack_start(Gtk.Label(label="Move selected sheets\nTo book:  %s" % self.doc_name(), xalign=0),
                       False, False, 0)
        box.pack_start(Gtk.Label(label="Before sheet:", xalign=0), False, False, 0)
        store = Gtk.ListStore(int, str)
        for k in range(n):
            store.append([k, self.book.sheet_info(k)["name"]])
        store.append([n, "(move to end)"])
        tv = Gtk.TreeView(model=store)
        tv.set_headers_visible(False)
        tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=1))
        tv.get_selection().select_path(Gtk.TreePath(min(g.sheet + 1, n)))
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(260, 160)
        sw.add(tv)
        box.pack_start(sw, True, True, 0)
        copy = Gtk.CheckButton(label="Create a copy")
        copy.set_sensitive(False)
        copy.set_tooltip_text("Copying sheets is not available yet.")
        box.pack_start(copy, False, False, 0)
        tv.connect("row-activated", lambda *_: d.response(Gtk.ResponseType.OK))
        d.show_all()
        r = d.run()
        m, it = tv.get_selection().get_selected()
        before = m[it][0] if it is not None else None
        d.destroy()
        if r != Gtk.ResponseType.OK or before is None:
            return
        frm = g.sheet
        to = before if before < frm else before - 1
        if to == frm:
            return
        self.book.move_sheet(frm, to)

        def mp(k):
            if k == frm:
                return to
            j = k - 1 if k > frm else k
            return j + 1 if j >= to else j
        self._remap_states({k: mp(k) for k in range(n)})
        self._after_structure(to)

    def act_context_menu(self):
        g = self.grid
        r, c = g.sel.active
        rect = self.book.cell_rect(g.sheet, r, c, g.params())
        ev = Gdk.Event.new(Gdk.EventType.BUTTON_PRESS)
        ev.window = g.area.get_window()
        if rect:
            ev.x, ev.y = rect[0] + rect[2] / 2, rect[1] + rect[3] / 2
        ev.button = 3
        self.context_menu(ev, ("cell", r, c))

    def on_search(self, entry):
        text = entry.get_text().strip().lower()
        entry.set_text("")
        if not text:
            return
        for cmd, label in commands.LABELS.items():
            if text in label.lower() and hasattr(self, commands.method_name(cmd)):
                self.run(cmd)
                return
        self.status.extra.set_text("No command matches “%s”." % text)

    # ------------------------------------------------------------ keys
    def on_key_release(self, w, ev):
        name = (Gdk.keyval_name(ev.keyval) or "").lower()
        if name in ("alt_l", "alt_r") and self._alt_alone and not self.keytips.active \
                and not self.backstage_open():
            self._alt_alone = False
            self.keytips.start()
            return True
        return False

    def on_key(self, w, ev):
        kv = ev.keyval
        state = ev.get_state() & Gtk.accelerator_get_default_mod_mask()
        ctrl = bool(state & Gdk.ModifierType.CONTROL_MASK)
        shift = bool(state & Gdk.ModifierType.SHIFT_MASK)
        alt = bool(state & Gdk.ModifierType.MOD1_MASK)
        name = (Gdk.keyval_name(kv) or "").lower()
        self._alt_alone = name in ("alt_l", "alt_r")
        if self.keytips.active:
            return self.keytips.key(ev)
        if kv == Gdk.KEY_Escape and self.ink.active() and not self.grid.editing():
            self.ink.set_tool(None)
            return True
        if kv == Gdk.KEY_Escape and getattr(self, "border_mode", None) and not self.grid.editing():
            self.set_border_mode(None)
            return True
        if self.backstage_open():
            if kv == Gdk.KEY_Escape:
                self.backstage.close()
                return True
            return False
        if kv == Gdk.KEY_F10 and not shift:
            self.keytips.start()
            return True
        if kv == Gdk.KEY_F7 and not ctrl and not alt and not self.grid.editing():
            self.run("thesaurus" if shift else "spelling")      # F7, Shift+F7
            return True
        if shift and kv == Gdk.KEY_F2 and not self.grid.editing():
            self.run("new-note")
            return True
        if alt and kv == Gdk.KEY_F8 and not ctrl:
            self.run("macros")                                   # Alt+F8: the Macro dialog
            return True
        if alt and kv == Gdk.KEY_F11 and not ctrl:
            self.run("visual-basic")                             # Alt+F11: the VBA editor
            return True
        if alt and kv == Gdk.KEY_F1 and not self.grid.editing():
            self.run("chart-column:col")
            return True
        if alt and not ctrl and kv < 0x10000 and chr(kv).isalnum() and not self.grid.editing():
            if self.keytips.start_with(chr(Gdk.keyval_to_upper(kv)).upper()):
                return True
        g = self.grid
        focus = self.get_focus()
        in_grid = focus is g.area
        if in_grid and g.sel_objects and not g.editing():
            if kv in (Gdk.KEY_Delete, Gdk.KEY_BackSpace):
                self.run("chart-delete")
                return True
            if kv == Gdk.KEY_Escape:
                g.deselect_objects()
                self.on_object_selection()
                return True
            arrows = {Gdk.KEY_Left: (-1, 0), Gdk.KEY_Right: (1, 0), Gdk.KEY_Up: (0, -1), Gdk.KEY_Down: (0, 1)}
            if kv in arrows and not ctrl:
                dx, dy = arrows[kv]
                step = 1.0 if alt else 8.0
                p = g.params()
                for i in list(g.sel_objects):
                    r = self.book.object_screen_rect(g.sheet, i, p)
                    if r:
                        kind = self.book.objects(g.sheet)[i]["kind"]
                        label = {"chart": "Move Chart", "picture": "Move Picture"}.get(kind, "Move Object")
                        self.book.place_object(g.sheet, i, r[0] + dx * step, r[1] + dy * step, r[2], r[3], p, label)
                g.refresh()
                self.update_title()
                return True
            if not ctrl and not alt and kv not in (Gdk.KEY_Shift_L, Gdk.KEY_Shift_R, Gdk.KEY_Tab):
                # anything else goes back to the cells
                g.deselect_objects()
                self.on_object_selection()
        # global shortcuts
        if alt and shift and not ctrl and kv in (Gdk.KEY_Right, Gdk.KEY_Left) and in_grid and not g.editing():
            self.run("group" if kv == Gdk.KEY_Right else "ungroup")     # Alt+Shift+Right / Left
            return True
        if alt and not ctrl and kv in (Gdk.KEY_Down, Gdk.KEY_KP_Down) and in_grid and not g.editing() \
                and not g.sel_objects:
            self.run("pick-from-list")
            return True
        if ctrl and alt and not g.editing() and kv in (Gdk.KEY_v, Gdk.KEY_V):
            self.run("paste-special")
            return True
        if ctrl and shift and not alt and not g.editing() and in_grid and kv in (Gdk.KEY_v, Gdk.KEY_V):
            self.run("paste-values")
            return True
        if ctrl and not alt:
            ch = chr(Gdk.keyval_to_lower(kv)) if kv < 0x10000 else ""
            table = {"s": "save", "o": "open", "n": "new", "w": "close", "p": "print", "z": "undo",
                     "y": "redo", "f": "find", "h": "replace", "g": "goto", "1": "format-cells",
                     "b": "bold", "i": "italic", "u": "underline", "5": "strikethrough", "k": "link",
                     "t": "table", "l": "table", "e": "flash-fill", "q": "quick-analysis"}
            if not g.editing() and in_grid:
                table.update({"c": "copy", "x": "cut", "v": "paste", "a": "select-all", "d": "fill-down",
                              "r": "fill-right"})
            if shift and ch == "u":
                self.formula_bar.toggle_expand()
                return True
            # Ctrl+Shift+~ ! @ # $ % ^ : number format shortcuts
            if shift and not g.editing():
                quick = {Gdk.KEY_asciitilde: "General", Gdk.KEY_exclam: "#,##0.00", Gdk.KEY_at: "h:mm AM/PM",
                         Gdk.KEY_numbersign: "d-mmm-yy", Gdk.KEY_dollar: '"$"#,##0.00_);[Red]\\("$"#,##0.00\\)',
                         Gdk.KEY_percent: "0%", Gdk.KEY_asciicircum: "0.00E+00"}
                if kv in quick:
                    self.apply_number_format(quick[kv])
                    return True
            if ch in ("f", "p") and shift:
                self.run("font-dialog")
                return True
            if ch == "l" and shift and not g.editing():
                self.run("filter")
                return True
            # Ctrl++ insert cells, Ctrl+- delete cells
            if not g.editing() and kv in (Gdk.KEY_plus, Gdk.KEY_KP_Add) or (shift and kv == Gdk.KEY_equal and not g.editing()):
                self.run("insert-cells")
                return True
            if not g.editing() and not shift and kv in (Gdk.KEY_minus, Gdk.KEY_KP_Subtract):
                self.run("delete-cells")
                return True
            # Ctrl+Shift+& outline border, Ctrl+Shift+_ no border
            if shift and not g.editing() and kv in (Gdk.KEY_ampersand, Gdk.KEY_7):
                self.run("border-outside")
                return True
            if shift and not g.editing() and kv in (Gdk.KEY_underscore, Gdk.KEY_minus):
                self.run("border-none")
                return True
            if kv == Gdk.KEY_F4:
                self.run("close")
                return True
            if ch in table and (not g.editing() or ch in ("s", "z", "y")):
                self.run(table[ch])
                return True
            if kv in (Gdk.KEY_Page_Down, Gdk.KEY_Page_Up) and not g.editing():
                order = self.tabs.visible_order()
                if g.sheet in order:
                    i = order.index(g.sheet) + (1 if kv == Gdk.KEY_Page_Down else -1)
                    if 0 <= i < len(order):
                        g.set_sheet(order[i])
                        self.sheet_changed()
                return True
        if kv == Gdk.KEY_F9 and not g.editing():
            self.run("calc-full" if (ctrl and alt) else ("calc-sheet" if shift else "calc-now"))
            return True
        if kv == Gdk.KEY_F3 and not alt:
            if shift and not ctrl:
                self.run("insert-function")
            elif ctrl and shift:
                self.run("names-from-selection")
            elif ctrl:
                self.run("name-manager")
            else:
                self.run("use-in-formula")
            return True
        if ctrl and kv in (Gdk.KEY_grave, Gdk.KEY_dead_grave) and not g.editing():
            self.run("show-formulas")
            return True
        if alt and kv == Gdk.KEY_equal and not ctrl:
            self.run("autosum")
            return True
        if kv == Gdk.KEY_F5 and shift and not g.editing():
            self.run("find")
            return True
        if kv == Gdk.KEY_F4 and shift and not ctrl and not g.editing():
            fd = getattr(self, "find_dialog", None)
            if fd is not None and fd.what.get_text():
                fd.find_next()
            else:
                self.run("find")
            return True
        if kv == Gdk.KEY_F5 and not g.editing():
            self.run("goto")
            return True
        if kv == Gdk.KEY_F11 and shift:
            self.run("insert-sheet")
            return True
        if kv == Gdk.KEY_F10 and shift and in_grid:
            self.run("context-menu")
            return True
        if kv == Gdk.KEY_Menu and in_grid:
            self.run("context-menu")
            return True
        if in_grid and not g.editing():
            return self.grid_key(ev, kv, ctrl, shift, alt)
        return False

    def grid_key(self, ev, kv, ctrl, shift, alt):
        g = self.grid
        arrows = {Gdk.KEY_Up: (-1, 0), Gdk.KEY_Down: (1, 0), Gdk.KEY_Left: (0, -1), Gdk.KEY_Right: (0, 1),
                  Gdk.KEY_KP_Up: (-1, 0), Gdk.KEY_KP_Down: (1, 0), Gdk.KEY_KP_Left: (0, -1),
                  Gdk.KEY_KP_Right: (0, 1)}
        if kv in arrows and not alt:
            g.move(*arrows[kv], extend=shift, ctrl=ctrl)
            return True
        if kv in (Gdk.KEY_Page_Down, Gdk.KEY_Page_Up):
            d = 1 if kv == Gdk.KEY_Page_Down else -1
            g.page(d, horizontal=alt, extend=shift)
            return True
        if kv == Gdk.KEY_Home:
            r, c = g.sel.cursor if shift else g.sel.active
            st = g.state()
            if ctrl:
                tgt = (st.freeze_rows, st.freeze_cols) if (st.freeze_rows or st.freeze_cols) else (0, 0)
            elif g.end_mode:
                ext = self.book.used_extent(g.sheet) or (0, 0)
                tgt = ext
                g.end_mode = False
            else:
                tgt = (r, st.freeze_cols if c > st.freeze_cols else 0)
            g.select_cell(*tgt, extend=shift)
            return True
        if kv == Gdk.KEY_End:
            if ctrl:
                ext = self.book.used_extent(g.sheet) or (0, 0)
                g.select_cell(*ext, extend=shift)
            else:
                g.end_mode = not g.end_mode
                self.status.extra.set_text("End Mode" if g.end_mode else "")
            return True
        if kv in (Gdk.KEY_Return, Gdk.KEY_KP_Enter):
            if not g.sel.is_single_cell():
                g.sel.step_within(-1 if shift else 1, 0)
                g.ensure_visible(*g.sel.active)
                g.refresh()
            else:
                g.tab_step(-1 if shift else 1, 0)
            return True
        if kv in (Gdk.KEY_Tab, Gdk.KEY_ISO_Left_Tab):
            back = shift or kv == Gdk.KEY_ISO_Left_Tab
            if not g.sel.is_single_cell():
                g.sel.step_within(0, -1 if back else 1)
                g.ensure_visible(*g.sel.active)
                g.refresh()
            else:
                g.tab_step(0, -1 if back else 1)
            return True
        if kv == Gdk.KEY_space and ctrl and not shift:
            r, c = g.sel.active
            g.sel.select_columns(c, c)
            g.sel.active = (r, c)
            g.refresh()
            return True
        if kv == Gdk.KEY_space and shift and not ctrl:
            r, c = g.sel.active
            g.sel.select_rows(r, r)
            g.sel.active = (r, c)
            g.refresh()
            return True
        if kv == Gdk.KEY_space and shift and ctrl:
            self.run("select-all")
            return True
        if kv == Gdk.KEY_BackSpace and ctrl:
            g.ensure_visible(*g.sel.active)
            g.refresh()
            return True
        if kv == Gdk.KEY_F2:
            g.start_edit(mode="edit")
            return True
        if kv == Gdk.KEY_F8:
            if shift:
                g.add_mode = not g.add_mode
                g.extend_mode = False
                self.status.extra.set_text("Add to Selection" if g.add_mode else "")
            else:
                g.extend_mode = not g.extend_mode
                g.add_mode = False
                self.status.extra.set_text("Extend Selection" if g.extend_mode else "")
            return True
        if kv == Gdk.KEY_Delete:
            self.run("clear-contents")
            return True
        if kv == Gdk.KEY_BackSpace:
            r, c = g.sel.active
            from .protect_ui import may_change_cells
            if not may_change_cells(self, [(r, c, r, c)]):
                return True
            self.book.clear_contents(g.sheet, [(r, c, r, c)])
            g.start_edit(initial="")
            return True
        if kv == Gdk.KEY_Escape:
            if self.painter:
                self.painter = None
            if g.copy_range is not None:
                self.cancel_copy()
            g.extend_mode = g.add_mode = False
            self.status.extra.set_text("")
            return True
        if ctrl or alt:
            return False
        uc = Gdk.keyval_to_unicode(kv)
        if uc and (chr(uc).isprintable()):
            g.start_edit(initial=chr(uc))
            return True
        return False
