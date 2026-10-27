"""Protect Sheet and Protect Workbook: the dialogs, passwords (hashed as the file format
wants: SHA-512 with a salt and 100,000 rounds; the old 16-bit hash is still read), and the
checks that keep a protected sheet's locked cells as they are."""
import base64
import hashlib
import os

from gi.repository import Gtk

REFUSED = ("The cell or chart you're trying to change is on a protected sheet. To make a change, unprotect "
           "the sheet. You might be requested to enter a password.")
STRUCTURE_REFUSED = "Workbook is protected and cannot be changed."
WRONG = ("The password you supplied is not correct. Verify that the CAPS LOCK key is off and be sure to use "
         "the correct capitalization.")

# what Protect Sheet can still allow, the attribute that forbids it, and whether it is
# allowed when the file doesn't say (as the file format has it)
PERMS = [("Select locked cells", "selectLockedCells", True), ("Select unlocked cells", "selectUnlockedCells", True),
         ("Format cells", "formatCells", False), ("Format columns", "formatColumns", False),
         ("Format rows", "formatRows", False), ("Insert columns", "insertColumns", False),
         ("Insert rows", "insertRows", False), ("Insert hyperlinks", "insertHyperlinks", False),
         ("Delete columns", "deleteColumns", False), ("Delete rows", "deleteRows", False),
         ("Sort", "sort", False), ("Use AutoFilter", "autoFilter", False), ("Use PivotTable & PivotChart", "pivotTables", False),
         ("Edit objects", "objects", True), ("Edit scenarios", "scenarios", True)]
DEFAULT_ON = {"selectLockedCells", "selectUnlockedCells"}      # ticked in a new dialog

# commands and what they need on a protected sheet: "cells" (unlocked cells only), a
# permission, or "never"
NEEDS = {
    "insert-rows": "insertRows", "insert-columns": "insertColumns", "delete-rows": "deleteRows",
    "delete-columns": "deleteColumns", "insert-cells": "never", "delete-cells": "never",
    "column-width": "formatColumns", "autofit-columns": "formatColumns", "hide-columns": "formatColumns",
    "unhide-columns": "formatColumns", "default-width": "formatColumns", "autofit-column-at": "formatColumns",
    "row-height": "formatRows", "autofit-rows": "formatRows", "hide-rows": "formatRows", "unhide-rows": "formatRows",
    "autofit-row-at": "formatRows",
    "sort-asc": "sort", "sort-desc": "sort", "sort-custom": "sort", "filter": "never", "filter-clear": "autoFilter",
    "filter-reapply": "autoFilter", "remove-duplicates": "never", "format-as-table": "never", "table": "never",
    "merge-center": "never", "merge-across": "never", "merge-cells": "never", "unmerge-cells": "never",
    "link": "insertHyperlinks", "chart": "objects", "new-note": "objects", "edit-note": "objects",
    "delete-note": "objects", "new-comment": "objects", "delete-comment": "objects",
    "paste": "cells", "paste-values": "cells", "paste-formulas": "cells", "paste-keep-widths": "cells",
    "paste-special": "cells", "paste-no-borders": "cells", "cut": "cells", "clear-contents": "cells",
    "clear-all": "never", "clear-formats": "formatCells", "fill-down": "cells", "fill-right": "cells",
    "fill-up": "cells", "fill-left": "cells", "fill-series": "cells", "fill-justify": "cells", "flash-fill": "cells",
    "autosum": "cells", "data-validation": "never", "text-to-columns": "never", "clear-hyperlinks": "cells",
    "remove-hyperlinks": "cells", "group": "never", "ungroup": "never", "subtotal": "never",
    "show-detail": "never", "hide-detail": "never", "spelling": "never",
    "equation": "objects", "equation-put": "objects", "shapes": "objects", "shape-insert": "objects",
    "text-box": "objects", "wordart": "objects", "wordart-insert": "objects", "screenshot": "objects",
    "pictures": "objects", "sparkline-line": "never", "sparkline-column": "never", "sparkline-winloss": "never",
    "spark-type": "never", "spark-show": "never", "spark-color": "never", "spark-weight": "never",
    "spark-marker": "never", "spark-clear": "never", "spark-clear-group": "never", "spark-group": "never",
    "spark-ungroup": "never", "spark-edit-data": "never",
    "pivot-options": "pivotTables", "pivot-field-settings": "pivotTables", "pivot-refresh": "pivotTables",
    "pivot-refresh-all": "pivotTables", "pivot-change-source": "pivotTables", "pivot-move": "never",
    "pivot-clear": "pivotTables", "pivot-design": "pivotTables", "pivot-sort": "pivotTables",
    "pivot-remove-field": "pivotTables", "pivot-summarize": "pivotTables", "pivot-show-as": "pivotTables",
    "pivot-number-format": "pivotTables", "pivot-field-headers": "pivotTables",
}
# sheet commands that Protect Workbook (structure) stops
STRUCTURE = {"insert-sheet", "delete-sheet", "rename-sheet", "move-copy-sheet", "hide-sheet", "unhide-sheet",
             "tab-color", "new-sheet"}
# commands that never change a protected sheet's cells (they may change the view)
FREE = {"protect-sheet", "protect-workbook", "unprotect-sheet", "undo", "redo", "copy", "save", "save-as",
        "find", "goto", "select-all", "zoom-in", "zoom-out", "zoom-100", "split", "freeze-panes", "new-window"}


# ------------------------------------------------------------------ passwords
def hash_password(password, salt=None, spin=100000):
    """The attributes for a password: SHA-512 of salt + the UTF-16 password, rehashed `spin` times."""
    salt = salt or os.urandom(16)
    h = hashlib.sha512(salt + password.encode("utf-16-le")).digest()
    for i in range(spin):
        h = hashlib.sha512(h + i.to_bytes(4, "little")).digest()
    return [("algorithmName", "SHA-512"), ("hashValue", base64.b64encode(h).decode()),
            ("saltValue", base64.b64encode(salt).decode()), ("spinCount", str(spin))]


def legacy_hash(password):
    """The old 16-bit password hash (the `password` attribute), as hex."""
    h = 0
    for ch in reversed(password[:15]):
        h = ((h >> 14) & 1) | ((h << 1) & 0x7FFF)
        h ^= ord(ch) & 0xFF
    h = ((h >> 14) & 1) | ((h << 1) & 0x7FFF)
    h ^= len(password[:15])
    h ^= 0xCE4B
    return "%04X" % h


def check_password(attrs, password, prefix=""):
    """Whether the password opens these protection attributes (prefix: "workbook" for the
    workbook's, whose attributes are workbookAlgorithmName and so on)."""
    a = dict(attrs or [])
    name = a.get(prefix + ("AlgorithmName" if prefix else "algorithmName"))
    if name:
        hv = a.get(prefix + ("HashValue" if prefix else "hashValue"), "")
        sv = a.get(prefix + ("SaltValue" if prefix else "saltValue"), "")
        spin = int(a.get(prefix + ("SpinCount" if prefix else "spinCount"), "0") or 0)
        algo = {"SHA-512": "sha512", "SHA-384": "sha384", "SHA-256": "sha256", "SHA-1": "sha1", "MD5": "md5"}.get(name)
        if algo is None:
            return False
        salt = base64.b64decode(sv) if sv else b""
        h = hashlib.new(algo, salt + password.encode("utf-16-le")).digest()
        for i in range(spin):
            h = hashlib.new(algo, h + i.to_bytes(4, "little")).digest()
        return base64.b64encode(h).decode() == hv
    old = a.get("workbookPassword" if prefix else "password")
    if old:
        return legacy_hash(password).upper() == old.upper()
    return True


def has_password(attrs, prefix=""):
    a = dict(attrs or [])
    return bool(a.get(prefix + ("HashValue" if prefix else "hashValue")) or
                a.get("workbookPassword" if prefix else "password"))


# ------------------------------------------------------------------ state
def protected(win, sheet=None):
    s = win.grid.sheet if sheet is None else sheet
    a = win.book.sheet_protection(s)
    return bool(a) and dict(a).get("sheet") in ("1", "true")


def allows(win, attr, sheet=None):
    """Whether the protected sheet still allows something (its attribute isn't set)."""
    a = dict(win.book.sheet_protection(win.grid.sheet if sheet is None else sheet) or [])
    default_allowed = next((d for _l, k, d in PERMS if k == attr), False)
    v = a.get(attr)
    if v is None:
        return default_allowed
    return v not in ("1", "true")


def structure_locked(win):
    a = dict(win.book.workbook_protection() or [])
    return a.get("lockStructure") in ("1", "true")


def refuse(win, text=REFUSED):
    win.dialogs.message(text)
    return False


def may_change_cells(win, ranges=None, tell=True):
    """Typing, pasting, clearing: only unlocked cells of a protected sheet."""
    if not protected(win):
        return True
    ranges = ranges or [tuple(r) for r in win.grid.sel.ranges]
    if not win.book.locked_in(win.grid.sheet, [tuple(r) for r in ranges]):
        return True
    return refuse(win) if tell else False


def may_format(win, ranges=None):
    """Formatting on a protected sheet needs Format cells (and then any cell may be formatted)."""
    if not protected(win) or allows(win, "formatCells"):
        return True
    return refuse(win)


def need_of(base):
    need = NEEDS.get(base)
    if need is None:
        for prefix, n in (("autosum", "cells"), ("paste", "cells"), ("fill-", "cells"), ("sort-", "sort")):
            if base.startswith(prefix):
                return n
    return need


def gate(win, cmd):
    """Before a command runs. False: refused (and said why)."""
    base = cmd.split(":", 1)[0]
    if base in STRUCTURE and structure_locked(win):
        return refuse(win, STRUCTURE_REFUSED)
    if not protected(win) or base in FREE:
        return True
    need = need_of(base)
    if need is None:
        return True
    if need == "never":
        return refuse(win)
    if need == "cells":
        return may_change_cells(win)
    if need == "formatCells":
        return may_format(win)
    if not allows(win, need):
        return refuse(win)
    if need == "insertHyperlinks":
        return may_change_cells(win)
    return True


def hides_formula(win, r, c):
    """A protected sheet keeps a Hidden cell's formula out of the formula bar."""
    if not protected(win):
        return False
    sp = win.book.style_spec(win.grid.sheet, r, c)
    return bool(sp.get("protection", {}).get("hidden"))


def may_select(win, r, c):
    if not protected(win):
        return True
    locked = win.book.locked_in(win.grid.sheet, [(r, c, r, c)])
    return allows(win, "selectLockedCells") if locked else allows(win, "selectUnlockedCells")


# ------------------------------------------------------------------ dialogs
class ProtectSheetDialog(Gtk.Dialog):
    def __init__(self, win):
        Gtk.Dialog.__init__(self, title="Protect Sheet", transient_for=win, modal=True)
        self.get_style_context().add_class("folio-dialog")
        self.set_resizable(False)
        self.add_button("OK", Gtk.ResponseType.OK)
        self.add_button("Cancel", Gtk.ResponseType.CANCEL)
        self.set_default_response(Gtk.ResponseType.OK)
        box = self.get_content_area()
        box.set_border_width(12)
        box.set_spacing(6)
        self.contents = Gtk.CheckButton(label="Protect worksheet and contents of locked cells")
        self.contents.set_active(True)
        box.pack_start(self.contents, False, False, 0)
        box.pack_start(Gtk.Label(label="Password to unprotect sheet:", xalign=0), False, False, 0)
        self.password = Gtk.Entry(visibility=False, activates_default=True)
        box.pack_start(self.password, False, False, 0)
        box.pack_start(Gtk.Label(label="Allow all users of this worksheet to:", xalign=0), False, False, 4)
        lb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.perms = {}
        for label, key, _d in PERMS:
            cb = Gtk.CheckButton(label=label)
            cb.set_active(key in DEFAULT_ON)
            self.perms[key] = cb
            lb.pack_start(cb, False, False, 0)
        sw = Gtk.ScrolledWindow()
        sw.set_shadow_type(Gtk.ShadowType.IN)
        sw.set_size_request(280, 180)
        sw.add(lb)
        box.pack_start(sw, True, True, 0)
        self.contents.connect("toggled", lambda b: self.set_response_sensitive(Gtk.ResponseType.OK, b.get_active()))

    def attrs(self):
        out = [("sheet", "1")]
        for _l, key, default_allowed in PERMS:
            on = self.perms[key].get_active()
            # write what differs from the file format's defaults (as spreadsheet programs do)
            if on != default_allowed:
                out.append((key, "0" if on else "1"))
        return out


def confirm_password(win, pw):
    t = win.dialogs.ask_password("Confirm Password", "Reenter password to proceed.")
    if t is None:
        return False
    if t != pw:
        win.dialogs.message("The confirmation password is not identical.")
        return False
    return True


def protect_sheet(win):
    g = win.grid
    b, s = win.book, g.sheet
    if protected(win):
        return unprotect_sheet(win)
    d = ProtectSheetDialog(win)
    d.show_all()
    r = d.run()
    attrs, pw = d.attrs(), d.password.get_text()
    d.destroy()
    if r != Gtk.ResponseType.OK:
        return False
    if pw:
        if not confirm_password(win, pw):
            return False
        attrs += hash_password(pw)
    b.set_sheet_protection(s, attrs)
    after(win)
    return True


def unprotect_sheet(win):
    b, s = win.book, win.grid.sheet
    a = b.sheet_protection(s)
    if has_password(a):
        pw = win.dialogs.ask_password("Unprotect Sheet", "Password:")
        if pw is None:
            return False
        if not check_password(a, pw):
            win.dialogs.message(WRONG)
            return False
    b.set_sheet_protection(s, None)
    after(win)
    return True


def protect_workbook(win):
    b = win.book
    a = b.workbook_protection()
    if structure_locked(win):
        if has_password(a, "workbook"):
            pw = win.dialogs.ask_password("Unprotect Workbook", "Password:")
            if pw is None:
                return False
            if not check_password(a, pw, "workbook"):
                win.dialogs.message(WRONG)
                return False
        b.set_workbook_protection(None)
        after(win)
        return True
    d = Gtk.Dialog(title="Protect Structure and Windows", transient_for=win, modal=True)
    d.get_style_context().add_class("folio-dialog")
    d.add_button("OK", Gtk.ResponseType.OK)
    d.add_button("Cancel", Gtk.ResponseType.CANCEL)
    d.set_default_response(Gtk.ResponseType.OK)
    box = d.get_content_area()
    box.set_border_width(12)
    box.set_spacing(6)
    box.pack_start(Gtk.Label(label="Protect workbook for", xalign=0), False, False, 0)
    structure = Gtk.CheckButton(label="Structure")
    structure.set_active(True)
    box.pack_start(structure, False, False, 0)
    box.pack_start(Gtk.Label(label="Password (optional):", xalign=0), False, False, 4)
    pw_entry = Gtk.Entry(visibility=False, activates_default=True)
    box.pack_start(pw_entry, False, False, 0)
    d.structure, d.password = structure, pw_entry
    d.show_all()
    r = d.run()
    on, pw = structure.get_active(), pw_entry.get_text()
    d.destroy()
    if r != Gtk.ResponseType.OK or not on:
        return False
    attrs = [("lockStructure", "1")]
    if pw:
        if not confirm_password(win, pw):
            return False
        attrs += [("workbook" + k[0].upper() + k[1:], v) for k, v in hash_password(pw)]
    b.set_workbook_protection(attrs)
    after(win)
    return True


def after(win):
    """The ribbon and title follow protection."""
    win.ribbon.update("protect-sheet", "true" if protected(win) else "false")
    win.ribbon.update("protect-workbook", "true" if structure_locked(win) else "false")
    for key, label in (("protect-sheet", "Unprotect\nSheet" if protected(win) else "Protect\nSheet"),
                       ("protect-workbook", "Protect\nWorkbook")):
        for c in getattr(win.ribbon, "controls", []):
            if getattr(c, "action", None) == key and hasattr(c, "set_label_text"):
                c.set_label_text(label)
    win.update_title()
    win.on_selection_changed()



def after_command(win, base, h0):
    """The safety net: a command that made a step on a protected sheet without being
    let through, or a paste or fill that reached locked cells, is taken back."""
    if win.book.history_state == h0 or base in ("undo", "redo"):
        return
    bad = not getattr(win, "_approved", False)
    if not bad and need_of(base) == "cells":
        # what the command left selected (a paste selects what it filled) must have been
        # unlocked before it ran: look at it as it was, then step forward again
        rs = [tuple(r) for r in win.grid.sel.ranges]
        win.book.undo()
        bad = win.book.locked_in(win.grid.sheet, rs)
        win.book.redo()
    if bad:
        win.book.undo()
        win.book.forget_redo()
        win.grid.refresh()
        refuse(win)
