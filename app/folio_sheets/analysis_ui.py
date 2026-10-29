"""Data > Data Analysis (S3-09): the Analysis ToolPak's dialogs. Pick a tool, give its input
range and options, and its output table goes to an output range, a new worksheet or a new
workbook, as values (like the desktop ToolPak, the output doesn't recalculate)."""
import re

from gi.repository import Gtk

from . import analysis as A
from .core import a1

REF = re.compile(r"^\s*=?\s*(?:('(?:[^']|'')+'|[^!:]+)!)?\$?([A-Za-z]{1,3})\$?(\d+)(?::\$?([A-Za-z]{1,3})\$?(\d+))?\s*$")


def _col(s):
    n = 0
    for ch in s.upper():
        n = n * 26 + ord(ch) - 64
    return n - 1


def parse(win, text):
    """'A1:B9', 'Sheet2!$A$1:$B$9' -> (sheet, (r1, c1, r2, c2)), or None."""
    m = REF.match(text or "")
    if not m:
        return None
    s = win.grid.sheet
    if m.group(1):
        name = m.group(1)
        if name.startswith("'"):
            name = name[1:-1].replace("''", "'")
        s = None
        for i in range(win.book.sheet_count):
            if win.book.sheet_info(i)["name"].lower() == name.lower():
                s = i
        if s is None:
            return None
    r1, c1 = int(m.group(3)) - 1, _col(m.group(2))
    r2, c2 = (int(m.group(5)) - 1, _col(m.group(4))) if m.group(4) else (r1, c1)
    return s, (min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2))


def abs_text(rg):
    def one(r, c):
        m = re.match(r"([A-Z]+)(\d+)", a1(r, c))
        return "$%s$%s" % (m.group(1), m.group(2))
    return one(rg[0], rg[1]) if rg[:2] == rg[2:] else "%s:%s" % (one(rg[0], rg[1]), one(rg[2], rg[3]))


def _num(b, s, r, c):
    """A cell's number; None when empty, "text" when it holds something else."""
    v = b.value(s, r, c)
    if v is None or v == "":
        return None
    if isinstance(v, bool):
        return "text"
    if isinstance(v, (int, float)):
        return float(v)
    return "text"


class InputError(ValueError):
    pass


def read_series(win, text, by_rows=False, labels=False, need=None):
    """The input range as series (columns, or rows): [(label, [numbers])]."""
    p = parse(win, text)
    if p is None:
        raise InputError("Input range must be a contiguous reference.")
    s, (r1, c1, r2, c2) = p
    b = win.book
    out = []
    lines = range(r1, r2 + 1) if by_rows else range(c1, c2 + 1)
    for k, line in enumerate(lines):
        cells = [(line, c) for c in range(c1, c2 + 1)] if by_rows else [(r, line) for r in range(r1, r2 + 1)]
        if labels:
            name = b.display(s, *cells[0])
            cells = cells[1:]
        else:
            name = ("Row %d" if by_rows else "Column %d") % (k + 1)
        vals = []
        for r, c in cells:
            v = _num(b, s, r, c)
            if v is None:
                continue
            if v == "text":
                raise InputError("Input range contains non-numeric data.")
            vals.append(v)
        out.append((name, vals))
    if need and len(out) != need:
        raise InputError("Input range must be a single %s." % ("row" if by_rows else "column"))
    for name, vals in out:
        if not vals:
            raise InputError("Input range has no numbers in %s." % name)
    return out


def read_grid(win, text, labels=False):
    """A block of numbers with optional row and column labels."""
    p = parse(win, text)
    if p is None:
        raise InputError("Input range must be a contiguous reference.")
    s, (r1, c1, r2, c2) = p
    b = win.book
    col_labels = [b.display(s, r1, c) for c in range(c1 + 1, c2 + 1)] if labels else ["Column %d" % (k + 1) for k in range(c2 - c1 + 1)]
    row_labels = []
    grid = []
    for r in range(r1 + (1 if labels else 0), r2 + 1):
        row_labels.append(b.display(s, r, c1) if labels else "Row %d" % (len(row_labels) + 1))
        row = []
        for c in range(c1 + (1 if labels else 0), c2 + 1):
            v = _num(b, s, r, c)
            if v is None or v == "text":
                raise InputError("Input range contains non-numeric data or empty cells.")
            row.append(v)
        grid.append(row)
    return row_labels, col_labels, grid


# ------------------------------------------------------------------ writing the output
def _cell_text(v):
    if v is None:
        return None
    if isinstance(v, bool):
        return "TRUE" if v else "FALSE"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        if v != v or v in (float("inf"), float("-inf")):
            return "#NUM!"
        return "%.15g" % v
    t = str(v)
    if t.startswith("#"):
        return t if t in ("#N/A", "#NUM!", "#DIV/0!", "#VALUE!") else "'" + t
    if re.match(r"^[-+]?\d", t) or t.startswith("="):
        return "'" + t
    return t


def write(win, table, out, title):
    """table rows into the destination: ("range", text) | ("sheet", name) | ("book", None).
    (book, sheet, row, col) of the top left."""
    kind, arg = out
    b = win.book
    w = win
    if kind == "book":
        w = win.app.new_window()
        b = w.book
        s, r0, c0 = 0, 0, 0
    elif kind == "sheet":
        s = b.insert_sheet(win.grid.sheet, None)
        if arg:
            try:
                b.rename_sheet(s, arg)
            except ValueError as e:
                b.delete_sheet(s)
                raise InputError(str(e))
        r0, c0 = 0, 0
    else:
        p = parse(win, arg)
        if p is None:
            raise InputError("Output range must be a cell reference.")
        s, (r0, c0, _r2, _c2) = p
    with b.group(title):
        for i, row in enumerate(table):
            for j, v in enumerate(row):
                t = _cell_text(v)
                if t is not None:
                    b.set_input(s, r0 + i, c0 + j, t)
    if kind == "sheet":
        win._after_structure(s)
    elif kind == "range":
        w.grid.set_sheet(s) if w.grid.sheet != s else None
        w.grid.refresh()
    w.update_title()
    return w, b, s, r0, c0


# ------------------------------------------------------------------ the tool dialogs
# field: (key, kind, label, default). kinds: range, check, num, int, grouped, choice:<a|b>
SPECS = {
    "Anova: Single Factor": [("input", "range", "Input Range:", ""), ("grouped", "grouped", "Grouped By:", "cols"),
                             ("labels", "check", "Labels in First Row", False), ("alpha", "num", "Alpha:", 0.05)],
    "Anova: Two-Factor With Replication": [("input", "range", "Input Range:", ""), ("rows", "int", "Rows per sample:", 2),
                                           ("alpha", "num", "Alpha:", 0.05)],
    "Anova: Two-Factor Without Replication": [("input", "range", "Input Range:", ""), ("labels", "check", "Labels", False),
                                              ("alpha", "num", "Alpha:", 0.05)],
    "Correlation": [("input", "range", "Input Range:", ""), ("grouped", "grouped", "Grouped By:", "cols"),
                    ("labels", "check", "Labels in First Row", False)],
    "Covariance": [("input", "range", "Input Range:", ""), ("grouped", "grouped", "Grouped By:", "cols"),
                   ("labels", "check", "Labels in First Row", False)],
    "Descriptive Statistics": [("input", "range", "Input Range:", ""), ("grouped", "grouped", "Grouped By:", "cols"),
                               ("labels", "check", "Labels in First Row", False), ("summary", "check", "Summary statistics", True),
                               ("conf_on", "check", "Confidence Level for Mean:", False), ("conf", "num", "  Confidence (%):", 95),
                               ("large_on", "check", "Kth Largest:", False), ("large", "int", "  K:", 1),
                               ("small_on", "check", "Kth Smallest:", False), ("small", "int", "  K:", 1)],
    "Exponential Smoothing": [("input", "range", "Input Range:", ""), ("damping", "num", "Damping factor:", 0.3),
                              ("labels", "check", "Labels", False), ("chart", "check", "Chart Output", False),
                              ("stderr", "check", "Standard Errors", False)],
    "F-Test Two-Sample for Variances": [("input", "range", "Variable 1 Range:", ""), ("input2", "range", "Variable 2 Range:", ""),
                                        ("labels", "check", "Labels", False), ("alpha", "num", "Alpha:", 0.05)],
    "Fourier Analysis": [("input", "range", "Input Range:", ""), ("labels", "check", "Labels in First Row", False),
                         ("inverse", "check", "Inverse", False)],
    "Histogram": [("input", "range", "Input Range:", ""), ("bins", "range", "Bin Range:", ""), ("labels", "check", "Labels", False),
                  ("pareto", "check", "Pareto (sorted histogram)", False), ("cumulative", "check", "Cumulative Percentage", False),
                  ("chart", "check", "Chart Output", False)],
    "Moving Average": [("input", "range", "Input Range:", ""), ("labels", "check", "Labels in First Row", False),
                       ("interval", "int", "Interval:", 3), ("chart", "check", "Chart Output", False),
                       ("stderr", "check", "Standard Errors", False)],
    "Random Number Generation": [("nvars", "int", "Number of Variables:", 1), ("count", "int", "Number of Random Numbers:", 10),
                                 ("dist", "choice:Uniform|Normal|Bernoulli|Binomial|Poisson|Patterned|Discrete", "Distribution:", "Uniform"),
                                 ("p1", "num", "Parameter 1 (from / mean / p / lambda):", 0), ("p2", "num", "Parameter 2 (to / std dev / trials):", 1),
                                 ("p3", "num", "Patterned: step", 1), ("p4", "int", "Patterned: repeat each number", 1),
                                 ("p5", "int", "Patterned: repeat each sequence", 1),
                                 ("input", "range", "Discrete: value and probability input range:", ""),
                                 ("seed", "text", "Random Seed:", "")],
    "Rank and Percentile": [("input", "range", "Input Range:", ""), ("grouped", "grouped", "Grouped By:", "cols"),
                            ("labels", "check", "Labels in First Row", False)],
    "Regression": [("input", "range", "Input Y Range:", ""), ("input2", "range", "Input X Range:", ""),
                   ("labels", "check", "Labels", False), ("zero", "check", "Constant is Zero", False),
                   ("conf_on", "check", "Confidence Level", False), ("conf", "num", "  (%):", 95),
                   ("residuals", "check", "Residuals", False), ("stdres", "check", "Standardized Residuals", False),
                   ("resplot", "check", "Residual Plots", False), ("fitplot", "check", "Line Fit Plots", False)],
    "Sampling": [("input", "range", "Input Range:", ""), ("labels", "check", "Labels", False),
                 ("method", "choice:Periodic|Random", "Sampling Method:", "Periodic"), ("period", "int", "Period:", 2),
                 ("count", "int", "Number of Samples:", 5), ("seed", "text", "Random Seed:", "")],
    "t-Test: Paired Two Sample for Means": [("input", "range", "Variable 1 Range:", ""), ("input2", "range", "Variable 2 Range:", ""),
                                            ("hyp", "num", "Hypothesized Mean Difference:", 0), ("labels", "check", "Labels", False),
                                            ("alpha", "num", "Alpha:", 0.05)],
    "t-Test: Two-Sample Assuming Equal Variances": [("input", "range", "Variable 1 Range:", ""), ("input2", "range", "Variable 2 Range:", ""),
                                                    ("hyp", "num", "Hypothesized Mean Difference:", 0), ("labels", "check", "Labels", False),
                                                    ("alpha", "num", "Alpha:", 0.05)],
    "t-Test: Two-Sample Assuming Unequal Variances": [("input", "range", "Variable 1 Range:", ""), ("input2", "range", "Variable 2 Range:", ""),
                                                      ("hyp", "num", "Hypothesized Mean Difference:", 0), ("labels", "check", "Labels", False),
                                                      ("alpha", "num", "Alpha:", 0.05)],
    "z-Test: Two Sample for Means": [("input", "range", "Variable 1 Range:", ""), ("input2", "range", "Variable 2 Range:", ""),
                                     ("hyp", "num", "Hypothesized Mean Difference:", 0), ("var1", "num", "Variable 1 Variance (known):", 1),
                                     ("var2", "num", "Variable 2 Variance (known):", 1), ("labels", "check", "Labels", False),
                                     ("alpha", "num", "Alpha:", 0.05)],
}


def run_tool(win, tool, v):
    """Work out a tool from its options v (a dict as the dialog fills it); where it went."""
    by_rows = v.get("grouped") == "rows"
    labels = v.get("labels", False)
    charts = []
    if tool in ("Descriptive Statistics", "Correlation", "Covariance", "Rank and Percentile", "Anova: Single Factor"):
        series = read_series(win, v["input"], by_rows, labels)
        names = [n for n, _x in series]
        cols = [x for _n, x in series]
        if tool == "Descriptive Statistics":
            table = A.descriptive(cols, names, conf=v["conf"] / 100 if v.get("conf_on") else None,
                                  kth_large=v["large"] if v.get("large_on") else None,
                                  kth_small=v["small"] if v.get("small_on") else None, summary=v.get("summary", True))
        elif tool == "Rank and Percentile":
            table = A.rank_percentile(cols, names)
        elif tool == "Anova: Single Factor":
            table = A.anova_single(cols, names, v.get("alpha", 0.05))
        else:
            if len(set(len(c) for c in cols)) > 1:
                raise InputError("The input columns must have the same number of values.")
            table = A.correlation(cols, names, covariance=(tool == "Covariance"))
    elif tool == "Anova: Two-Factor Without Replication":
        rl, cl, grid = read_grid(win, v["input"], labels)
        table = A.anova_two_no_rep(grid, rl, cl, v.get("alpha", 0.05))
    elif tool == "Anova: Two-Factor With Replication":
        rl, cl, grid = read_grid(win, v["input"], True)
        n = v.get("rows", 2)
        if n < 2 or len(grid) % n:
            raise InputError("Each sample must have the same number of rows (Rows per sample).")
        table = A.anova_two_rep(grid, n, [rl[k * n] for k in range(len(grid) // n)], cl, v.get("alpha", 0.05))
    elif tool in ("F-Test Two-Sample for Variances", "t-Test: Paired Two Sample for Means", "t-Test: Two-Sample Assuming Equal Variances",
                  "t-Test: Two-Sample Assuming Unequal Variances", "z-Test: Two Sample for Means"):
        (la, a), = read_series(win, v["input"], False, labels, need=1)
        (lb, bb), = read_series(win, v["input2"], False, labels, need=1)
        if not labels:
            la, lb = "Variable 1", "Variable 2"
        alpha = v.get("alpha", 0.05)
        if tool.startswith("F-Test"):
            table = A.f_test(a, bb, la, lb, alpha)
        elif "Paired" in tool:
            table = A.t_paired(a, bb, la, lb, v.get("hyp", 0), alpha)
        elif "Equal" in tool:
            table = A.t_equal(a, bb, la, lb, v.get("hyp", 0), alpha)
        elif "Unequal" in tool:
            table = A.t_unequal(a, bb, la, lb, v.get("hyp", 0), alpha)
        else:
            table = A.z_test(a, bb, la, lb, v.get("var1", 1), v.get("var2", 1), v.get("hyp", 0), alpha)
    elif tool == "Histogram":
        (_l, xs), = read_series(win, v["input"], False, labels, need=1)
        bins = None
        if v.get("bins"):
            (_bl, bins), = read_series(win, v["bins"], False, labels, need=1)
        table, _bins, _counts = A.histogram(xs, bins, v.get("pareto"), v.get("cumulative"), "Bin" if not labels or not v.get("bins") else _bl)
        if v.get("chart"):
            charts.append(("col", (1, 0, len(table) - 1, 1), "Histogram"))
    elif tool == "Regression":
        (ly, y), = read_series(win, v["input"], False, labels, need=1)
        xs = read_series(win, v["input2"], False, labels)
        if any(len(x) != len(y) for _n, x in xs):
            raise InputError("Input Y and X ranges must have the same number of rows.")
        if len(y) < len(xs) + 2:
            raise InputError("Regression needs more observations than X variables.")
        table = A.regression(y, [x for _n, x in xs], [n if labels else "X Variable %d" % (k + 1) for k, (n, _x) in enumerate(xs)],
                             v.get("zero", False), v["conf"] / 100 if v.get("conf_on") else 0.95,
                             v.get("residuals") or v.get("resplot") or v.get("fitplot"), v.get("stdres"), ly if labels else "Y")
    elif tool == "Moving Average":
        (_l, xs), = read_series(win, v["input"], False, labels, need=1)
        n = v.get("interval", 3)
        if not 2 <= n <= len(xs):
            raise InputError("Interval must be between 2 and the number of values.")
        table = A.moving_average(xs, n, v.get("stderr"))
        if v.get("chart"):
            charts.append(("line", (0, 0, len(table) - 1, 0), "Moving Average"))
    elif tool == "Exponential Smoothing":
        (_l, xs), = read_series(win, v["input"], False, labels, need=1)
        d = v.get("damping", 0.3)
        if not 0 <= d <= 1:
            raise InputError("Damping factor must be between 0 and 1.")
        table = A.exp_smoothing(xs, d, v.get("stderr"))
        if v.get("chart"):
            charts.append(("line", (0, 0, len(table) - 1, 0), "Exponential Smoothing"))
    elif tool == "Sampling":
        xs = [x for _n, col in read_series(win, v["input"], False, labels) for x in col]
        seed = int(v["seed"]) if str(v.get("seed", "")).strip().lstrip("-").isdigit() else None
        if v.get("method", "Periodic") == "Periodic":
            if not 1 <= v.get("period", 2) <= len(xs):
                raise InputError("Period must be between 1 and the number of values.")
            table = A.sampling(xs, periodic=v.get("period", 2))
        else:
            table = A.sampling(xs, count=max(1, v.get("count", 5)), seed=seed)
    elif tool == "Fourier Analysis":
        p = parse(win, v["input"])
        if p is None:
            raise InputError("Input range must be a contiguous reference.")
        s, (r1, c1, r2, c2) = p
        vals = [win.book.display(s, r, c) for r in range(r1, r2 + 1) for c in range(c1, c2 + 1)]
        head = []
        if labels:
            head, vals = [[vals[0]]], vals[1:]
        try:
            table = head + A.fourier(vals, v.get("inverse", False))
        except ValueError as e:
            raise InputError(str(e))
    elif tool == "Random Number Generation":
        dist = v.get("dist", "Uniform")
        seed = int(v["seed"]) if str(v.get("seed", "")).strip().lstrip("-").isdigit() else None
        if dist == "Patterned":
            params = (v["p1"], v["p2"], v["p3"] or 1, max(1, v["p4"]), max(1, v["p5"]))
        elif dist == "Discrete":
            rl, _cl, grid = read_grid(win, v["input"], False)
            if len(grid[0]) != 2 or abs(sum(r[1] for r in grid) - 1) > 1e-9:
                raise InputError("The value and probability input range must be two columns whose probabilities add up to 1.")
            params = ([r[0] for r in grid], [r[1] for r in grid])
        else:
            params = (v["p1"], v["p2"])
        if dist in ("Bernoulli", "Binomial") and not 0 <= v["p1"] <= 1:
            raise InputError("p must be between 0 and 1.")
        table = A.random_numbers(max(1, v.get("nvars", 1)), max(1, v.get("count", 10)), dist, params, seed)
    else:
        raise InputError("Unknown tool.")
    return table, charts


class ToolDialog(Gtk.Dialog):
    def __init__(self, win, tool):
        super().__init__(title=tool, transient_for=win, modal=True)
        self.win, self.tool = win, tool
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("OK", Gtk.ResponseType.OK)
        grid = Gtk.Grid(column_spacing=8, row_spacing=5, border_width=12)
        self.get_content_area().pack_start(grid, True, True, 0)
        self.w = {}
        row = 0
        sel = win.grid.sel.ranges[-1] if win.grid.sel.ranges else None
        for key, kind, label, default in SPECS[tool]:
            if kind == "check":
                wd = Gtk.CheckButton(label=label)
                wd.set_active(default)
                grid.attach(wd, 0, row, 2, 1)
            elif kind == "grouped":
                box = Gtk.Box(spacing=8)
                c = Gtk.RadioButton.new_with_label(None, "Columns")
                r = Gtk.RadioButton.new_with_label_from_widget(c, "Rows")
                (r if default == "rows" else c).set_active(True)
                box.pack_start(c, False, False, 0)
                box.pack_start(r, False, False, 0)
                wd = (c, r)
                grid.attach(Gtk.Label(label=label, xalign=0), 0, row, 1, 1)
                grid.attach(box, 1, row, 1, 1)
            elif kind.startswith("choice:"):
                wd = Gtk.ComboBoxText()
                for opt in kind[7:].split("|"):
                    wd.append(opt, opt)
                wd.set_active_id(default)
                grid.attach(Gtk.Label(label=label, xalign=0), 0, row, 1, 1)
                grid.attach(wd, 1, row, 1, 1)
            else:
                wd = Gtk.Entry()
                text = str(default)
                if key == "input" and sel and (sel[2] > sel[0] or sel[3] > sel[1]):
                    text = abs_text(sel)
                wd.set_text(text)
                wd.set_hexpand(True)
                grid.attach(Gtk.Label(label=label, xalign=0), 0, row, 1, 1)
                grid.attach(wd, 1, row, 1, 1)
            self.w[key] = (kind, wd)
            row += 1
        grid.attach(Gtk.Separator(), 0, row, 2, 1)
        row += 1
        grid.attach(Gtk.Label(label="Output options", xalign=0), 0, row, 2, 1)
        row += 1
        self.o_range = Gtk.RadioButton.new_with_label(None, "Output Range:")
        self.o_range_e = Gtk.Entry()
        self.o_sheet = Gtk.RadioButton.new_with_label_from_widget(self.o_range, "New Worksheet Ply:")
        self.o_sheet_e = Gtk.Entry()
        self.o_book = Gtk.RadioButton.new_with_label_from_widget(self.o_range, "New Workbook")
        self.o_sheet.set_active(True)
        grid.attach(self.o_range, 0, row, 1, 1)
        grid.attach(self.o_range_e, 1, row, 1, 1)
        grid.attach(self.o_sheet, 0, row + 1, 1, 1)
        grid.attach(self.o_sheet_e, 1, row + 1, 1, 1)
        grid.attach(self.o_book, 0, row + 2, 2, 1)
        self.o_range_e.connect("changed", lambda *_: self.o_range.set_active(True))
        self.show_all()

    def values(self):
        v = {}
        for key, (kind, wd) in self.w.items():
            if kind == "check":
                v[key] = wd.get_active()
            elif kind == "grouped":
                v[key] = "rows" if wd[1].get_active() else "cols"
            elif kind.startswith("choice:"):
                v[key] = wd.get_active_id()
            elif kind in ("num", "int"):
                t = wd.get_text().strip()
                try:
                    v[key] = int(float(t)) if kind == "int" else float(t)
                except ValueError:
                    raise InputError("%s must be a number." % key.capitalize())
            else:
                v[key] = wd.get_text().strip()
        return v

    def output(self):
        if self.o_range.get_active():
            return ("range", self.o_range_e.get_text().strip())
        if self.o_sheet.get_active():
            return ("sheet", self.o_sheet_e.get_text().strip())
        return ("book", None)

    def run_and_apply(self):
        while True:
            if self.run() != Gtk.ResponseType.OK:
                self.destroy()
                return None
            try:
                got = apply(self.win, self.tool, self.values(), self.output())
                self.destroy()
                return got
            except InputError as e:
                self.win.dialogs.message(str(e), self.tool)


def apply(win, tool, v, out):
    table, charts = run_tool(win, tool, v)
    w, b, s, r0, c0 = write(win, table, out, tool)
    for kind, (r1, c1, r2, c2), _title in charts:
        x, y = b.col_x(s, c0 + max(len(r) for r in table) + 1), b.row_y(s, r0)
        try:
            b.insert_chart(s, (r0 + r1, c0 + c1, r0 + r2, c0 + c2), kind, float(x), float(y))
        except ValueError:
            pass
    if tool == "Regression" and (v.get("resplot") or v.get("fitplot")):
        # the residual / line fit plots: next to the output, from the residual table
        start = next((i for i, r in enumerate(table) if r[:1] == ["Observation"]), None)
        if start is not None:
            x = b.col_x(s, c0 + 10)
            n = len(table) - start - 1
            try:
                b.insert_chart(s, (r0 + start, c0 + 1, r0 + start + n, c0 + 2), "scatter", float(x), float(b.row_y(s, r0)))
            except ValueError:
                pass
    w.grid.refresh()
    return w, s, r0, c0


class AnalysisDialog(Gtk.Dialog):
    """Data Analysis: the list of tools."""

    def __init__(self, win):
        super().__init__(title="Data Analysis", transient_for=win, modal=True)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.add_button("OK", Gtk.ResponseType.OK)
        box = self.get_content_area()
        box.set_border_width(10)
        box.pack_start(Gtk.Label(label="Analysis Tools", xalign=0), False, False, 0)
        self.store = Gtk.ListStore(str)
        for t in A.TOOLS:
            self.store.append([t])
        self.tv = Gtk.TreeView(model=self.store, headers_visible=False)
        self.tv.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        self.tv.get_selection().select_path(Gtk.TreePath(0))
        self.tv.connect("row-activated", lambda *_: self.response(Gtk.ResponseType.OK))
        sw = Gtk.ScrolledWindow()
        sw.set_size_request(300, 220)
        sw.add(self.tv)
        box.pack_start(sw, True, True, 0)
        self.show_all()

    def picked(self):
        m, it = self.tv.get_selection().get_selected()
        return m[it][0] if it else None

    def select(self, tool):
        self.tv.get_selection().select_path(Gtk.TreePath(A.TOOLS.index(tool)))


def show(win):
    d = AnalysisDialog(win)
    r = d.run()
    tool = d.picked()
    d.destroy()
    if r != Gtk.ResponseType.OK or not tool:
        return None
    return ToolDialog(win, tool).run_and_apply()
