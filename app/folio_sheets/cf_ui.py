"""Home ▸ Conditional Formatting: the menu's galleries, the rule dialogs, Clear Rules
and Manage Rules."""
from gi.repository import Gdk, Gtk

from . import core

PRESETS = [
    ("Light Red Fill with Dark Red Text", {"fill": 0xFFC7CE, "font_color": 0x9C0006}),
    ("Yellow Fill with Dark Yellow Text", {"fill": 0xFFEB9C, "font_color": 0x9C5700}),
    ("Green Fill with Dark Green Text", {"fill": 0xC6EFCE, "font_color": 0x006100}),
    ("Light Red Fill", {"fill": 0xFFC7CE}),
    ("Red Text", {"font_color": 0x9C0006}),
    ("Red Border", {"border": 0xFF0000}),
    ("Custom Format...", None),
]

BAR_COLORS = [("Blue", 0x638EC6), ("Green", 0x63C384), ("Red", 0xFF555A), ("Orange", 0xFFB628),
              ("Light Blue", 0x008AEF), ("Purple", 0xD6007B)]

# name, colours from the highest values to the lowest
SCALES = [
    ("Green - Yellow - Red", [0x63BE7B, 0xFFEB84, 0xF8696B]),
    ("Red - Yellow - Green", [0xF8696B, 0xFFEB84, 0x63BE7B]),
    ("Green - White - Red", [0x63BE7B, 0xFCFCFF, 0xF8696B]),
    ("Red - White - Green", [0xF8696B, 0xFCFCFF, 0x63BE7B]),
    ("Blue - White - Red", [0x5A8AC6, 0xFCFCFF, 0xF8696B]),
    ("Red - White - Blue", [0xF8696B, 0xFCFCFF, 0x5A8AC6]),
    ("White - Red", [0xFCFCFF, 0xF8696B]),
    ("Red - White", [0xF8696B, 0xFCFCFF]),
    ("Green - White", [0x63BE7B, 0xFCFCFF]),
    ("White - Green", [0xFCFCFF, 0x63BE7B]),
    ("Green - Yellow", [0x63BE7B, 0xFFEF9C]),
    ("Yellow - Green", [0xFFEF9C, 0x63BE7B]),
]

ICON_SETS = [
    ("Directional", [("3 Arrows (Colored)", "3Arrows"), ("3 Arrows (Gray)", "3ArrowsGray"), ("4 Arrows (Colored)", "4Arrows"),
                     ("4 Arrows (Gray)", "4ArrowsGray"), ("5 Arrows (Colored)", "5Arrows"), ("5 Arrows (Gray)", "5ArrowsGray")]),
    ("Shapes", [("3 Traffic Lights (Unrimmed)", "3TrafficLights1"), ("3 Traffic Lights (Rimmed)", "3TrafficLights2"),
                ("3 Signs", "3Signs"), ("4 Traffic Lights", "4TrafficLights"), ("Red To Black", "4RedToBlack")]),
    ("Indicators", [("3 Symbols (Circled)", "3Symbols"), ("3 Symbols (Uncircled)", "3Symbols2"), ("3 Flags", "3Flags")]),
    ("Ratings", [("4 Ratings", "4Rating"), ("5 Quarters", "5Quarters"), ("5 Ratings", "5Rating")]),
]

PERIODS = [("Yesterday", "yesterday"), ("Today", "today"), ("Tomorrow", "tomorrow"), ("In the last 7 days", "last7Days"),
           ("Last week", "lastWeek"), ("This week", "thisWeek"), ("Next week", "nextWeek"), ("Last month", "lastMonth"),
           ("This month", "thisMonth"), ("Next month", "nextMonth")]


def _rgb(c):
    return ((c >> 16) & 255) / 255.0, ((c >> 8) & 255) / 255.0, (c & 255) / 255.0


def _swatch(draw, w=28, h=22):
    a = Gtk.DrawingArea()
    a.set_size_request(w, h)
    a.connect("draw", lambda area, cr: (draw(cr, area.get_allocated_width(), area.get_allocated_height()), False)[1])
    return a


def _gallery_button(win, tip, draw, cmd):
    b = Gtk.Button()
    b.set_relief(Gtk.ReliefStyle.NONE)
    b.set_tooltip_text(tip)
    b.add(_swatch(draw))

    def go(_b):
        pop = b.get_ancestor(Gtk.Popover)
        if pop is not None:
            pop.popdown()
        win.run(cmd)
    b.connect("clicked", go)
    b.cmd = cmd
    return b


def _row(buttons):
    box = Gtk.FlowBox()
    box.set_selection_mode(Gtk.SelectionMode.NONE)
    box.set_max_children_per_line(6)
    box.set_min_children_per_line(6)
    box.set_margin_start(8)
    for b in buttons:
        box.add(b)
    return box


def bar_gallery(win):
    out = []
    for gradient in (True, False):
        for name, col in BAR_COLORS:
            def draw(cr, w, h, col=col, gradient=gradient):
                for k, frac in enumerate((0.9, 0.5, 0.25)):
                    y = 2 + k * (h - 4) / 3.0
                    bw = (w - 4) * frac
                    if gradient:
                        import cairo
                        pat = cairo.LinearGradient(2, 0, 2 + bw, 0)
                        pat.add_color_stop_rgb(0, *_rgb(col))
                        pat.add_color_stop_rgb(1, 1, 1, 1)
                        cr.set_source(pat)
                    else:
                        cr.set_source_rgb(*_rgb(col))
                    cr.rectangle(2, y, bw, (h - 4) / 3.0 - 1)
                    cr.fill()
            out.append(_gallery_button(win, "%s %s Data Bar" % ("Gradient Fill" if gradient else "Solid Fill", name), draw,
                                       "conditional-formatting:bar:%s:%06X" % ("g" if gradient else "s", col)))
    return [("#", "Data Bars"), _row(out[:6]), _row(out[6:])]


def scale_gallery(win):
    out = []
    for k, (name, cols) in enumerate(SCALES):
        def draw(cr, w, h, cols=cols):
            n = 3
            for i in range(n):
                t = i / (n - 1)
                c = _interp(cols, t)
                cr.set_source_rgb(*_rgb(c))
                cr.rectangle(2, 2 + i * (h - 4) / n, w - 4, (h - 4) / n)
                cr.fill()
        out.append(_gallery_button(win, name + " Color Scale", draw, "conditional-formatting:scale:%d" % k))
    return [("#", "Color Scales"), _row(out[:6]), _row(out[6:])]


def _interp(cols, t):
    # cols run from the highest to the lowest; t = 0 is the top
    if len(cols) == 2:
        a, b, u = cols[0], cols[1], t
    elif t <= 0.5:
        a, b, u = cols[0], cols[1], t * 2
    else:
        a, b, u = cols[1], cols[2], (t - 0.5) * 2
    ch = lambda s: int(round(((a >> s) & 255) * (1 - u) + ((b >> s) & 255) * u))
    return (ch(16) << 16) | (ch(8) << 8) | ch(0)


def icon_gallery(win):
    items = [("#", "Icon Sets")]
    for group, sets in ICON_SETS:
        btns = []
        for name, key in sets:
            n = int(key[0])

            def draw(cr, w, h, key=key, n=n):
                sz = min(h - 2, (w - 2) / n)
                for i in range(n):
                    core.draw_cf_icon(cr, key, n - 1 - i, 1 + i * sz, (h - sz) / 2, sz)
            btns.append(_gallery_button(win, name, draw, "conditional-formatting:icons:" + key))
        for b in btns:
            b.get_child().set_size_request(78, 20)
        box = Gtk.FlowBox()
        box.set_selection_mode(Gtk.SelectionMode.NONE)
        box.set_max_children_per_line(3)
        box.set_min_children_per_line(3)
        box.set_margin_start(8)
        for b in btns:
            box.add(b)
        items.append(box)
    return items


def menu(win):
    """The Conditional Formatting menu."""
    items = [("#", "Highlight Cells Rules"),
             ("Greater Than...", "conditional-formatting:gt"), ("Less Than...", "conditional-formatting:lt"),
             ("Between...", "conditional-formatting:between"), ("Equal To...", "conditional-formatting:eq"),
             ("Text that Contains...", "conditional-formatting:text"), ("A Date Occurring...", "conditional-formatting:date"),
             ("Duplicate Values...", "conditional-formatting:dup"),
             ("#", "Top/Bottom Rules"),
             ("Top 10 Items...", "conditional-formatting:top10"), ("Top 10%...", "conditional-formatting:top10pct"),
             ("Bottom 10 Items...", "conditional-formatting:bottom10"), ("Bottom 10%...", "conditional-formatting:bottom10pct"),
             ("Above Average...", "conditional-formatting:above"), ("Below Average...", "conditional-formatting:below")]
    items += bar_gallery(win) + scale_gallery(win) + icon_gallery(win)
    items += [None, ("New Rule...", "conditional-formatting:new"),
              ("Clear Rules from Selected Cells", "conditional-formatting:clear-sel"),
              ("Clear Rules from Entire Sheet", "conditional-formatting:clear-sheet"),
              ("Manage Rules...", "conditional-formatting:manage")]
    return items


def popover(win):
    """The menu as a two-column popover: rules on the left, galleries on the right."""
    from gi.repository import GLib
    pop = Gtk.Popover()
    outer = Gtk.Box(spacing=12)
    outer.set_margin_top(4)
    outer.set_margin_bottom(4)
    outer.set_margin_end(6)
    pop.add(outer)
    items = menu(win)
    left = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    right = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    outer.pack_start(left, False, False, 0)
    outer.pack_start(Gtk.Separator(orientation=Gtk.Orientation.VERTICAL), False, False, 0)
    outer.pack_start(right, False, False, 0)
    target = left
    for it in items:
        if it is None:
            left.pack_start(Gtk.Separator(), False, False, 3)
            continue
        if isinstance(it, Gtk.Widget):
            right.pack_start(it, False, False, 0)
            continue
        if it[0] == "#":
            if it[1] == "Data Bars":
                target = right
            lab = Gtk.Label(xalign=0)
            lab.set_markup("<b>%s</b>" % GLib.markup_escape_text(it[1]))
            lab.set_margin_start(10)
            lab.set_margin_top(4)
            lab.set_margin_bottom(2)
            (right if target is right else left).pack_start(lab, False, False, 0)
            continue
        label, cmd = it
        b = Gtk.Button(label=label)
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.get_child().set_xalign(0)
        b.cmd = cmd

        def go(_b, cmd=cmd):
            pop.popdown()
            GLib.idle_add(lambda: (win.run(cmd), False)[1])
        b.connect("clicked", go)
        # the commands after the galleries go back to the left column
        (left if target is left or not cmd.split(":")[1].startswith(("bar", "scale", "icons")) else right).pack_start(b, False, False, 0)
    outer.show_all()
    return pop


# ---------------------------------------------------------------- dialogs
def _dialog(win, title):
    d = Gtk.Dialog(title=title, transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    box.set_border_width(12)
    d.get_content_area().pack_start(box, True, True, 0)
    return d, box


def _preset_combo():
    cb = Gtk.ComboBoxText()
    cb.set_name("format")
    for name, _ in PRESETS:
        cb.append_text(name)
    cb.set_active(0)
    return cb


def custom_format(win, start=None):
    """A small Format Cells for rules: font colour, style, fill, border. Returns a dict or None."""
    d, box = _dialog(win, "Format Cells")
    start = start or {}
    grid = Gtk.Grid(row_spacing=6, column_spacing=12)
    box.pack_start(grid, False, False, 0)

    def colour_row(y, label, key, default):
        chk = Gtk.CheckButton(label=label)
        chk.set_name(key + "-on")
        btn = Gtk.ColorButton()
        btn.set_name(key)
        c = start.get(key, default)
        rgba = Gdk.RGBA()
        rgba.red, rgba.green, rgba.blue, rgba.alpha = (*_rgb(c), 1.0)
        btn.set_rgba(rgba)
        chk.set_active(key in start)
        grid.attach(chk, 0, y, 1, 1)
        grid.attach(btn, 1, y, 1, 1)
        return chk, btn
    fc = colour_row(0, "Font color", "font_color", 0x9C0006)
    fl = colour_row(1, "Fill", "fill", 0xFFC7CE)
    bd = colour_row(2, "Border", "border", 0xFF0000)
    styles = {}
    for k, (label, key) in enumerate((("Bold", "bold"), ("Italic", "italic"), ("Underline", "underline"), ("Strikethrough", "strike"))):
        c = Gtk.CheckButton(label=label)
        c.set_name(key)
        c.set_active(bool(start.get(key)))
        grid.attach(c, k % 2, 3 + k // 2, 1, 1)
        styles[key] = c
    d.show_all()
    r = d.run()
    out = None
    if r == Gtk.ResponseType.OK:
        out = {}
        for chk, btn in (fc, fl, bd):
            if chk.get_active():
                c = btn.get_rgba()
                out[btn.get_name()] = (int(round(c.red * 255)) << 16) | (int(round(c.green * 255)) << 8) | int(round(c.blue * 255))
        for k, c in styles.items():
            if c.get_active():
                out[k] = True
    d.destroy()
    return out


def _format_of(win, combo, state):
    i = combo.get_active()
    if PRESETS[i][1] is None:
        return state.get("custom") or {}
    return dict(PRESETS[i][1])


def _hook_custom(win, combo, state):
    def changed(cb):
        if PRESETS[cb.get_active()][1] is None:
            got = custom_format(win, state.get("custom"))
            if got is not None:
                state["custom"] = got
            elif not state.get("custom"):
                cb.set_active(0)
    combo.connect("changed", changed)


def _sel_average(win):
    """A starting value: the average of the numbers selected (as the original suggests)."""
    g = win.grid
    b = win.book
    nums = []
    seen = 0
    for r1, c1, r2, c2 in g.sel.ranges:
        for r in range(r1, min(r2, r1 + 500) + 1):
            for c in range(c1, min(c2, c1 + 50) + 1):
                v = b.value(g.sheet, r, c)
                seen += 1
                if isinstance(v, float):
                    nums.append(v)
    if not nums:
        return ""
    return "%g" % round(sum(nums) / len(nums), 2)


HIGHLIGHT = {
    "gt": ("Greater Than", "Format cells that are GREATER THAN:", "greaterThan"),
    "lt": ("Less Than", "Format cells that are LESS THAN:", "lessThan"),
    "between": ("Between", "Format cells that are BETWEEN:", "between"),
    "eq": ("Equal To", "Format cells that are EQUAL TO:", "equal"),
    "text": ("Text That Contains", "Format cells that contain the text:", None),
    "date": ("A Date Occurring", "Format cells that contain a date occurring:", None),
    "dup": ("Duplicate Values", "Format cells that contain:", None),
}


def highlight(win, kind):
    title, prompt, op = HIGHLIGHT[kind]
    d, box = _dialog(win, title)
    box.pack_start(Gtk.Label(label=prompt, xalign=0), False, False, 0)
    row = Gtk.Box(spacing=8)
    box.pack_start(row, False, False, 0)
    entries = []
    combo2 = None
    if kind in ("gt", "lt", "eq", "text"):
        e = Gtk.Entry(text=_sel_average(win) if kind != "text" else "")
        e.set_name("value")
        e.set_activates_default(True)
        row.pack_start(e, True, True, 0)
        entries.append(e)
    elif kind == "between":
        for k in range(2):
            e = Gtk.Entry()
            e.set_name("value%d" % (k + 1))
            e.set_activates_default(True)
            row.pack_start(e, True, True, 0)
            if k == 0:
                row.pack_start(Gtk.Label(label="and"), False, False, 0)
            entries.append(e)
    elif kind == "date":
        combo2 = Gtk.ComboBoxText()
        combo2.set_name("period")
        for lab, key in PERIODS:
            combo2.append(key, lab)
        combo2.set_active(0)
        row.pack_start(combo2, True, True, 0)
    else:
        combo2 = Gtk.ComboBoxText()
        combo2.set_name("which")
        combo2.append("duplicateValues", "Duplicate")
        combo2.append("uniqueValues", "Unique")
        combo2.set_active(0)
        row.pack_start(combo2, False, False, 0)
        row.pack_start(Gtk.Label(label="values"), False, False, 0)
    row.pack_start(Gtk.Label(label="with"), False, False, 0)
    combo = _preset_combo()
    state = {}
    _hook_custom(win, combo, state)
    row.pack_start(combo, False, False, 0)
    d.show_all()
    while True:
        r = d.run()
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        spec = _format_of(win, combo, state)
        if op:
            spec.update({"type": "cellIs", "operator": op, "values": [e.get_text() for e in entries]})
            if any(not v.strip() for v in spec["values"]):
                win.dialogs.message("Enter a value for the rule.", title)
                continue
        elif kind == "text":
            spec.update({"type": "containsText", "text": entries[0].get_text()})
        elif kind == "date":
            spec.update({"type": "timePeriod", "time_period": combo2.get_active_id()})
        else:
            spec.update({"type": combo2.get_active_id()})
        if _add(win, spec, title):
            break
    d.destroy()
    return True


TOPBOTTOM = {
    "top10": ("Top 10 Items", "Format cells that rank in the TOP:", False, False),
    "top10pct": ("Top 10%", "Format cells that rank in the TOP:", False, True),
    "bottom10": ("Bottom 10 Items", "Format cells that rank in the BOTTOM:", True, False),
    "bottom10pct": ("Bottom 10%", "Format cells that rank in the BOTTOM:", True, True),
    "above": ("Above Average", "Format cells that are ABOVE AVERAGE:", None, None),
    "below": ("Below Average", "Format cells that are BELOW AVERAGE:", None, None),
}


def top_bottom(win, kind):
    title, prompt, bottom, percent = TOPBOTTOM[kind]
    d, box = _dialog(win, title)
    box.pack_start(Gtk.Label(label=prompt, xalign=0), False, False, 0)
    row = Gtk.Box(spacing=8)
    box.pack_start(row, False, False, 0)
    spin = None
    if bottom is not None:
        spin = Gtk.SpinButton.new_with_range(1, 1000 if not percent else 100, 1)
        spin.set_name("rank")
        spin.set_value(10)
        row.pack_start(spin, False, False, 0)
        if percent:
            row.pack_start(Gtk.Label(label="%"), False, False, 0)
    row.pack_start(Gtk.Label(label="with" if bottom is not None else "for the selected range with"), False, False, 0)
    combo = _preset_combo()
    state = {}
    _hook_custom(win, combo, state)
    row.pack_start(combo, False, False, 0)
    d.show_all()
    r = d.run()
    if r != Gtk.ResponseType.OK:
        d.destroy()
        return False
    spec = _format_of(win, combo, state)
    if bottom is not None:
        spec.update({"type": "top10", "rank": int(spin.get_value()), "bottom": bottom, "percent": percent})
    else:
        spec.update({"type": "aboveAverage", "above": kind == "above"})
    d.destroy()
    return _add(win, spec, title)


def _ranges(win):
    return [tuple(r) for r in win.grid.sel.ranges]


def _add(win, spec, title="Conditional Formatting"):
    try:
        win.book.add_cf_rule(win.grid.sheet, _ranges(win), spec)
    except ValueError as e:
        win.dialogs.message(str(e), title)
        return False
    win.grid.refresh()
    win.update_title()
    return True


def data_bar(win, arg):
    kind, _, hexcol = arg.partition(":")
    return _add(win, {"type": "dataBar", "bar_color": int(hexcol, 16), "gradient": kind == "g"})


def color_scale(win, k):
    cols = list(reversed(SCALES[int(k)][1]))
    if len(cols) == 3:
        pts = [("min", "", cols[0]), ("percentile", "50", cols[1]), ("max", "", cols[2])]
    else:
        pts = [("min", "", cols[0]), ("max", "", cols[1])]
    return _add(win, {"type": "colorScale", "points": pts})


def icon_set(win, key):
    return _add(win, {"type": "iconSet", "icon_set": key})


def clear(win, whole):
    g = win.grid
    try:
        win.book.clear_cf(g.sheet, None if whole else _ranges(win))
    except ValueError as e:
        win.dialogs.message(str(e))
        return
    g.refresh()
    win.update_title()


# ---------------------------------------------------------------- New Rule
RULE_TYPES = [
    ("Format all cells based on their values", "values"),
    ("Format only cells that contain", "contain"),
    ("Format only top or bottom ranked values", "top"),
    ("Format only values that are above or below average", "average"),
    ("Format only unique or duplicate values", "unique"),
    ("Use a formula to determine which cells to format", "formula"),
]

CONTAIN_KINDS = [("Cell Value", "cell"), ("Specific Text", "text"), ("Dates Occurring", "date"), ("Blanks", "containsBlanks"),
                 ("No Blanks", "notContainsBlanks"), ("Errors", "containsErrors"), ("No Errors", "notContainsErrors")]
OPERATORS = [("between", "between"), ("not between", "notBetween"), ("equal to", "equal"), ("not equal to", "notEqual"),
             ("greater than", "greaterThan"), ("less than", "lessThan"), ("greater than or equal to", "greaterThanOrEqual"),
             ("less than or equal to", "lessThanOrEqual")]
TEXT_OPS = [("containing", "containsText"), ("not containing", "notContainsText"), ("beginning with", "beginsWith"),
            ("ending with", "endsWith")]


def new_rule(win):
    d, box = _dialog(win, "New Formatting Rule")
    d.set_default_size(560, 0)
    box.pack_start(Gtk.Label(label="Select a Rule Type:", xalign=0), False, False, 0)
    types = Gtk.ListBox()
    types.set_name("types")
    for lab, key in RULE_TYPES:
        row = Gtk.ListBoxRow()
        l = Gtk.Label(label=lab, xalign=0)
        l.set_margin_start(6)
        l.set_margin_top(3)
        l.set_margin_bottom(3)
        row.add(l)
        row.key = key
        types.add(row)
    box.pack_start(types, False, False, 0)
    box.pack_start(Gtk.Label(label="Edit the Rule Description:", xalign=0), False, False, 0)
    stack = Gtk.Stack()
    stack.set_name("pages")
    box.pack_start(stack, False, False, 0)
    w = {}

    def named(widget, name):
        widget.set_name(name)
        w[name] = widget
        return widget

    # values: colour scales, bars, icons
    pv = Gtk.Grid(row_spacing=6, column_spacing=8)
    style = named(Gtk.ComboBoxText(), "style")
    for k, lab in (("2scale", "2-Color Scale"), ("3scale", "3-Color Scale"), ("bar", "Data Bar"), ("icons", "Icon Sets")):
        style.append(k, lab)
    style.set_active(0)
    pv.attach(Gtk.Label(label="Format Style:", xalign=0), 0, 0, 1, 1)
    pv.attach(style, 1, 0, 2, 1)
    lo = named(Gtk.ColorButton(), "min-color")
    hi = named(Gtk.ColorButton(), "max-color")
    for btn, c in ((lo, 0xF8696B), (hi, 0x63BE7B)):
        rgba = Gdk.RGBA()
        rgba.red, rgba.green, rgba.blue, rgba.alpha = (*_rgb(c), 1.0)
        btn.set_rgba(rgba)
    pv.attach(Gtk.Label(label="Lowest / bar color:", xalign=0), 0, 1, 1, 1)
    pv.attach(lo, 1, 1, 1, 1)
    pv.attach(Gtk.Label(label="Highest color:", xalign=0), 0, 2, 1, 1)
    pv.attach(hi, 1, 2, 1, 1)
    iconset = named(Gtk.ComboBoxText(), "icon-set")
    for _, sets in ICON_SETS:
        for lab, key in sets:
            iconset.append(key, lab)
    iconset.set_active(0)
    pv.attach(Gtk.Label(label="Icon Style:", xalign=0), 0, 3, 1, 1)
    pv.attach(iconset, 1, 3, 2, 1)
    stack.add_named(pv, "values")
    # contain
    pc = Gtk.Box(spacing=6)
    ck = named(Gtk.ComboBoxText(), "contain")
    for lab, key in CONTAIN_KINDS:
        ck.append(key, lab)
    ck.set_active(0)
    pc.pack_start(ck, False, False, 0)
    op = named(Gtk.ComboBoxText(), "operator")
    for lab, key in OPERATORS:
        op.append(key, lab)
    op.set_active(4)
    pc.pack_start(op, False, False, 0)
    v1 = named(Gtk.Entry(), "value1")
    v2 = named(Gtk.Entry(), "value2")
    v1.set_width_chars(10)
    v2.set_width_chars(10)
    pc.pack_start(v1, True, True, 0)
    pc.pack_start(v2, True, True, 0)
    stack.add_named(pc, "contain")
    # top/bottom
    pt = Gtk.Box(spacing=6)
    tb = named(Gtk.ComboBoxText(), "topbottom")
    tb.append("top", "Top")
    tb.append("bottom", "Bottom")
    tb.set_active(0)
    pt.pack_start(tb, False, False, 0)
    rank = named(Gtk.SpinButton.new_with_range(1, 1000, 1), "rank")
    rank.set_value(10)
    pt.pack_start(rank, False, False, 0)
    pct = named(Gtk.CheckButton(label="% of the selected range"), "percent")
    pt.pack_start(pct, False, False, 0)
    stack.add_named(pt, "top")
    # average
    pa = Gtk.Box(spacing=6)
    av = named(Gtk.ComboBoxText(), "average")
    for key, lab in (("above", "above"), ("below", "below"), ("eabove", "equal or above"), ("ebelow", "equal or below"),
                     ("1above", "1 std dev above"), ("1below", "1 std dev below"), ("2above", "2 std dev above"),
                     ("2below", "2 std dev below"), ("3above", "3 std dev above"), ("3below", "3 std dev below")):
        av.append(key, lab)
    av.set_active(0)
    pa.pack_start(Gtk.Label(label="Format values that are"), False, False, 0)
    pa.pack_start(av, False, False, 0)
    pa.pack_start(Gtk.Label(label="the average for the selected range"), False, False, 0)
    stack.add_named(pa, "average")
    # unique
    pu = Gtk.Box(spacing=6)
    un = named(Gtk.ComboBoxText(), "unique")
    un.append("duplicateValues", "duplicate")
    un.append("uniqueValues", "unique")
    un.set_active(0)
    pu.pack_start(Gtk.Label(label="Format all:"), False, False, 0)
    pu.pack_start(un, False, False, 0)
    pu.pack_start(Gtk.Label(label="values in the selected range"), False, False, 0)
    stack.add_named(pu, "unique")
    # formula
    pf = Gtk.Box(spacing=6)
    pf.pack_start(Gtk.Label(label="Format values where this formula is true:"), False, False, 0)
    fe = named(Gtk.Entry(), "formula")
    fe.set_activates_default(True)
    pf.pack_start(fe, True, True, 0)
    stack.add_named(pf, "formula")
    fmt_row = Gtk.Box(spacing=8)
    preview = Gtk.Label(label="No Format Set")
    fmt_btn = Gtk.Button(label="Format...")
    fmt_btn.set_name("format-button")
    fmt_row.pack_start(Gtk.Label(label="Preview:"), False, False, 0)
    fmt_row.pack_start(preview, True, True, 0)
    fmt_row.pack_start(fmt_btn, False, False, 0)
    box.pack_start(fmt_row, False, False, 0)
    state = {"format": {}}

    def pick_format(_b):
        got = custom_format(win, state["format"])
        if got is not None:
            state["format"] = got
            preview.set_text("AaBbCcYyZz" if got else "No Format Set")
    fmt_btn.connect("clicked", pick_format)

    def on_type(lb, row):
        if row is not None:
            stack.set_visible_child_name(row.key)
            fmt_row.set_visible(row.key != "values")
    types.connect("row-selected", on_type)
    types.select_row(types.get_row_at_index(0))
    d.show_all()
    on_type(types, types.get_selected_row())

    def color(btn):
        c = btn.get_rgba()
        return (int(round(c.red * 255)) << 16) | (int(round(c.green * 255)) << 8) | int(round(c.blue * 255))
    while True:
        r = d.run()
        if r != Gtk.ResponseType.OK:
            d.destroy()
            return False
        key = types.get_selected_row().key
        spec = dict(state["format"])
        if key == "values":
            st = style.get_active_id()
            if st == "2scale":
                spec = {"type": "colorScale", "points": [("min", "", color(lo)), ("max", "", color(hi))]}
            elif st == "3scale":
                spec = {"type": "colorScale", "points": [("min", "", color(lo)), ("percentile", "50", 0xFFEB84), ("max", "", color(hi))]}
            elif st == "bar":
                spec = {"type": "dataBar", "bar_color": color(lo), "gradient": True}
            else:
                spec = {"type": "iconSet", "icon_set": iconset.get_active_id()}
        elif key == "contain":
            kind = ck.get_active_id()
            if kind == "cell":
                vals = [v1.get_text()] + ([v2.get_text()] if "etween" in op.get_active_id() else [])
                spec.update({"type": "cellIs", "operator": op.get_active_id(), "values": vals})
            elif kind == "text":
                spec.update({"type": dict((k, k) for _, k in TEXT_OPS).get(op.get_active_id(), "containsText"), "text": v1.get_text()})
            elif kind == "date":
                spec.update({"type": "timePeriod", "time_period": v1.get_text() or "today"})
            else:
                spec.update({"type": kind})
        elif key == "top":
            spec.update({"type": "top10", "rank": int(rank.get_value()), "bottom": tb.get_active_id() == "bottom", "percent": pct.get_active()})
        elif key == "average":
            a = av.get_active_id()
            sd = int(a[0]) if a[0].isdigit() else 0
            spec.update({"type": "aboveAverage", "above": a.endswith("above"), "equal": a.startswith("e"), "std_dev": sd})
        elif key == "unique":
            spec.update({"type": un.get_active_id()})
        else:
            spec.update({"type": "expression", "values": [fe.get_text()]})
        if _add(win, spec, "New Formatting Rule"):
            break
    d.destroy()
    return True


# ---------------------------------------------------------------- Manage Rules
def manage(win):
    g = win.grid
    b = win.book
    s = g.sheet
    d = Gtk.Dialog(title="Conditional Formatting Rules Manager", transient_for=win, modal=True)
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_size(720, 380)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
    box.set_border_width(12)
    d.get_content_area().pack_start(box, True, True, 0)
    top = Gtk.Box(spacing=8)
    scope = Gtk.ComboBoxText()
    scope.set_name("scope")
    scope.append("sel", "Current Selection")
    scope.append("sheet", "This Worksheet")
    scope.set_active(0)
    top.pack_start(Gtk.Label(label="Show formatting rules for:"), False, False, 0)
    top.pack_start(scope, False, False, 0)
    box.pack_start(top, False, False, 0)
    btns = Gtk.Box(spacing=6)
    names = {}
    for lab, key in (("New Rule...", "new"), ("Delete Rule", "delete"), ("Move Up", "up"), ("Move Down", "down")):
        bt = Gtk.Button(label=lab)
        bt.set_name(key)
        names[key] = bt
        btns.pack_start(bt, False, False, 0)
    box.pack_start(btns, False, False, 0)
    store = Gtk.ListStore(str, str, str, bool, int, int, str)   # rule, format, applies, stop, piece, index, fill
    view = Gtk.TreeView(model=store)
    view.set_name("rules")
    view.append_column(Gtk.TreeViewColumn("Rule (applied in order shown)", Gtk.CellRendererText(), text=0))
    fcell = Gtk.CellRendererText()
    fcol = Gtk.TreeViewColumn("Format", fcell, text=1)
    fcol.add_attribute(fcell, "background", 6)
    view.append_column(fcol)
    acell = Gtk.CellRendererText(editable=True)
    view.append_column(Gtk.TreeViewColumn("Applies to", acell, text=2))
    tcell = Gtk.CellRendererToggle()
    view.append_column(Gtk.TreeViewColumn("Stop If True", tcell, active=3))
    sw = Gtk.ScrolledWindow()
    sw.set_vexpand(True)
    sw.add(view)
    box.pack_start(sw, True, True, 0)
    steps = [0]

    def fill():
        rg = tuple(g.sel.ranges[-1]) if scope.get_active_id() == "sel" else None
        store.clear()
        for r in b.cf_rules(s, rg):
            if r["piece"] is None:
                continue
            fmt = "AaBbCcYyZz" if r["fill"] is not None or r["color"] is not None or r["bold"] else ""
            bg = "#%06X" % r["fill"] if r["fill"] is not None else "#FFFFFF"
            store.append([r["desc"], fmt, r["applies"], r["stop"], r["piece"], r["index"], bg])

    def selected():
        m, it = view.get_selection().get_selected()
        if it is None:
            return None
        return m[it][4], m[it][5], m.get_path(it)[0]

    def act(fn, keep_row=None):
        try:
            fn()
            steps[0] += 1
        except ValueError as e:
            win.dialogs.message(str(e), "Manage Rules")
        fill()
        if keep_row is not None and 0 <= keep_row < len(store):
            view.get_selection().select_path(Gtk.TreePath(keep_row))
        g.refresh()

    def on_edit_applies(_c, path, text):
        p, i = store[path][4], store[path][5]
        act(lambda: b.edit_cf_rule(s, p, i, None, text, 0), int(path))

    def on_toggle(_c, path):
        p, i = store[path][4], store[path][5]
        act(lambda: b.edit_cf_rule(s, p, i, not store[path][3], None, 0), int(path))
    acell.connect("edited", on_edit_applies)
    tcell.connect("toggled", on_toggle)

    def on_delete(_b):
        x = selected()
        if x:
            act(lambda: b.delete_cf_rule(s, x[0], x[1]), x[2])

    def on_move(step):
        x = selected()
        if x:
            act(lambda: b.edit_cf_rule(s, x[0], x[1], None, None, step), x[2] + step)

    def on_new(_b):
        if new_rule(win):
            steps[0] += 1
            fill()
    names["delete"].connect("clicked", on_delete)
    names["up"].connect("clicked", lambda _b: on_move(-1))
    names["down"].connect("clicked", lambda _b: on_move(1))
    names["new"].connect("clicked", on_new)
    scope.connect("changed", lambda _c: fill())
    fill()
    d.show_all()
    r = d.run()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        for _ in range(steps[0]):
            b.undo()
    g.refresh()
    win.update_title()
    return r == Gtk.ResponseType.OK
