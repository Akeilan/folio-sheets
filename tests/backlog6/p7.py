import sys
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open(sys.argv[2])
b.set_number_format(0, [(0, 0, 5, 0)], "@")
for r, t in enumerate(["00123", "=1+1", "1/2", "1e5", "TRUE", "50%"]):
    b.set_input(0, r, 0, t); print(repr(t), "->", repr(b.value(0, r, 0)), repr(b.display(0, r, 0)), b.number_format(0, r, 0))
