"""Review > Spelling and Thesaurus, offline: Hunspell dictionaries (hunspell-xx packages) for
spelling, MyThes data (mythes-xx packages) for the thesaurus. Words added to the dictionary
are kept in the user's own word list."""
import bisect
import ctypes
import ctypes.util
import glob
import locale
import os
import re

from gi.repository import GLib, Gtk

DICT_DIRS = ["/usr/share/hunspell", "/usr/share/myspell", "/usr/share/myspell/dicts"]
THES_DIRS = ["/usr/share/mythes", "/usr/share/myspell/dicts"]
WORD = re.compile(r"[^\W\d_]+(?:['’][^\W\d_]+)*")
NOT_FOUND = ("No dictionary for checking spelling was found. Install a Hunspell dictionary (for example "
             "the hunspell-en-us package) and try again.")


def user_words_path():
    return os.path.join(GLib.get_user_config_dir(), "folio-sheets", "custom.dic")


def dictionaries():
    """The installed dictionaries: {language code: (aff, dic)}."""
    out = {}
    for d in DICT_DIRS:
        for dic in sorted(glob.glob(os.path.join(d, "*.dic"))):
            aff = dic[:-4] + ".aff"
            code = os.path.basename(dic)[:-4]
            if os.path.exists(aff) and code not in out and not code.startswith("hyph"):
                out[code] = (aff, dic)
    return out


def default_language(available):
    loc = (locale.getlocale()[0] or os.environ.get("LANG", "") or "en_US").split(".")[0]
    for cand in (loc, loc.split("_")[0], "en_US", "en_GB"):
        for code in available:
            if code == cand or code.split("_")[0] == cand:
                return code
    return next(iter(available), None)


class Speller:
    """libhunspell through ctypes."""
    _lib = None

    @classmethod
    def lib(cls):
        if cls._lib is None:
            name = ctypes.util.find_library("hunspell-1.7") or ctypes.util.find_library("hunspell") or \
                "libhunspell-1.7.so.0"
            try:
                lib = ctypes.CDLL(name)
            except OSError:
                lib = False
            if lib:
                lib.Hunspell_create.restype = ctypes.c_void_p
                lib.Hunspell_create.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
                lib.Hunspell_destroy.argtypes = [ctypes.c_void_p]
                lib.Hunspell_spell.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
                lib.Hunspell_suggest.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.POINTER(ctypes.c_char_p)),
                                                 ctypes.c_char_p]
                lib.Hunspell_free_list.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.POINTER(ctypes.c_char_p)),
                                                   ctypes.c_int]
                lib.Hunspell_add.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
                lib.Hunspell_get_dic_encoding.restype = ctypes.c_char_p
                lib.Hunspell_get_dic_encoding.argtypes = [ctypes.c_void_p]
            cls._lib = lib
        return cls._lib or None

    def __init__(self, aff, dic):
        lib = self.lib()
        if lib is None:
            raise OSError("libhunspell is not installed")
        self.h = lib.Hunspell_create(aff.encode(), dic.encode())
        if not self.h:
            raise OSError("the dictionary could not be read")
        enc = lib.Hunspell_get_dic_encoding(self.h)
        self.enc = (enc or b"UTF-8").decode("ascii", "replace")
        self.added = set()
        for w in read_user_words():
            self.add(w, keep=False)

    def _b(self, word):
        return word.replace("’", "'").encode(self.enc, "replace")

    def check(self, word):
        if word in self.added:
            return True
        return bool(self.lib().Hunspell_spell(self.h, self._b(word)))

    def suggest(self, word, limit=12):
        lst = ctypes.POINTER(ctypes.c_char_p)()
        n = self.lib().Hunspell_suggest(self.h, ctypes.byref(lst), self._b(word))
        out = [lst[i].decode(self.enc, "replace") for i in range(n)]
        if n:
            self.lib().Hunspell_free_list(self.h, ctypes.byref(lst), n)
        return out[:limit]

    def add(self, word, keep=True):
        self.lib().Hunspell_add(self.h, self._b(word))
        self.added.add(word)
        if keep:
            write_user_word(word)

    def close(self):
        if self.h:
            self.lib().Hunspell_destroy(self.h)
            self.h = None


def read_user_words():
    try:
        with open(user_words_path(), encoding="utf-8") as f:
            return [ln.strip() for ln in f if ln.strip()]
    except OSError:
        return []


def write_user_word(word):
    p = user_words_path()
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "a", encoding="utf-8") as f:
        f.write(word + "\n")


def skip_word(word, ignore_upper=True):
    if ignore_upper and len(word) > 1 and word.isupper():
        return True
    return False


def words_in(text):
    """(start, end, word) for each word of a cell's text; addresses and words with numbers are left out."""
    out = []
    for tok in re.finditer(r"\S+", text):
        t = tok.group(0)
        if "://" in t or t.startswith("www.") or "@" in t or any(ch.isdigit() for ch in t):
            continue
        for m in WORD.finditer(t):
            out.append((tok.start() + m.start(), tok.start() + m.end(), m.group(0)))
    return out


# ------------------------------------------------------------------ the spelling check
class SpellDialog(Gtk.Dialog):
    def __init__(self, win, languages, lang):
        Gtk.Dialog.__init__(self, title="Spelling", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.set_resizable(False)
        g = Gtk.Grid(row_spacing=6, column_spacing=10, border_width=10)
        self.title_label = Gtk.Label(label="Not in Dictionary:", xalign=0)
        g.attach(self.title_label, 0, 0, 1, 1)
        self.word = Gtk.Entry(hexpand=True)
        g.attach(self.word, 0, 1, 1, 2)
        g.attach(Gtk.Label(label="Suggestions:", xalign=0), 0, 3, 1, 1)
        self.store = Gtk.ListStore(str)
        self.list = Gtk.TreeView(model=self.store, headers_visible=False)
        self.list.append_column(Gtk.TreeViewColumn("", Gtk.CellRendererText(), text=0))
        self.list.get_selection().connect("changed", lambda *_: self._picked())
        sw = Gtk.ScrolledWindow()
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.set_size_request(300, 140)
        sw.add(self.list)
        g.attach(sw, 0, 4, 1, 4)
        self.buttons = {}
        for i, (key, label) in enumerate([("ignore", "Ignore Once"), ("ignore-all", "Ignore All"),
                                          ("add", "Add to Dictionary"), ("change", "Change"),
                                          ("change-all", "Change All"), ("close", "Cancel")]):
            bt = Gtk.Button(label=label)
            bt.connect("clicked", lambda _b, k=key: self.response(100 + ["ignore", "ignore-all", "add", "change",
                                                                         "change-all", "close"].index(k)))
            self.buttons[key] = bt
            g.attach(bt, 1, 1 + i + (1 if i >= 3 else 0), 1, 1)
        g.attach(Gtk.Label(label="Dictionary language:", xalign=0), 0, 8, 1, 1)
        self.lang = Gtk.ComboBoxText()
        self.langs = list(languages)
        for code in self.langs:
            self.lang.append_text(code.replace("_", "-"))
        self.lang.set_active(self.langs.index(lang) if lang in self.langs else 0)
        g.attach(self.lang, 0, 9, 1, 1)
        self.get_content_area().pack_start(g, True, True, 0)

    def show_word(self, word, suggestions):
        self.word.set_text(word)
        self.store.clear()
        for s in suggestions or ["(No Suggestions)"]:
            self.store.append([s])
        if suggestions:
            self.list.get_selection().select_path(Gtk.TreePath(0))
        self.buttons["change"].set_sensitive(bool(suggestions))
        self.buttons["change-all"].set_sensitive(bool(suggestions))

    def _picked(self):
        m, it = self.list.get_selection().get_selected()
        if it is not None and m[it][0] != "(No Suggestions)":
            self.buttons["change"].set_sensitive(True)
            self.buttons["change-all"].set_sensitive(True)

    def replacement(self):
        """What Change puts in: the edited word if it was retyped, else the suggestion picked."""
        m, it = self.list.get_selection().get_selected()
        typed = self.word.get_text()
        if it is not None and m[it][0] != "(No Suggestions)" and typed == self.current:
            return m[it][0]
        return typed


def cells_to_check(win):
    """The text cells to check, in order from the active cell (whole sheet) or the selection."""
    g = win.grid
    b, s = win.book, g.sheet
    rg = tuple(g.sel.ranges[-1])
    whole = rg[0] == rg[2] and rg[1] == rg[3]
    if whole:
        ext = b.used_extent(s)
        if not ext:
            return [], True
        area = (0, 0, ext[0], ext[1])
    else:
        ext = b.used_extent(s) or (0, 0)
        area = (rg[0], rg[1], min(rg[2], ext[0]), min(rg[3], ext[1]))
    cells = []
    for r in range(area[0], area[2] + 1):
        for c in range(area[1], area[3] + 1):
            t = b.input(s, r, c)
            if t and not t.startswith("=") and not _is_number(t):
                cells.append((r, c))
    if whole:
        # from the active cell on, then from the top
        a = (rg[0], rg[1])
        cells = [x for x in cells if x >= a] + [x for x in cells if x < a]
    return cells, whole


def spelling(win, speller_factory=None):
    g = win.grid
    b, s = win.book, g.sheet
    dicts = dictionaries()
    if not dicts or Speller.lib() is None:
        win.dialogs.message(NOT_FOUND, "Spelling")
        return False
    lang = win.app.settings.get("spelling_language") or default_language(dicts)
    cells, whole = cells_to_check(win)
    make = speller_factory or (lambda code: Speller(*dicts[code]))
    sp = make(lang)
    ignored, changes = set(), {}
    dlg = None
    found_any = False
    try:
        for (r, c) in cells:
            text = b.input(s, r, c)
            if text.startswith("'"):
                text = text[1:]
            pos = 0
            while True:
                bad = None
                for a, z, w in words_in(text):
                    if a < pos or w in ignored or skip_word(w) or sp.check(w):
                        continue
                    bad = (a, z, w)
                    break
                if bad is None:
                    break
                a, z, w = bad
                if w in changes:
                    new = changes[w]
                    text = text[:a] + new + text[z:]
                    b.set_input(s, r, c, text)
                    pos = a + len(new)
                    continue
                found_any = True
                g.select_cell(r, c)
                if dlg is None:
                    dlg = SpellDialog(win, dicts, lang)
                    dlg.show_all()
                dlg.current = w
                dlg.show_word(w, sp.suggest(w))
                resp = dlg.run()
                code = dlg.langs[max(dlg.lang.get_active(), 0)]
                if code != lang:
                    lang = code
                    win.app.settings["spelling_language"] = code
                    sp.close()
                    sp = make(lang)
                    continue          # the same word, with the new dictionary
                if resp in (105, Gtk.ResponseType.DELETE_EVENT, Gtk.ResponseType.CANCEL):
                    return True
                if resp == 100:
                    pos = z
                elif resp == 101:
                    ignored.add(w)
                elif resp == 102:
                    sp.add(w)
                elif resp in (103, 104):
                    new = dlg.replacement()
                    if resp == 104:
                        changes[w] = new
                    text = text[:a] + new + text[z:]
                    b.set_input(s, r, c, text)
                    pos = a + len(new)
                else:
                    return True
    finally:
        if dlg is not None:
            dlg.destroy()
        sp.close()
        g.refresh()
        win.update_title()
    win.dialogs.info("The spelling check is complete for the entire sheet." if whole else
                     "The spelling check is complete for the selected cells.", "Spelling")
    return found_any


def _is_number(t):
    try:
        float(t.replace(",", ""))
        return True
    except ValueError:
        return False


# ------------------------------------------------------------------ the thesaurus
class Thesaurus:
    """A MyThes data file (.idx + .dat): word -> [(part of speech, [synonyms])]."""

    def __init__(self, idx, dat):
        self.dat = dat
        with open(idx, "rb") as f:
            enc = f.readline().strip().decode("ascii", "replace") or "UTF-8"
            f.readline()
            self.words, self.offsets = [], []
            for ln in f:
                w, _, off = ln.rstrip(b"\n").rpartition(b"|")
                self.words.append(w.decode(enc, "replace").lower())
                self.offsets.append(int(off))
        self.enc = enc

    def lookup(self, word):
        w = word.strip().lower()
        i = bisect.bisect_left(self.words, w)
        if i >= len(self.words) or self.words[i] != w:
            return []
        out = []
        with open(self.dat, "rb") as f:
            f.seek(self.offsets[i])
            head = f.readline().decode(self.enc, "replace").strip()
            n = int(head.rpartition("|")[2] or 0)
            for _ in range(n):
                parts = f.readline().decode(self.enc, "replace").strip().split("|")
                pos = parts[0].strip("()")
                syns = [re.sub(r"\s*\((similar|related|antonym|generic|specific) term\)$", "", p) for p in parts[1:]
                        if "(antonym)" not in p]
                out.append((pos, syns))
        return out

    def related(self, word):
        """Close matches when the word itself isn't there."""
        w = word.strip().lower()
        i = bisect.bisect_left(self.words, w)
        return self.words[max(0, i - 3):i + 3]


def thesauri():
    out = {}
    for d in THES_DIRS:
        for idx in sorted(glob.glob(os.path.join(d, "th_*.idx"))):
            dat = idx[:-4] + ".dat"
            code = os.path.basename(idx)[3:-4].replace("_v2", "")
            if os.path.exists(dat) and code not in out:
                out[code] = (idx, dat)
    return out


_THES = {}


def thesaurus_for(code, files):
    if code not in _THES:
        _THES[code] = Thesaurus(*files)
    return _THES[code]


class ThesaurusDialog(Gtk.Dialog):
    def __init__(self, win, th, word):
        Gtk.Dialog.__init__(self, title="Thesaurus", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.th = th
        self.insert_btn = self.add_button("Insert", 1)
        self.add_button("Close", Gtk.ResponseType.CLOSE)
        self.set_default_size(340, 420)
        box = self.get_content_area()
        box.set_border_width(10)
        box.set_spacing(6)
        row = Gtk.Box(spacing=6)
        self.search = Gtk.SearchEntry(hexpand=True)
        self.search.set_text(word)
        self.search.connect("activate", lambda *_: self.look(self.search.get_text()))
        row.pack_start(self.search, True, True, 0)
        go = Gtk.Button(label="Look Up")
        go.connect("clicked", lambda *_: self.look(self.search.get_text()))
        row.pack_start(go, False, False, 0)
        box.pack_start(row, False, False, 0)
        self.store = Gtk.TreeStore(str, bool)
        self.view = Gtk.TreeView(model=self.store, headers_visible=False)
        rend = Gtk.CellRendererText()
        col = Gtk.TreeViewColumn("", rend, text=0)
        col.add_attribute(rend, "weight-set", 1)
        rend.set_property("weight", 700)
        self.view.append_column(col)
        self.view.connect("row-activated", lambda v, path, c: self._activated(path))
        self.view.get_selection().connect("changed", lambda *_: self._sel())
        sw = Gtk.ScrolledWindow(vexpand=True)
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.add(self.view)
        box.pack_start(sw, True, True, 0)
        self.note = Gtk.Label(xalign=0, wrap=True)
        box.pack_start(self.note, False, False, 0)
        self.look(word)

    def look(self, word):
        self.store.clear()
        self.word = word
        res = self.th.lookup(word) if word.strip() else []
        if not res:
            near = self.th.related(word) if word.strip() else []
            self.note.set_text("No results were found for “%s”." % word if word.strip() else
                               "Type a word to look up.")
            for w in near:
                self.store.append(None, [w, False])
        else:
            self.note.set_text("")
            for pos, syns in res:
                parent = self.store.append(None, ["%s (%s)" % (word, pos), True])
                for sy in syns:
                    self.store.append(parent, [sy, False])
            self.view.expand_all()
        self._sel()
        return res

    def chosen(self):
        m, it = self.view.get_selection().get_selected()
        if it is None or m[it][1]:
            return None
        return m[it][0]

    def _sel(self):
        self.insert_btn.set_sensitive(self.chosen() is not None)

    def _activated(self, path):
        w = self.store[path][0]
        if not self.store[path][1]:
            self.search.set_text(w)
            self.look(w)


def thesaurus(win):
    g = win.grid
    b, s = win.book, g.sheet
    found = thesauri()
    if not found:
        win.dialogs.message("No thesaurus was found. Install thesaurus data (for example the mythes-en-us "
                            "package) and try again.", "Thesaurus")
        return None
    code = default_language(found)
    th = thesaurus_for(code, found[code])
    r, c = g.sel.active
    text = b.input(s, r, c)
    word = text if text and not text.startswith("=") and len(text.split()) <= 3 else ""
    d = ThesaurusDialog(win, th, word)
    d.show_all()
    while True:
        resp = d.run()
        if resp == 1 and d.chosen():
            new = d.chosen()
            d.destroy()
            from .protect_ui import may_change_cells
            if not may_change_cells(win, [(r, c, r, c)]):
                return None
            with b.group("Thesaurus"):
                b.set_input(s, r, c, new)
            g.refresh()
            win.update_title()
            return new
        if resp == 1:
            continue
        d.destroy()
        return None
