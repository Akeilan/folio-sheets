# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Data Validation through the window — the
# dialog, the check on entry (Stop, Warning, Information), the drop-down list, Alt+Down,
# the input message, Circle Invalid Data, and paste.
import os
import sys
import tempfile
import time
import traceback

from gi.repository import Gdk, Gtk

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
    pump(0.03)
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

from folio_sheets.validation_ui import ValidationDialog, bound_labels  # noqa: E402


class Ev:
    def __init__(self, x, y, button=1, kind=Gdk.EventType.BUTTON_PRESS, state=0):
        self.x, self.y, self.button, self.type, self.state = x, y, button, kind, state

    def get_state(self):
        return self.state


win.resize(1280, 800)
pump(0.5)
g = win.grid
b = win.book
S = 0
dv = win.dv


def sel(r1, c1, r2=None, c2=None):
    r2 = r1 if r2 is None else r2
    c2 = c1 if c2 is None else c2
    g.select_range((r1, c1, r2, c2), (r1, c1))
    pump(0.02)


def type_into(r, c, text, move=(1, 0)):
    sel(r, c)
    g.start_edit(initial=text)
    pump(0.02)
    return g.commit_edit(move=move)


def settings(**kw):
    """A SCRIPT step that fills the Data Validation dialog."""
    def fn(d):
        check("the dialog is the Data Validation dialog", isinstance(d, ValidationDialog))
        if "allow" in kw:
            d.allow.set_active(kw["allow"])
        if "op" in kw:
            d.op.set_active(kw["op"])
        for key in ("f1", "f2", "in_title", "err_title"):
            if key in kw:
                getattr(d, key).set_text(kw[key])
        for key in ("in_msg", "err_msg"):
            if key in kw:
                getattr(d, key).get_buffer().set_text(kw[key])
        for key in ("ignore_blank", "dropdown", "show_input", "show_error", "apply_same"):
            if key in kw:
                getattr(d, key).set_active(kw[key])
        if "style" in kw:
            d.style.set_active(kw["style"])
        if "look" in kw:
            kw["look"](d)
        return kw.get("resp", Gtk.ResponseType.OK)
    return fn


# ---- labels follow Allow and Data
check("between asks for Minimum and Maximum", bound_labels("whole", "between") == ("Minimum:", "Maximum:"))
check("dates ask for Start and End date", bound_labels("date", "between") == ("Start date:", "End date:"))
check("less than asks for the Maximum", bound_labels("decimal", "lessThan") == ("Maximum:", None))
check("text length equal asks for the Length", bound_labels("textLength", "equal") == ("Length:", None))
check("a list asks for the Source", bound_labels("list", "between") == ("Source:", None))

# ---- a whole-number rule with an input message and a Stop alert
seen_ui = {}


def look_whole(d):
    seen_ui["op_visible"] = d.op.get_visible()
    seen_ui["f2_label"] = d.f2_label.get_text()
    seen_ui["dropdown_hidden"] = not d.dropdown.get_visible()


sel(0, 0, 4, 0)
SCRIPT.append(settings(allow=1, op=0, f1="1", f2="10", in_title="Quantity", in_msg="A whole number, 1 to 10.",
                       err_title="Out of range", err_msg="Type 1 to 10.", look=look_whole))
win.run("data-validation")
r = b.dv_at(S, 2, 0)
check("OK stores the rule on the selection", r is not None and (r["kind"], r["op"], r["f1"], r["f2"]) ==
      ("whole", "between", "1", "10"), r)
check("Data and Maximum show for Whole number; In-cell dropdown hides",
      seen_ui == {"op_visible": True, "f2_label": "Maximum:", "dropdown_hidden": True}, seen_ui)
check("the input message and error alert are kept", r and r["prompt_title"] == "Quantity" and
      r["error"] == "Type 1 to 10.", r)
check("cells outside the selection have no rule", b.dv_at(S, 5, 0) is None)
win.run("undo")
check("undo takes the rule away in one step", b.dv_at(S, 2, 0) is None)
win.run("redo")
check("redo puts it back", b.dv_at(S, 2, 0) is not None)

# ---- the check on entry
type_into(1, 0, "7")
check("a valid entry is stored", b.input(S, 1, 0) == "7" and not g.editing())
SCRIPT.append(lambda d: Gtk.ResponseType.OK)          # Retry
type_into(1, 0, "50")
check("an invalid entry shows the Stop alert with its title and message",
      SEEN[-1] == ("Out of range", "Type 1 to 10."), SEEN[-1])
check("Retry keeps the edit going with the entry selected",
      g.editing() and g.editor.get_buffer().get_has_selection() and b.input(S, 1, 0) == "7")
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)      # Cancel
g.commit_edit(move=(1, 0))
check("Cancel throws the entry away and keeps the old value", not g.editing() and b.input(S, 1, 0) == "7")
type_into(2, 0, "")
check("Ignore blank lets an empty entry in", b.input(S, 2, 0) == "" and not g.editing())
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
type_into(3, 0, "2.5")
check("a fraction is not a whole number", b.input(S, 3, 0) == "" and SEEN[-1][0] == "Out of range")

# ---- Warning and Information alerts
sel(0, 1, 4, 1)
SCRIPT.append(settings(allow=2, op=4, f1="0", style=1, err_msg=""))
win.run("data-validation")
check("decimal greater than 0, Warning style", (b.dv_at(S, 0, 1) or {}).get("style") == "warning", b.dv_at(S, 0, 1))
SCRIPT.append(lambda d: Gtk.ResponseType.YES)
type_into(0, 1, "-3")
check("Warning: Yes keeps the entry", b.input(S, 0, 1) == "-3", b.input(S, 0, 1))
check("the default message is shown when none is set",
      SEEN[-1][1] == "This value doesn't match the data validation restrictions defined for this cell.", SEEN[-1])
SCRIPT.append(lambda d: Gtk.ResponseType.NO)
type_into(1, 1, "-4")
check("Warning: No goes back to the edit", g.editing() and b.input(S, 1, 1) == "")
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
g.commit_edit()
check("Warning: Cancel drops it", not g.editing() and b.input(S, 1, 1) == "")
sel(0, 1, 4, 1)
SCRIPT.append(settings(style=2))
win.run("data-validation")
check("the dialog opens with the cells' rule", (b.dv_at(S, 0, 1) or {}).get("op") == "greaterThan")
SCRIPT.append(lambda d: Gtk.ResponseType.OK)
type_into(2, 1, "-1")
check("Information: OK keeps the entry", b.input(S, 2, 1) == "-1")
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
type_into(3, 1, "-2")
check("Information: Cancel drops it", b.input(S, 3, 1) == "")

# ---- a list with its drop-down
sel(0, 2, 9, 2)
SCRIPT.append(settings(allow=3, f1="Red,Green,Blue", in_msg="Pick a colour."))
win.run("data-validation")
check("a typed list", b.dv_list(S, 4, 2) == ["Red", "Green", "Blue"], b.dv_list(S, 4, 2))
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
type_into(0, 2, "Purple")
check("an entry not on the list is refused", b.input(S, 0, 2) == "")
type_into(0, 2, "green")
check("list entries match without regard to case", b.input(S, 0, 2) == "green")
sel(1, 2)
x, y, w, h = b.cell_rect(S, 1, 2, g.params())
hit = g.hit(x + w + 6, y + h / 2)
check("the arrow beside the active cell is the drop-down button", list(hit)[:1] == ["dvbtn"], hit)
g.on_press(g.area, Ev(x + w + 6, y + h / 2))
g.on_release(g.area, Ev(x + w + 6, y + h / 2))
pump(0.1)
pop = dv.popover
check("clicking it opens the list", pop is not None and pop.items == ["Red", "Green", "Blue"],
      pop and pop.items)
if pop:
    pop.pick(2)
    pump(0.1)
check("picking an item stores it", b.input(S, 1, 2) == "Blue", b.input(S, 1, 2))
check("and closes the list", dv.popover is None)
sel(2, 2)
win.run("pick-from-list")
pump(0.05)
check("Alt+Down's command opens the same list", dv.popover is not None and dv.popover.why == "validation")
dv.close_popup()
pump(0.05)


class KeyEv:
    keyval = Gdk.KEY_Down
    type = Gdk.EventType.KEY_PRESS
    hardware_keycode = 0

    def get_state(self):
        return Gdk.ModifierType.MOD1_MASK


ev = KeyEv()
g.area.grab_focus()
pump(0.05)
win.on_key(win, ev)
pump(0.05)
check("Alt+Down itself opens it", dv.popover is not None)
dv.close_popup()

# a column without validation: Alt+Down offers the column's entries
for i, t in enumerate(["pear", "apple", "Pear", "fig", "12"]):
    b.set_input(S, 10 + i, 5, t)
sel(15, 5)
win.run("pick-from-list")
pump(0.05)
check("Pick From Drop-down List offers the column's text entries",
      dv.popover is not None and dv.popover.why == "column" and dv.popover.items == ["apple", "fig", "pear"] and "Pear" not in dv.popover.items,
      dv.popover and dv.popover.items)
if dv.popover:
    dv.popover.pick(1)
check("and stores the one picked", b.input(S, 15, 5) == "fig")

# ---- the input message
sel(3, 0)
pump(0.1)
check("selecting a cell with an input message shows it", dv.message_shown() and
      dv.msg_title.get_text() == "Quantity" and dv.msg_text.get_text() == "A whole number, 1 to 10.")
a = dv.box.get_allocation()
x, y, w, h = b.cell_rect(S, 3, 0, g.params())
check("the message sits under the cell", a.y >= y + h and a.x >= x, (a.x, a.y, x, y, w, h))
sel(3, 2)
pump(0.05)
check("another cell shows its own message", dv.message_shown() and dv.msg_text.get_text() == "Pick a colour.")
sel(3, 4)
pump(0.05)
check("a cell without one hides it", not dv.message_shown())

# ---- Circle Invalid Data
b.set_input(S, 4, 0, "99")                  # typed past the check (as a file or a formula could)
b.set_input(S, 5, 2, "Pink")
win.run("data-validation:circle")
circ = sorted(map(tuple, g.params()["dv_circles"]))
check("Circle Invalid Data rings the invalid cells", (4, 0) in circ and (5, 2) in circ and (1, 0) not in circ, circ)
check("the circles are drawn", (pump(0.05) or True))
type_into(4, 0, "4")
circ = sorted(map(tuple, g.params()["dv_circles"]))
check("putting a cell right takes its circle away", (4, 0) not in circ and (5, 2) in circ, circ)
win.run("data-validation:clear-circles")
check("Clear Validation Circles clears the rest", g.params()["dv_circles"] == [])

# ---- mixed selections
sel(0, 0, 0, 2)
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
win.run("data-validation")
check("different rules ask to erase first", "more than one type of validation" in (SEEN[-1][1] or ""), SEEN[-1])
sel(3, 0, 6, 0)
SCRIPT.append(lambda d: Gtk.ResponseType.YES)
SCRIPT.append(settings(look=lambda d: seen_ui.update(kind=d.kind())))
win.run("data-validation")
check("cells without the rule are offered it", "extend Data Validation" in (SEEN[-2][1] or ""), SEEN[-2:])
check("and get it", seen_ui.get("kind") == "whole" and (b.dv_at(S, 6, 0) or {}).get("kind") == "whole")

# ---- apply to all cells with the same settings
sel(1, 0)
SCRIPT.append(settings(f2="20", apply_same=True))
win.run("data-validation")
check("Apply to all cells with the same settings changes the whole rule",
      (b.dv_at(S, 4, 0) or {}).get("f2") == "20" and (b.dv_at(S, 6, 0) or {}).get("f2") == "20",
      (b.dv_at(S, 4, 0), b.dv_at(S, 6, 0)))

# ---- a wrong bound keeps the dialog open
sel(0, 6)
SCRIPT.append(settings(allow=1, op=4, f1="ten"))
SCRIPT.append(lambda d: Gtk.ResponseType.OK)              # the message
SCRIPT.append(settings(f1="10"))                           # the dialog again, put right
win.run("data-validation")
check("a bound that isn't a number is reported", "must be a number" in (SEEN[-2][1] or ""), SEEN[-3:])
check("and the dialog stays for it to be fixed", (b.dv_at(S, 0, 6) or {}).get("f1") == "10")

# ---- dates: typed bounds, shown back as dates
sel(0, 7)
SCRIPT.append(settings(allow=4, op=0, f1="1/1/2026", f2="12/31/2026"))
win.run("data-validation")
check("typed dates are stored as dates", (b.dv_at(S, 0, 7) or {}).get("f1") == "46023", b.dv_at(S, 0, 7))
SCRIPT.append(settings(look=lambda d: seen_ui.update(f1=d.f1.get_text(), f2=d.f2_label.get_text()),
                       resp=Gtk.ResponseType.CANCEL))
win.run("data-validation")
check("and shown as dates again", seen_ui.get("f1") == "1/1/2026" and seen_ui.get("f2") == "End date:", seen_ui)
SCRIPT.append(lambda d: Gtk.ResponseType.CANCEL)
type_into(0, 7, "2/30/2027")
check("a date outside the range is refused", b.input(S, 0, 7) == "")
type_into(0, 7, "6/15/2026")
check("one inside is let in", b.display(S, 0, 7) != "")

# ---- Clear All
sel(0, 6)
SCRIPT.append(lambda d: 1)                                  # Clear All
SCRIPT.append(settings(look=lambda d: seen_ui.update(cleared=d.kind())))
win.run("data-validation")
check("Clear All empties the dialog", seen_ui.get("cleared") == "none")
check("and OK then removes the rule", b.dv_at(S, 0, 6) is None)

# ---- paste
sel(1, 2)
win.run("copy")
sel(0, 9)
win.run("paste")
check("Paste brings the validation along", (b.dv_at(S, 0, 9) or {}).get("kind") == "list" and
      b.input(S, 0, 9) == "Blue")
win.run("undo")
check("undone with the paste", b.dv_at(S, 0, 9) is None and b.input(S, 0, 9) == "")
b.set_input(S, 0, 10, "keep")
sel(1, 2)
win.run("copy")
sel(0, 10)


def only_validation(d):
    for rb in _radios(d):
        if rb.get_label() == "Validation":
            rb.set_active(True)
    return Gtk.ResponseType.OK


def _radios(w):
    out = []
    if isinstance(w, Gtk.RadioButton):
        out.append(w)
    if isinstance(w, Gtk.Container):
        for ch in w.get_children():
            out.extend(_radios(ch))
    return out


SCRIPT.append(only_validation)
win.run("paste-special")
check("Paste Special > Validation copies only the rule",
      (b.dv_at(S, 0, 10) or {}).get("kind") == "list" and b.input(S, 0, 10) == "keep", (b.dv_at(S, 0, 10), b.input(S, 0, 10)))
sel(1, 2)
win.run("copy")
sel(0, 11)
win.run("paste-values")
check("Paste Values leaves validation behind", b.dv_at(S, 0, 11) is None and b.input(S, 0, 11) == "Blue")

# ---- saved and opened again
path = os.path.join(tempfile.mkdtemp(), "dv.xlsx")
b.save(path)
from folio_sheets.core import Book  # noqa: E402
b2 = Book.open(path)
r2 = b2.dv_at(S, 1, 0)
check("the rules survive saving", r2 is not None and r2["prompt_title"] == "Quantity" and r2["f2"] == "20", r2)
check("and the list too", b2.dv_list(S, 3, 2) == ["Red", "Green", "Blue"])

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
