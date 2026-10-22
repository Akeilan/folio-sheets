"""The window's handle on the workbook.

The engine keeps the whole undo history itself (contents, formats, merges, sheets), so
this is a thin pass-through that adds grouping several engine edits into one undo
step (`with book.group(label): ...`).
"""


class HistoryBook:
    def __init__(self, book):
        object.__setattr__(self, "_book", book)
        object.__setattr__(self, "_depth", 0)
        object.__setattr__(self, "on_switch", None)

    @property
    def current(self):
        return self._book

    def __getattr__(self, name):
        return getattr(self._book, name)

    def __setattr__(self, name, value):
        if name == "on_switch":
            object.__setattr__(self, name, value)
        else:
            setattr(self._book, name, value)

    def group(self, label):
        hb = self

        class _G:
            def __enter__(self):
                hb._book.begin_group(label, hb._book.active_sheet)
                object.__setattr__(hb, "_depth", hb._depth + 1)

            def __exit__(self, *exc):
                object.__setattr__(hb, "_depth", hb._depth - 1)
                hb._book.end_group()
                return False
        return _G()

    def undo(self):
        label = self._book.undo()
        if label and self.on_switch:
            self.on_switch()
        return label

    def redo(self):
        label = self._book.redo()
        if label and self.on_switch:
            self.on_switch()
        return label
