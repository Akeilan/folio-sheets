# Run inside Folio Sheets (FOLIO_TEST_SCRIPT): notes and threaded comments from the Review
# tab, the context menu and the keyboard; the Comments pane; the file.
import os
import sys
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


Gtk.Dialog.run = lambda d: Gtk.ResponseType.OK
Gtk.MessageDialog.run = lambda d: Gtk.ResponseType.OK

g = win.grid
b = win.book
S = 0
from folio_sheets import notes_ui
me = notes_ui.author()
b.set_input(S, 1, 1, "Budget")
g.select_cell(1, 1)
pump(0.05)

# New Note: a box to type in, signed
win.run("new-note")
pump(0.1)
ed = win.note_editor
check("New Note opens an editor beside the cell", ed is not None and ed.get_visible() and g.edit_note == (1, 1))
check("it starts with the author's name", ed.text() == "%s:\n" % me, ed.text())
ed.view.get_buffer().insert(ed.view.get_buffer().get_end_iter(), "Ask about Q3")
ed.finish()
n = b.note(S, 1, 1)
check("the note is kept", n is not None and n["text"] == "%s:\nAsk about Q3" % me and n["author"] == me, n)
check("as one undo step", b.undo_labels()[0] in ("Edit Note", "New Note"), b.undo_labels()[:2])

# hovering shows it
x, y, w, h = b.cell_rect(S, 1, 1, g.params())
g._hover(b.hit(S, x + 5, y + 5, g.params()))
check("the pointer over the cell shows the note", g.hover_note == (1, 1))
g._hover(("cell", 5, 5))
check("and moving away hides it", g.hover_note is None)

# Show/Hide, Show All
win.run("show-hide-note")
check("Show/Hide Note", b.note(S, 1, 1)["visible"])
win.run("show-all-notes")
check("Show All Notes toggles them all", not b.note(S, 1, 1)["visible"])

# Shift+F2 on a cell with a note edits it
class Key:
    keyval = Gdk.KEY_F2

    def get_state(self):
        return Gdk.ModifierType.SHIFT_MASK


win.set_focus(g.area)
win.on_key(win, Key())
pump(0.05)
check("Shift+F2 edits the note", win.note_editor is not None and win.note_editor.text().endswith("Ask about Q3"))
win.note_editor.finish()

# the context menu offers the right items
items = win._note_items()
check("the context menu has Edit/Delete Note", [x[1] for x in items] == ["edit-note", "delete-note", "show-hide-note"], items)

# a comment
g.select_cell(4, 2)
win.run("new-comment")
pump(0.1)
card = win.comment_card
check("New Comment opens a card", card is not None and card.get_visible())
card.card.view.get_buffer().set_text("Is this final?")
card.card.post.clicked()
pump(0.05)
t = b.thread(S, 4, 2)
check("the comment is posted", t is not None and t["entries"][0][2] == "Is this final?" and t["entries"][0][0] == me, t)
check("the card closes", win.comment_card is None)
notes_ui.open_card(win, 4, 2)
pump(0.05)
win.comment_card.card.view.get_buffer().set_text("Not yet")
win.comment_card.card.post.clicked()
check("a reply", len(b.thread(S, 4, 2)["entries"]) == 2)
check("a note and a comment can't share a cell", notes_ui.open_card(win, 1, 1) is None)

# next/previous
g.select_cell(0, 0)
win.run("next-comment")
check("Next Comment goes to it", tuple(g.sel.active) == (4, 2))
notes_ui.close_card(win)
win.run("next-note")
check("Next Note goes to the note", tuple(g.sel.active) == (1, 1))

# the Comments pane
win.run("show-comments")
pump(0.05)
check("Show Comments opens the pane with the thread", win.comments_pane.get_visible() and len(win.comments_pane.list.get_children()) == 1)
win.run("show-comments")
check("and closes it", not win.comments_pane.get_visible())

# rows inserted above
b.insert_rows(S, 0, 1)
check("notes and comments move with their cells", b.note(S, 2, 1) is not None and b.thread(S, 5, 2) is not None)
win.run("undo")

# copy and paste brings the note
g.sel.ranges = [(1, 1, 1, 1)]
g.sel.active = (1, 1)
win.run("copy")
g.select_cell(10, 1)
win.run("paste")
check("paste copies the note", b.note(S, 10, 1) is not None)
win.run("undo")

# saved and opened again
path = "/tmp/notes_audit.xlsx"
win.save_to(path)
from folio_sheets import core
b2 = core.Book.open(path)
check("the note is saved", b2.note(0, 1, 1)["text"].endswith("Ask about Q3"))
t2 = b2.thread(0, 4, 2)
check("the thread is saved", t2 is not None and [e[2] for e in t2["entries"]] == ["Is this final?", "Not yet"], t2)

# Convert to Comments, Delete
win.run("convert-notes")
check("Convert to Comments", b.note(S, 1, 1) is None and b.thread(S, 1, 1)["entries"][0][2] == "Ask about Q3", b.thread(S, 1, 1))
win.run("undo")
g.sel.ranges = [(0, 0, 9, 9)]
g.sel.active = (0, 0)
win.run("clear-comments")
check("Clear Comments and Notes", b.notes(S) == [] and b.threads(S) == [])
win.run("undo")
check("undo brings them back", b.note(S, 1, 1) is not None and b.thread(S, 4, 2) is not None)

b.show_notes(S, [(1, 1)], True)
shot = os.environ.get("FOLIO_SHOT", "/tmp/notes_audit.png")
b.render_png(S, shot, g.params({"width": 600.0, "height": 260.0}))
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
