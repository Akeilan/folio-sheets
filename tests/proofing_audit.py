# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): Spelling (Hunspell) and the Thesaurus (MyThes),
# offline: the dialog's buttons, the scope, the user's word list.
import os
import sys
import tempfile
import time
import traceback

from gi.repository import GLib, Gtk

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
    word = d.word.get_text() if hasattr(d, "word") and isinstance(getattr(d, "word"), Gtk.Entry) else None
    SEEN.append((d.get_title(), text or word))
    fn = SCRIPT.pop(0) if SCRIPT else (lambda d: Gtk.ResponseType.OK)
    try:
        return fn(d)
    except Exception:
        ERRORS.append(traceback.format_exc())
        return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = fake_run
os.environ["XDG_CONFIG_HOME"] = tempfile.mkdtemp()
GLib.get_user_config_dir = lambda: os.environ["XDG_CONFIG_HOME"]

from folio_sheets import proofing as P  # noqa: E402

dicts = P.dictionaries()
check("an installed dictionary is found", "en_US" in dicts, list(dicts))
sp = P.Speller(*dicts["en_US"])
check("Hunspell knows a word", sp.check("spreadsheet") and sp.check("colour") is False or sp.check("color"))
check("and suggests for a misspelling", "receive" in sp.suggest("recieve"), sp.suggest("recieve"))
sp.close()
check("words with digits, addresses and capitals are left alone",
      [w for _a, _z, w in P.words_in("abc123 www.x.org me@x.org NASA don't")] == ["NASA", "don't"])

g = win.grid
b = win.book
S = 0
b.set_input(S, 0, 0, "Teh quick brown fox")
b.set_input(S, 1, 0, "Teh end")
b.set_input(S, 2, 0, "=1+1")
b.set_input(S, 3, 0, "Folio is gr8")
b.set_input(S, 4, 0, "a recieve b")
b.set_input(S, 5, 0, "zzyzxq")


def resp(code, look=None):
    def fn(d):
        if look:
            look(d)
        return code
    return fn


seen = {}
g.select_range((0, 0, 0, 0), (0, 0))
def choose(d, word):
    for i, row in enumerate(d.store):
        if row[0] == word:
            d.list.get_selection().select_path(Gtk.TreePath(i))


SCRIPT.append(resp(104, lambda d: (seen.update(w=d.word.get_text(), offered=[r[0] for r in d.store]),
                                   choose(d, "The"))))   # Change All: Teh -> The
SCRIPT.append(resp(103))                       # recieve -> receive
SCRIPT.append(resp(102))                       # zzyzxq: add to dictionary
win.run("spelling")
check("the first word not in the dictionary is offered", seen.get("w") == "Teh" and "The" in seen.get("offered", []), seen)
check("Change All changes it everywhere", b.input(S, 0, 0) == "The quick brown fox" and b.input(S, 1, 0) == "The end",
      (b.input(S, 0, 0), b.input(S, 1, 0)))
check("Change takes the suggestion", b.input(S, 4, 0) == "a receive b")
check("formulas and words with numbers aren't checked", b.input(S, 2, 0) == "=1+1" and b.input(S, 3, 0) == "Folio is gr8")
check("it ends with the sheet done", SEEN[-1][1] == "The spelling check is complete for the entire sheet.", SEEN[-1])
words = open(P.user_words_path()).read()
check("Add to Dictionary keeps the word", "zzyzxq" in words)
win.run("undo")
check("one undo takes the changes back", b.input(S, 0, 0) == "Teh quick brown fox" and b.input(S, 4, 0) == "a recieve b")
SCRIPT.append(resp(101))                       # Ignore All: Teh
SEEN.clear()
SCRIPT.append(resp(100))                       # Ignore Once: recieve
win.run("spelling")
check("the added word is known now, Ignore All skips the rest",
      [w for _t, w in SEEN[:-1]] == ["Teh", "recieve"], SEEN)
g.select_range((4, 0, 5, 0), (4, 0))
SEEN.clear()
SCRIPT.append(resp(103, lambda d: d.word.set_text("received")))
win.run("spelling")
check("a selection is checked alone", SEEN[0][1] == "recieve" and len(SEEN) == 2, SEEN)
check("a retyped word goes in with Change", b.input(S, 4, 0) == "a received b", b.input(S, 4, 0))
check("and the end says the selection is done", SEEN[-1][1] == "The spelling check is complete for the selected cells.")
SCRIPT.append(resp(105))
g.select_range((0, 0, 0, 0), (0, 0))
win.run("spelling")
check("Cancel stops", b.input(S, 0, 0) == "Teh quick brown fox")

# ---- thesaurus
th = P.thesauri()
check("thesaurus data is found", "en_US" in th, list(th))
t = P.thesaurus_for("en_US", th["en_US"])
res = t.lookup("happy")
check("a word's meanings and synonyms", res and res[0][0] == "adj" and "glad" in res[0][1], res[:1])
check("an unknown word gives near words", not t.lookup("happyxq") and t.related("happyxq"))
b.set_input(S, 10, 0, "happy")
g.select_range((10, 0, 10, 0), (10, 0))


def pick(d):
    seen["looked"] = d.word
    it = d.store.get_iter_first()
    child = d.store.iter_children(it)
    while child is not None and d.store[child][0] != "glad":
        child = d.store.iter_next(child)
    d.view.get_selection().select_iter(child)
    return 1


SCRIPT.append(pick)
win.run("thesaurus")
check("the Thesaurus looks up the active cell's word", seen.get("looked") == "happy")
check("Insert puts the synonym in the cell", b.input(S, 10, 0) == "glad", b.input(S, 10, 0))

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
