"""One undo history across engine edits and file-level formatting.

A formatting command (restyle.py) produces a new engine Book from edited file bytes.
The previous Book is kept, untouched, at exactly the state before the command, with
its own undo stack intact. So the history is a chain of Books ("segments"):

    undo:  undo inside the current Book; when it has nothing left, step back to the
           previous Book (which sits at the state just before the formatting command)
    redo:  redo inside the current Book; when it has nothing left, step forward
    edit:  any change to the current Book drops the segments after it

The window talks to a HistoryBook exactly as it would to a Book.
"""
import os
import tempfile

# Book methods that change the workbook (and so end any redo beyond this point)
MUTATORS = {
    "adjust_decimals", "autofit_cols", "autofit_rows", "clear_contents", "define_name", "delete_name",
    "delete_sheet", "insert_sheet", "move_sheet", "rename_sheet", "set_col_px", "set_col_width",
    "set_cols_hidden", "set_default_col_width", "set_input", "set_number_format", "set_row_height",
    "set_row_px", "set_rows_hidden", "set_sheet_state", "set_tab_color", "set_iterate",
}
MAX_SEGMENTS = 60


class Segment:
    __slots__ = ("book", "label", "dirty", "base", "groups", "redo_groups")

    def __init__(self, book, label, base=0):
        self.book = book
        self.label = label      # the formatting command that produced this Book
        self.dirty = False      # unsaved changes the engine's own flag no longer shows
        self.base = base        # engine undo steps that belong to the command itself
        self.groups = []        # (start, end, label): engine steps undone as one
        self.redo_groups = []

    def own_undo(self):
        return len(self.book.undo_labels()) > self.base


class HistoryBook:
    def __init__(self, book):
        object.__setattr__(self, "_segs", [Segment(book, None)])
        object.__setattr__(self, "_idx", 0)
        object.__setattr__(self, "_saved", 0)
        object.__setattr__(self, "on_switch", None)

    # ---- plumbing
    @property
    def current(self):
        return self._segs[self._idx].book

    def __getattr__(self, name):
        attr = getattr(self._segs[self._idx].book, name)
        if name in MUTATORS and callable(attr):
            def call(*a, **k):
                self._truncate()
                self._segs[self._idx].redo_groups.clear()
                return attr(*a, **k)
            return call
        return attr

    def __setattr__(self, name, value):
        if name in ("on_switch",):
            object.__setattr__(self, name, value)
        else:
            if name in ("calc_manual",):
                self._truncate()
            setattr(self._segs[self._idx].book, name, value)

    def _truncate(self):
        if self._idx < len(self._segs) - 1:
            del self._segs[self._idx + 1:]
            if self._saved > self._idx:
                object.__setattr__(self, "_saved", -1)

    def _switch(self, idx):
        old = self.current
        object.__setattr__(self, "_idx", idx)
        new = self.current
        try:
            new.active_sheet = old.active_sheet
        except Exception:
            pass
        if self.on_switch:
            self.on_switch()

    # ---- several engine edits as one undo step
    def group(self, label):
        hb = self

        class _G:
            def __enter__(self):
                self.start = len(hb.current.undo_labels())
                self.seg = hb._segs[hb._idx]

            def __exit__(self, *exc):
                if hb._segs[hb._idx] is not self.seg:
                    return False
                end = len(hb.current.undo_labels())
                if end - self.start > 1:
                    self.seg.groups.append((self.start, end, label))
                return False
        return _G()

    # ---- the workbook as bytes, without disturbing it
    def snapshot(self):
        """The current workbook as file bytes. The engine's save() clears its modified
        flag, so remember that the workbook still has unsaved changes."""
        seg = self._segs[self._idx]
        was = seg.book.modified
        fd, path = tempfile.mkstemp(prefix="folio-sheets-", suffix=".xlsx", dir=_tmpdir())
        os.close(fd)
        try:
            seg.book.save(path)
            with open(path, "rb") as f:
                data = f.read()
        finally:
            try:
                os.unlink(path)
            except OSError:
                pass
        if was:
            seg.dirty = True
        return data

    def push(self, book, label):
        """A formatting command made a new Book: it becomes the current state. Engine
        edits already made on it (contents a merge discards) are part of the command."""
        self._truncate()
        self._segs.append(Segment(book, label, len(book.undo_labels())))
        object.__setattr__(self, "_idx", self._idx + 1)
        if len(self._segs) > MAX_SEGMENTS:
            del self._segs[0]
            object.__setattr__(self, "_idx", self._idx - 1)
            object.__setattr__(self, "_saved", self._saved - 1 if self._saved > 0 else -1)
        if self.on_switch:
            self.on_switch()

    # ---- undo / redo
    @property
    def can_undo(self):
        return self._segs[self._idx].own_undo() or self._idx > 0

    @property
    def can_redo(self):
        return self.current.can_redo or self._idx < len(self._segs) - 1

    def undo(self):
        seg = self._segs[self._idx]
        if seg.own_undo():
            n = len(seg.book.undo_labels())
            if seg.groups and seg.groups[-1][1] == n:
                g = seg.groups.pop()
                for _ in range(g[1] - g[0]):
                    seg.book.undo()
                seg.redo_groups.append(g)
                return g[2]
            return self.current.undo()
        if self._idx > 0:
            label = self._segs[self._idx].label
            self._switch(self._idx - 1)
            return label
        return None

    def redo(self):
        seg = self._segs[self._idx]
        if seg.book.can_redo:
            n = len(seg.book.undo_labels())
            if seg.redo_groups and seg.redo_groups[-1][0] == n:
                g = seg.redo_groups.pop()
                for _ in range(g[1] - g[0]):
                    seg.book.redo()
                seg.groups.append(g)
                return g[2]
            return self.current.redo()
        if self._idx < len(self._segs) - 1:
            self._switch(self._idx + 1)
            return self._segs[self._idx].label
        return None

    def undo_labels(self):
        out = []
        for i in range(self._idx, -1, -1):
            seg = self._segs[i]
            labels = list(seg.book.undo_labels())
            n = len(labels)
            ends = {e: (st, lab) for st, e, lab in seg.groups}
            pos = n
            while pos > seg.base:
                if pos in ends:
                    out.append(ends[pos][1])
                    pos = ends[pos][0]
                else:
                    out.append(labels[n - pos])
                    pos -= 1
            if i:
                out.append(seg.label)
        return out

    # ---- saved state
    @property
    def modified(self):
        seg = self._segs[self._idx]
        return self._idx != self._saved or seg.dirty or seg.book.modified

    def mark_saved(self):
        self.current.mark_saved()
        self._segs[self._idx].dirty = False
        object.__setattr__(self, "_saved", self._idx)

    def save(self, path):
        self.current.save(path)
        self._segs[self._idx].dirty = False
        object.__setattr__(self, "_saved", self._idx)


def _tmpdir():
    d = "/dev/shm"
    return d if os.path.isdir(d) and os.access(d, os.W_OK) else None
