"""The selection: one or more ranges, the active cell and the anchor,
moved the way the original moves them."""
from .core import MAX_COLS, MAX_ROWS


def norm(r1, c1, r2, c2):
    return (min(r1, r2), min(c1, c2), max(r1, r2), max(c1, c2))


def contains(rg, r, c):
    return rg[0] <= r <= rg[2] and rg[1] <= c <= rg[3]


class Selection:
    def __init__(self, ranges=None, active=(0, 0)):
        self.ranges = list(ranges or [(active[0], active[1], active[0], active[1])])
        self.active = tuple(active)
        self.anchor = tuple(active)
        # where Shift+arrow grows from: the far corner of the last range
        self.cursor = tuple(active)
        if not any(contains(rg, *self.active) for rg in self.ranges):
            self.ranges = [(active[0], active[1], active[0], active[1])]

    def copy(self):
        s = Selection(list(self.ranges), self.active)
        s.anchor = self.anchor
        s.cursor = self.cursor
        return s

    @property
    def last(self):
        return self.ranges[-1]

    def is_single_cell(self):
        return len(self.ranges) == 1 and self.ranges[0][0] == self.ranges[0][2] and \
            self.ranges[0][1] == self.ranges[0][3]

    # ------------------------------------------------------------ setting
    def set_cell(self, r, c, merge=None):
        self.active = self.anchor = self.cursor = (r, c)
        self.ranges = [merge or (r, c, r, c)]

    def set_range(self, rg, active=None):
        self.ranges = [norm(*rg)]
        self.active = active or (rg[0], rg[1])
        self.anchor = self.active
        self.cursor = (rg[2], rg[3])

    def add_cell(self, r, c):
        """Ctrl+click: a new area."""
        self.ranges.append((r, c, r, c))
        self.active = self.anchor = self.cursor = (r, c)

    def extend_to(self, r, c):
        """Shift+click / Shift+arrow: the last range spans anchor..(r, c)."""
        ar, ac = self.anchor
        self.ranges[-1] = norm(ar, ac, r, c)
        self.cursor = (r, c)

    def select_columns(self, c1, c2, add=False):
        rg = (0, min(c1, c2), MAX_ROWS - 1, max(c1, c2))
        if add:
            self.ranges.append(rg)
        else:
            self.ranges = [rg]
        self.active = (self._top_row_hint(), c1)
        self.anchor = self.active
        self.cursor = (MAX_ROWS - 1, c2)

    def select_rows(self, r1, r2, add=False):
        rg = (min(r1, r2), 0, max(r1, r2), MAX_COLS - 1)
        if add:
            self.ranges.append(rg)
        else:
            self.ranges = [rg]
        self.active = (r1, self._left_col_hint())
        self.anchor = self.active
        self.cursor = (r2, MAX_COLS - 1)

    def _top_row_hint(self):
        return getattr(self, "top_hint", 0)

    def _left_col_hint(self):
        return getattr(self, "left_hint", 0)

    def select_all(self):
        self.ranges = [(0, 0, MAX_ROWS - 1, MAX_COLS - 1)]
        self.anchor = self.active
        self.cursor = (MAX_ROWS - 1, MAX_COLS - 1)

    # ------------------------------------------------------------ Enter/Tab
    def step_within(self, dr, dc, merges_at=None):
        """Enter/Tab inside a multi-cell selection: move the active cell
        through the selection without changing it (rows first for Tab,
        columns first for Enter), wrapping and moving on to the next area."""
        r, c = self.active
        idx = next((i for i, rg in enumerate(self.ranges) if contains(rg, r, c)), 0)
        rg = self.ranges[idx]
        r1, c1, r2, c2 = rg
        if dr:
            r += dr
            if r > r2:
                r = r1
                c += 1
                if c > c2:
                    idx = (idx + 1) % len(self.ranges)
                    r1, c1, r2, c2 = self.ranges[idx]
                    r, c = r1, c1
            elif r < r1:
                r = r2
                c -= 1
                if c < c1:
                    idx = (idx - 1) % len(self.ranges)
                    r1, c1, r2, c2 = self.ranges[idx]
                    r, c = r2, c2
        else:
            c += dc
            if c > c2:
                c = c1
                r += 1
                if r > r2:
                    idx = (idx + 1) % len(self.ranges)
                    r1, c1, r2, c2 = self.ranges[idx]
                    r, c = r1, c1
            elif c < c1:
                c = c2
                r -= 1
                if r < r1:
                    idx = (idx - 1) % len(self.ranges)
                    r1, c1, r2, c2 = self.ranges[idx]
                    r, c = r2, c2
        self.active = (r, c)
        self.anchor = (r, c)
