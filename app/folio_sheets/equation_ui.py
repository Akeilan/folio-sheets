"""Equations (SH-21): Insert › Equation puts a text box holding an equation on the sheet; the
Equation tab offers the ready-made equations, symbols and structures. An equation is typed in the
linear format (UnicodeMath, and the common LaTeX commands such as \\frac, \\sqrt and \\alpha) with
the built-up equation shown as you type; it is kept as Office Math, so it shows and edits in Excel."""
from gi.repository import Gdk, GLib, Gtk

from . import APP_NAME
from .core import Book

SLOT = "⬚"   # the dotted box where something is to be typed
PLACEHOLDER = "Type equation here."

STRUCTURES = [
    ("Fraction", "r-eq-fraction", [("Stacked Fraction", "⬚/⬚"), ("dy over dx", "dy/dx"), ("Partial Differential", "∂y/∂x"),
                                   ("Delta y over Delta x", "Δy/Δx"), ("Pi over 2", "π/2")]),
    ("Script", "r-eq-script", [("Superscript", "⬚^⬚"), ("Subscript", "⬚_⬚"), ("Subscript-Superscript", "⬚_⬚^⬚"),
                               ("x squared", "x^2"), ("x subscript n", "x_n"), ("e to the minus i omega t", "e^(−iωt)")]),
    ("Radical", "r-eq-radical", [("Square Root", "√⬚"), ("Radical with Degree", "√(⬚&⬚)"), ("Cube Root", "∛⬚"),
                                 ("Square Root of a squared plus b squared", "√(a^2+b^2)"),
                                 ("Quadratic Formula", "(−b±√(b^2−4ac))/2a")]),
    ("Integral", "r-eq-integral", [("Integral", "∫▒⬚"), ("Integral with Limits", "∫_⬚^⬚▒⬚"), ("Double Integral", "∬▒⬚"),
                                   ("Triple Integral", "∭▒⬚"), ("Contour Integral", "∮▒⬚"),
                                   ("Integral of f of x dx from 0 to 1", "∫_0^1▒〖f(x)dx〗")]),
    ("Large\nOperator", "r-eq-large-operator", [("Summation", "∑▒⬚"), ("Summation with Limits", "∑_⬚^⬚▒⬚"),
                                                ("Summation with Subscript", "∑_⬚▒⬚"), ("Product with Limits", "∏_⬚^⬚▒⬚"),
                                                ("Union with Limits", "⋃_⬚^⬚▒⬚"), ("Intersection with Limits", "⋂_⬚^⬚▒⬚"),
                                                ("Sum over k of n choose k", "∑_(k=0)^n▒(n¦k)")]),
    ("Bracket", "r-eq-bracket", [("Parentheses", "(⬚)"), ("Brackets", "[⬚]"), ("Braces", "{⬚}"), ("Single Bars", "|⬚|"),
                                 ("Double Bars", "‖⬚‖"), ("Angle Brackets", "⟨⬚⟩"), ("Floor", "⌊⬚⌋"), ("Ceiling", "⌈⬚⌉"),
                                 ("Binomial Coefficient", "(⬚¦⬚)"), ("n choose k", "(n¦k)")]),
    ("Function", "r-eq-function", [("Sine Function", "sin⁡⬚"), ("Cosine Function", "cos⁡⬚"), ("Tangent Function", "tan⁡⬚"),
                                   ("Cosecant Function", "csc⁡⬚"), ("Secant Function", "sec⁡⬚"),
                                   ("Cotangent Function", "cot⁡⬚"), ("Hyperbolic Sine", "sinh⁡⬚"),
                                   ("Hyperbolic Cosine", "cosh⁡⬚"), ("Sine theta", "sin⁡θ"), ("Cos 2x", "cos⁡2x")]),
    ("Accent", "r-eq-accent", [("Dot", "⬚̇"), ("Double Dot", "⬚̈"), ("Hat", "⬚̂"), ("Check", "⬚̌"), ("Acute", "⬚́"),
                               ("Grave", "⬚̀"), ("Breve", "⬚̆"), ("Tilde", "⬚̃"), ("Bar", "¯(⬚)"), ("Vector", "⬚⃗")]),
    ("Limit and\nLog", "r-eq-limit", [("Logarithm with Base", "log_⬚⁡⬚"), ("Logarithm", "log⁡⬚"), ("Limit", "lim┬⬚⁡⬚"),
                                      ("Minimum", "min┬⬚⁡⬚"), ("Maximum", "max┬⬚⁡⬚"), ("Natural Logarithm", "ln⁡⬚"),
                                      ("Limit as n approaches infinity", "lim┬(n→∞)⁡〖(1+1/n)^n〗")]),
    ("Operator", "r-eq-operator", [("Colon Equals", "≔"), ("Equal Equal", "=="), ("Plus Equal", "+="),
                                   ("Minus Equal", "−="), ("Equal to by Definition", "≝"), ("Measured by", "≞"),
                                   ("Delta Equal to", "≜"), ("Arrow with Text Above", "→┴⬚"),
                                   ("Arrow with Text Below", "→┬⬚")]),
    ("Matrix", "r-eq-matrix", [("1x2 Empty Matrix", "■(⬚&⬚)"), ("2x1 Empty Matrix", "■(⬚@⬚)"),
                               ("2x2 Empty Matrix", "■(⬚&⬚@⬚&⬚)"), ("3x3 Empty Matrix", "■(⬚&⬚&⬚@⬚&⬚&⬚@⬚&⬚&⬚)"),
                               ("Matrix with Parentheses", "(■(⬚&⬚@⬚&⬚))"), ("Matrix with Brackets", "[■(⬚&⬚@⬚&⬚)]"),
                               ("2x2 Identity Matrix", "(■(1&0@0&1))")]),
]
SYMBOLS = [
    ("Basic Math", "±∞=≠∼×÷!∝<≪>≫≤≥∓≅≈≡∀∁∂√∛∜∪∩∅%°℉℃∆∇∃∄∈∋←↑→↓↔∴+−¬αβγδεϵθϑμπρστφω∗∙⋮⋯⋰⋱ℵℶ∎"),
    ("Greek Letters", "αβγδεϵζηθϑικλμνξοπϖρϱσςτυφϕχψωΓΔΘΛΞΠΣΥΦΨΩ"),
    ("Letter-Like Symbols", "∀∁ℂ∂ð∃∄Ⅎℊℋℌℍℎℏℐℑℒℓℕ№℘ℙℚℛℜℝℤ℧Å℮ℬℭℯℰℱℳℴℵℶℷℸ"),
    ("Operators", "+−±∓∖×∗∘∙÷∕⊕⊖⊗⊘⊙∧∨∩∪⊎⊓⊔∫∬∭∮∑∏∐=≠<>≤≥≪≫≈≅≡≢∼≃∝≺≻⪯⪰⊂⊃⊆⊇∈∋∉⊢⊣⊨"),
    ("Arrows", "←→↑↓↔↕⇐⇒⇑⇓⇔⇕↖↗↘↙↚↛↮⇍⇏⇎⟵⟶⟷⟸⟹⟺↦↩↪↼↽⇀⇁⇄⇆⇋⇌"),
    ("Negated Relations", "≠≮≯≰≱≢≁≄≉≇∉∌⊄⊅⊈⊉⊀⊁⋠⋡∤∦⊬⊭"),
    ("Geometry", "∟∠∡∢⊾⊿⋕⊥∤∥∦∶∷∴∵∎□△▭▱◊⊙"),
]


def _equation_selected(win):
    from . import shapes_ui
    g = win.grid
    for i in shapes_ui.selected_shapes(win):
        info = win.book.shape_info(g.sheet, i)
        if info and info.get("equation") is not None:
            return i
    return None


def preview(linear, size=15, w=None, h=None, pad=6):
    """A drawing of an equation (for galleries and the editor)."""
    da = Gtk.DrawingArea()
    ext = Book.equation_extent(linear or SLOT, size)
    da.set_size_request(w or int(ext[0] + 2 * pad), h or int(ext[1] + ext[2] + 2 * pad))
    da.linear = linear

    def draw(widget, cr):
        from . import appearance
        a = widget.get_allocation()
        lin = widget.linear or SLOT
        ww, asc, desc = Book.equation_extent(lin, size)
        sc = min(1.0, (a.width - 2 * pad) / max(ww, 1), (a.height - 2 * pad) / max(asc + desc, 1))
        cr.save()
        cr.translate(a.width / 2 - ww * sc / 2, a.height / 2 - (asc + desc) * sc / 2 + asc * sc)
        cr.scale(sc, sc)
        Book.paint_equation(cr, lin, 0, 0, size, 0xE8E8E8 if appearance.is_dark() else 0x000000)
        cr.restore()
    da.connect("draw", draw)
    return da


def gallery(win, items, on_pick, title=None, cols=2, size=15):
    """A popover of drawn equations; picking one calls on_pick(linear)."""
    pop = Gtk.Popover()
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
    box.set_border_width(6)
    if title:
        lab = Gtk.Label(label=title, xalign=0)
        lab.get_style_context().add_class("dim-label")
        box.pack_start(lab, False, False, 0)
    flow = Gtk.FlowBox()
    flow.set_max_children_per_line(cols)
    flow.set_min_children_per_line(min(cols, len(items)))
    flow.set_selection_mode(Gtk.SelectionMode.NONE)
    flow.set_name("equation-gallery")
    for name, lin in items:
        b = Gtk.Button()
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.set_tooltip_text(name)
        b.add(preview(lin, size=size, w=150 if cols > 1 else 300))
        b.linear = lin
        b.connect("clicked", lambda _b, lin=lin: (pop.popdown(), on_pick(lin)))
        flow.add(b)
    box.pack_start(flow, False, False, 0)
    pop.add(box)
    box.show_all()
    pop.flow = flow
    return pop


def equation_menu(win):
    """Insert › Equation ▾ : the ready-made equations, Insert New Equation, Ink Equation."""
    pop = gallery(win, Book.equation_built_ins(), lambda lin: insert(win, lin), "Built-In", cols=1, size=16)
    box = pop.get_child()
    box.pack_start(Gtk.Separator(), False, False, 2)
    for label, cb in (("Insert New Equation", lambda: insert(win)), ("Ink Equation", lambda: ink_not_available(win))):
        mb = Gtk.ModelButton(text=label)
        mb.connect("clicked", lambda _b, cb=cb: (pop.popdown(), cb()))
        box.pack_start(mb, False, False, 0)
    box.show_all()
    return pop


def ink_not_available(win):
    win.dialogs.info("Ink Equation isn't available in %s. Type the equation instead: for example "
                     "x=(−b±√(b^2−4ac))/2a or \\frac{a}{b}." % APP_NAME, "Ink Equation")


# ---------------------------------------------------------------- inserting and editing
def insert(win, linear=None):
    """A new equation in the middle of the window; empty ones open for typing."""
    g = win.grid
    b = win.book
    a = g.area.get_allocation()
    p = g.params()
    x, y = b.sheet_point(g.sheet, a.width / 2 - 60, a.height / 2 - 20, p)
    try:
        i = b.insert_equation(g.sheet, linear or "", x, y)
    except ValueError as e:
        win.dialogs.message(str(e), "Equation")
        return None
    g.select_object(i)
    win.on_object_selection(switch_tab=True)
    win.update_title()
    g.refresh()
    if not linear:
        GLib.idle_add(lambda: (edit(win, i), False)[1])
    return i


class Editor:
    """The equation being typed: its linear format, with the built-up equation above it."""
    def __init__(self, win, i, first=None):
        self.win = win
        self.i = i
        g = win.grid
        b = win.book
        self.sheet = g.sheet
        self.start = b.equation_linear(g.sheet, i) or ""
        rect = b.object_screen_rect(g.sheet, i, g.params())
        pop = Gtk.Popover.new(g.area)
        r = Gdk.Rectangle()
        r.x, r.y, r.width, r.height = int(rect[0]), int(rect[1] + rect[3]), int(max(rect[2], 1)), 1
        pop.set_pointing_to(r)
        pop.set_position(Gtk.PositionType.BOTTOM)
        pop.set_modal(False)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        box.set_border_width(6)
        self.view = preview(self.start, size=20, w=360, h=90)
        box.pack_start(self.view, False, False, 0)
        e = Gtk.Entry()
        e.set_name("equation-linear")
        e.set_placeholder_text(PLACEHOLDER)
        e.set_width_chars(44)
        e.set_text(self.start if first is None else self.start + first)
        e.connect("changed", self.changed)
        e.connect("activate", lambda *_: self.finish())
        e.connect("key-press-event", self.key)
        box.pack_start(e, False, False, 0)
        hint = Gtk.Label(label="Linear format: x^2, a/b, √x, ∑_(i=1)^n▒x_i — or \\frac{a}{b}, \\sqrt{x}, \\alpha",
                         xalign=0)
        hint.get_style_context().add_class("dim-label")
        box.pack_start(hint, False, False, 0)
        pop.add(box)
        self.pop = pop
        self.entry = e
        self.done = False
        pop.connect("closed", lambda *_: GLib.idle_add(lambda: (self.finish(), False)[1]))
        box.show_all()
        pop.popup()
        e.grab_focus()
        e.set_position(-1)
        self.changed(e)

    def changed(self, e):
        self.view.linear = e.get_text()
        self.view.queue_draw()

    def key(self, w, ev):
        if ev.keyval == Gdk.KEY_Escape:
            self.entry.set_text(self.start)
            self.finish()
            return True
        return False

    def insert(self, text):
        """A structure or symbol at the cursor; the first empty box in it is selected for typing."""
        e = self.entry
        bounds = e.get_selection_bounds()
        if bounds:
            e.delete_text(*bounds)
        pos = e.get_position()
        e.insert_text(text, pos)
        k = text.find(SLOT)
        if k >= 0:
            e.select_region(pos + k, pos + k + 1)
        else:
            e.set_position(pos + len(text))
        e.grab_focus_without_selecting()

    def finish(self):
        if self.done:
            return
        self.done = True
        text = self.entry.get_text()
        w = self.win
        if w.equation_editor is self:
            w.equation_editor = None
        try:
            if text != self.start:
                w.book.set_equation(self.sheet, self.i, text)
                w.update_title()
        except ValueError as ex:
            w.status.extra.set_text(str(ex))
        try:
            self.pop.popdown()
            self.pop.destroy()
        except Exception:
            pass
        w.grid.refresh()


def edit(win, i, first=None):
    cur = getattr(win, "equation_editor", None)
    if cur is not None:
        cur.finish()
    g = win.grid
    if win.book is None or win.book.equation_linear(g.sheet, i) is None or win.book.object_screen_rect(g.sheet, i, g.params()) is None:
        return False
    win.equation_editor = Editor(win, i, first)
    return True


def put(win, text):
    """A structure or symbol from the Equation tab: into the equation being typed, else into the
    selected equation, else as a new equation."""
    ed = getattr(win, "equation_editor", None)
    if ed is not None and not ed.done:
        ed.insert(text)
        return True
    i = _equation_selected(win)
    if i is None:
        return insert(win, text) is not None
    edit(win, i)
    win.equation_editor.insert(text)
    return True


# ---------------------------------------------------------------- the Equation tab
def build_tab(win, B):
    from .ribbon import Tab
    r = win.ribbon
    win.equation_editor = None
    t = r.building(Tab("Equation", "JE", contextual=True))
    B.group(t, "Tools", [
        B.big("Equation", "r-equation", None, "E", menu=lambda: equation_menu(win)),
        B.big("Ink\nEquation", "r-eq-ink", "equation-ink", "I"),
    ])

    def symbols_menu():
        pop = Gtk.Popover()
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        box.set_border_width(6)
        combo = Gtk.ComboBoxText()
        for name, _ in SYMBOLS:
            combo.append(name, name)
        combo.set_active(0)
        box.pack_start(combo, False, False, 0)
        grid = Gtk.FlowBox()
        grid.set_name("equation-symbols")
        grid.set_max_children_per_line(12)
        grid.set_min_children_per_line(12)
        grid.set_selection_mode(Gtk.SelectionMode.NONE)
        box.pack_start(grid, False, False, 0)

        def fill(*_):
            for c in grid.get_children():
                c.destroy()
            chars = dict(SYMBOLS)[combo.get_active_id()]
            for ch in chars:
                b = Gtk.Button(label=ch)
                b.set_relief(Gtk.ReliefStyle.NONE)
                b.set_size_request(28, 28)
                b.connect("clicked", lambda _b, ch=ch: win.run("equation-put:" + ch))
                grid.add(b)
            grid.show_all()
        combo.connect("changed", fill)
        fill()
        pop.add(box)
        box.show_all()
        pop.symbols = grid
        pop.combo = combo
        return pop
    B.group(t, "Symbols", [B.big("Symbols", "r-symbol", None, "S", menu=symbols_menu)])
    cols = []
    for k, (label, icon, items) in enumerate(STRUCTURES):
        cols.append(B.big(label, icon, None, label[0] + str(k),
                          menu=lambda items=items, label=label: gallery(
                              win, items, lambda lin: win.run("equation-put:" + lin), label.replace("\n", " "))))
    B.group(t, "Structures", cols)
    r.add_tab(t)
    r.show_contextual("Equation", False)


def sync_tab(win):
    i = _equation_selected(win) if win.book is not None else None
    ed = getattr(win, "equation_editor", None)
    win.ribbon.show_contextual("Equation", i is not None or (ed is not None and not ed.done))
    return i
