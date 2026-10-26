# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): Text to Columns and the Text Import Wizard —
# the parsing, the wizard's three steps, column formats, destination, and files.
import os
import sys
import tempfile
import time
import traceback

from gi.repository import Gtk

checks = []
ERRORS = []
_hook = sys.excepthook
sys.excepthook = lambda t, v, tb: (ERRORS.append("".join(traceback.format_exception(t, v, tb))), _hook(t, v, tb))


def check(name, cond, detail=""):
    checks.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  " + repr(detail)), flush=True)


def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.005)


SCRIPT = []
SEEN = []


def fake_run(d):
    pump(0.02)
    text = d.get_property("text") if isinstance(d, Gtk.MessageDialog) else None
    SEEN.append((d.get_title(), text))
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run

from folio_sheets import text_ui as T  # noqa: E402

# ---- parsing
check("quoted fields keep their commas", T.split_delimited('a,"b, c",d', ",") == ["a", "b, c", "d"])
check("a doubled quote is one quote", T.split_delimited('"say ""hi""",x', ",") == ['say "hi"', "x"])
check("empty fields stay", T.split_delimited("a,,b,", ",") == ["a", "", "b", ""])
check("consecutive delimiters as one", T.split_delimited("a  b   c", " ", consecutive=True) == ["a", "b", "c"])
check("no qualifier: quotes are text", T.split_delimited('"a",b', ",", qualifier="") == ['"a"', "b"])
check("fixed width cuts at the breaks", T.split_fixed("Pear    12  red", [8, 12]) == ["Pear", "12", "red"])
check("breaks are guessed from the spaces", T.guess_breaks(["Pear    12  red", "Fig     3   blue"]) == [8, 12],
      T.guess_breaks(["Pear    12  red", "Fig     3   blue"]))
check("dates in day-month-year order", T.parse_date("05/03/2026", "DMY") == "2026-03-05")
check("and year-month-day", T.parse_date("2026.3.5", "YMD") == "2026-03-05")
check("two-digit years", T.parse_date("3/5/26", "MDY") == "2026-03-05")
check("not a date stays text", T.parse_date("13/13/2026", "DMY") is None)
check("decimal comma and thousands point", T.convert("1.234,5", "general", decimal=",", thousands=".") == "1234.5")
check("trailing minus", T.convert("12-", "general") == "-12")
check("text that looks like a formula stays text", T.convert("=SUM(A1)", "general") == "'=SUM(A1)")
check("Text keeps leading zeros", T.convert("007", "text") == "007")
check("skipped columns go", T.convert("x", "skip") is None)

g = win.grid
b = win.book
S = 0


def sel(r, c, r2=None, c2=None):
    g.select_range((r, c, r if r2 is None else r2, c if c2 is None else c2), (r, c))
    pump(0.01)


def wizard(*steps):
    """SCRIPT steps for the wizard: each is a function of the dialog, run before Next / Finish."""
    for i, fn in enumerate(steps):
        def step(d, fn=fn, last=(i == len(steps) - 1)):
            if fn:
                fn(d)
            return Gtk.ResponseType.OK if last else 2
        SCRIPT.append(step)


# ---- Text to Columns, delimited
for r, t in enumerate(["Name,Qty,When,Code", "pear,7,05/03/2026,007", "fig,12,28/02/2026,010", '"kiwi, gold",3,01/01/2026,099']):
    b.set_input(S, r, 0, t)
sel(0, 0, 3, 0)
seen = {}


def s1(d):
    seen["kind"] = d.opts.kind
    seen["heading"] = d.heading.get_text()


def s2(d):
    seen["comma"] = d.d_checks[","].get_active()
    seen["cols"] = d.columns


def s3(d):
    d.select_column(2)
    d.f_date.set_active(True)
    d.date_order.set_active(T.DATE_ORDERS.index("DMY"))
    d.select_column(3)
    d.f_text.set_active(True)
    seen["fmts"] = dict(d.opts.formats)


wizard(s1, s2, s3)
win.run("text-to-columns")
check("the wizard sees delimited text", seen.get("kind") == "delimited" and seen["heading"].startswith("Step 1 of 3"), seen)
check("and picks the comma, splitting into 4 columns", seen.get("comma") and seen.get("cols") == 4, seen)
check("formats are set per column", seen.get("fmts") == {2: "date:DMY", 3: "text"}, seen.get("fmts"))
check("the column is split across", [b.input(S, 1, c) for c in range(4)][:2] == ["pear", "7"] and b.input(S, 3, 0) == "kiwi, gold",
      [b.input(S, 1, c) for c in range(4)])
check("numbers become numbers", b.value(S, 2, 1) == 12 if hasattr(b, "value") else b.display(S, 2, 1) == "12")
check("dates are read day first", b.display(S, 1, 2) in ("3/5/2026", "5/3/2026", "2026-03-05") and
      b.input(S, 1, 2) != "05/03/2026", (b.display(S, 1, 2), b.input(S, 1, 2)))
check("Text keeps leading zeros", b.display(S, 1, 3) == "007" and b.cell_style(S, 1, 3)["numfmt"] == "@",
      (b.display(S, 1, 3), b.cell_style(S, 1, 3)["numfmt"]))
check("the result is selected", tuple(g.sel.ranges[0]) == (0, 0, 3, 3), g.sel.ranges)
win.run("undo")
check("one undo puts the column back", b.input(S, 1, 0) == "pear,7,05/03/2026,007" and b.input(S, 1, 1) == "")

# ---- a destination elsewhere keeps the source; replacing asks
sel(0, 0, 3, 0)
b.set_input(S, 0, 7, "busy")


def dest(d):
    d.dest.set_text("G1")


SCRIPT.append(lambda d: 2)
SCRIPT.append(lambda d: 2)
SCRIPT.append(lambda d: (dest(d), Gtk.ResponseType.OK)[1])
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)       # don't replace
win.run("text-to-columns")
check("it asks before replacing cells", "replace the contents" in (SEEN[-1][1] or "") and b.input(S, 0, 6) == "", SEEN[-1])
SCRIPT.append(lambda d: (dest(d), Gtk.ResponseType.OK)[1])
SCRIPT.append(lambda d: Gtk.ResponseType.OK)
win.run("text-to-columns")
check("to a destination: the result there", b.input(S, 0, 6) == "Name" and b.input(S, 0, 7) == "Qty")
check("and the source stays", b.input(S, 0, 0) == "Name,Qty,When,Code")
win.run("undo")
sel(0, 0, 0, 1)
SEEN.clear()
win.run("text-to-columns")
check("more than one column is refused", "one column" in (SEEN[-1][1] or ""), SEEN)

# ---- fixed width, with a column skipped
for r, t in enumerate(["Pear    12  red", "Fig     3   blue", "Lime    150 green"]):
    b.set_input(S, 10 + r, 0, t)
sel(10, 0, 12, 0)


def f1(d):
    d.k_fixed.set_active(True)


def f2(d):
    seen["breaks"] = d._breaks()


def f3(d):
    d.select_column(2)
    d.f_skip.set_active(True)


wizard(f1, f2, f3)
win.run("text-to-columns")
check("fixed width breaks found", seen.get("breaks") == [8, 12], seen.get("breaks"))
check("fixed-width columns split, the skipped one left out",
      [b.input(S, 12, c) for c in range(3)] == ["Lime", "150", ""], [b.input(S, 12, c) for c in range(3)])

# ---- the import wizard, from a file
d = tempfile.mkdtemp()
p = os.path.join(d, "prices.txt")
with open(p, "w", encoding="utf-8") as f:
    f.write("Title line\nItem;Price;Stock\nTea;1.234,50;12-\nCoffee;7,25;3\n")


def i1(d):
    d.start.set_value(2)


def i2(d):
    seen["semi"] = d.d_checks[";"].get_active()


def i3(d):
    d.dec.set_text(",")
    d.thou.set_text(".")


wizard(i1, i2, i3)
sel(20, 0)
T.import_into(win, p)
check("the import guesses semicolons", seen.get("semi"))
check("starting at row 2", b.input(S, 20, 0) == "Item")
check("with a decimal comma and thousands point", b.input(S, 21, 1) == "1234.5", b.input(S, 21, 1))
check("and a trailing minus", b.input(S, 21, 2) == "-12", b.input(S, 21, 2))

# ---- opening a .txt goes through the wizard
p2 = os.path.join(d, "plain.txt")
with open(p2, "w", encoding="utf-8") as f:
    f.write("a\tb\n1\t2\n")
wizard(None, None, None)
win.load(p2)
pump(0.1)
check("opening a text file uses the wizard", SEEN[-1][0] == "Text Import Wizard" and win.book.input(0, 1, 1) == "2",
      (SEEN[-1:], win.book.input(0, 1, 1)))
check("as a new, unsaved workbook", win.path is None and win.imported_from == p2)

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
