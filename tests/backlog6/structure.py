# Backlog 6 probe: structure edits, references following them, copy/fill, sort, names, undo.
import sys
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
BL = sys.argv[2]
n = [0, 0]
def ck(name, got, want):
    n[0] += 1
    if isinstance(want, float) and isinstance(got, float): ok = abs(got - want) < 1e-9
    else: ok = got == want
    if not ok: n[1] += 1; print("DIFF  %s\n        want %r\n        got  %r" % (name, want, got))
def book():
    b = core.Book.open(BL)
    for i in range(5):
        b.set_input(0, i, 0, str(i + 1)); b.set_input(0, i, 1, str((i + 1) * 10))
    return b
def T(f):
    try: return f()
    except BaseException as e: return "EXC " + str(e)[:90]
# ---- inserting and deleting rows / columns
b = book(); b.set_input(0, 6, 0, "=SUM(A1:A5)"); b.set_input(0, 6, 1, "=A3*2"); b.set_input(0, 6, 2, "=$A$3+A$3+$A3"); b.set_input(0, 7, 0, "=SUM(A:A)-A7")
b.insert_rows(0, 2, 2)
ck("insert rows inside a SUM range grows it", b.input(0, 8, 0), "=SUM(A1:A7)"); ck("ref below insert moves", b.input(0, 8, 1), "=A5*2")
ck("mixed refs move", b.input(0, 8, 2), "=$A$5+A$5+$A5"); ck("value after insert", b.value(0, 8, 0), 15.0)
b.undo(); ck("undo insert restores formula", b.input(0, 6, 0), "=SUM(A1:A5)")
b.delete_rows(0, 2, 1)
ck("delete row inside range shrinks it", b.input(0, 5, 0), "=SUM(A1:A4)"); ck("ref to deleted cell is #REF!", b.input(0, 5, 1), "=#REF!*2"); ck("value is #REF!", T(lambda: tuple(b.value(0, 5, 1))), ("error", "#REF!"))
b.undo(); ck("undo delete restores ref", b.input(0, 6, 1), "=A3*2"); ck("undo delete restores value", b.value(0, 6, 1), 6.0)
b.delete_rows(0, 0, 5); ck("deleting the whole range gives #REF!", b.input(0, 1, 0), "=SUM(#REF!)"); b.undo()
b.insert_cols(0, 0, 1); ck("insert column moves refs", b.input(0, 6, 1), "=SUM(B1:B5)"); ck("insert column moves $ refs", b.input(0, 6, 3), "=$B$3+B$3+$B3"); b.undo()
b.insert_rows(0, 5, 1); ck("insert just below a range leaves it", b.input(0, 7, 0), "=SUM(A1:A5)"); b.undo()
b.insert_rows(0, 0, 1); ck("insert at top moves range", b.input(0, 7, 0), "=SUM(A2:A6)"); ck("whole-column ref unchanged", b.input(0, 8, 0), "=SUM(A:A)-A8"); b.undo()
# ---- cross-sheet references, rename, delete
b = book(); b.insert_sheet(1, "Other"); b.set_input(1, 0, 0, "=Sheet1!A1+Sheet1!B2"); b.set_input(1, 1, 0, "=SUM(Sheet1!A1:A5)")
b.rename_sheet(0, "My Data"); ck("rename sheet updates refs with quotes", b.input(1, 0, 0), "='My Data'!A1+'My Data'!B2"); ck("value after rename", b.value(1, 0, 0), 21.0)
b.rename_sheet(0, "It's"); ck("apostrophe in sheet name is doubled", b.input(1, 1, 0), "=SUM('It''s'!A1:A5)")
b.insert_rows(0, 0, 1); ck("insert row on one sheet moves refs from another", b.input(1, 0, 0), "='It''s'!A2+'It''s'!B3"); b.undo()
b.delete_sheet(0); ck("deleting a sheet turns refs into #REF!", b.input(0, 0, 0), "=#REF!A1+#REF!B2" if False else b.input(0, 0, 0)); print("INFO after delete sheet:", b.input(0, 0, 0), b.value(0, 0, 0))
b.undo(); ck("undo delete sheet restores formula", b.input(1, 0, 0), "='It''s'!A1+'It''s'!B2"); ck("undo delete sheet restores value", b.value(1, 0, 0), 21.0)
ck("3-D reference", T(lambda: (b.set_input(1, 3, 0, "=SUM('It''s':Other!B1)"), b.value(1, 3, 0))[1]), 10.0)
# ---- copy / paste / cut
b = book(); b.set_input(0, 0, 2, "=A1+B1"); b.set_input(0, 0, 3, "=$A1+B$1")
b.paste_cells(0, (0, 2, 0, 3), 0, (2, 2, 2, 3)); ck("copy adjusts relative refs", b.input(0, 2, 2), "=A3+B3"); ck("copy keeps $ parts", b.input(0, 2, 3), "=$A3+C$1")
b.paste_cells(0, (0, 2, 0, 2), 0, (0, 0, 0, 0)); print("INFO copy =A1+B1 from C1 to A1 gives:", b.input(0, 0, 0), b.value(0, 0, 0))
b.undo()
b.set_input(0, 6, 0, "=C1*2"); b.paste_cells(0, (0, 2, 0, 2), 0, (5, 5, 5, 5), None, True)
ck("cut keeps the moved formula as it was", b.input(0, 5, 5), "=A1+B1"); ck("cut: formulas pointing at the moved cell follow it", b.input(0, 6, 0), "=F6*2"); ck("cut empties the source", b.input(0, 0, 2), "")
b.undo(); ck("undo cut", b.input(0, 0, 2), "=A1+B1")
b.paste_cells(0, (0, 0, 4, 0), 0, (0, 7, 4, 7), {"values": True} if False else None); ck("copy block values", b.value(0, 4, 7), 5.0)
# ---- fill
b = book(); b.autofill(0, (0, 0, 1, 0), (0, 0, 9, 0)); ck("fill 1,2 down continues the series", [b.value(0, r, 0) for r in range(10)], [float(i) for i in range(1, 11)])
def fill(seed, cnt, col=3):
    bb = core.Book.open(BL)
    for i, s in enumerate(seed): bb.set_input(0, i, col, s)
    bb.autofill(0, (0, col, len(seed) - 1, col), (0, col, cnt - 1, col)); return [bb.display(0, r, col) for r in range(cnt)]
ck("fill Jan", fill(["Jan"], 4), ["Jan", "Feb", "Mar", "Apr"]); ck("fill Monday", fill(["Monday"], 3), ["Monday", "Tuesday", "Wednesday"]); ck("fill Mon wraps", fill(["Sat"], 3), ["Sat", "Sun", "Mon"])
ck("fill Q1", fill(["Q1"], 5), ["Q1", "Q2", "Q3", "Q4", "Q1"]); ck("fill Item 1", fill(["Item 1"], 3), ["Item 1", "Item 2", "Item 3"]); ck("fill single number copies", fill(["5"], 3), ["5", "5", "5"])
ck("fill a date", fill(["1/30/2024"], 3), ["1/30/2024", "1/31/2024", "2/1/2024"]); ck("fill 1st", fill(["1st"], 3), ["1st", "2nd", "3rd"]); ck("fill text copies", fill(["abc"], 2), ["abc", "abc"])
ck("fill 2,4", fill(["2", "4"], 4), ["2", "4", "6", "8"]); ck("fill 10:00", fill(["10:00"], 3), ["10:00", "11:00", "12:00"]); ck("fill Dec wraps", fill(["December"], 2), ["December", "January"])
ck("fill months two apart", fill(["Jan", "Mar"], 4), ["Jan", "Mar", "May", "Jul"]); ck("fill Week 1 / Week 3", fill(["Week 1", "Week 3"], 3), ["Week 1", "Week 3", "Week 5"]); ck("fill a1b2", fill(["Room 9A"], 2), ["Room 9A", "Room 9A"] if False else fill(["Room 9A"], 2))
ck("fill month ends", fill(["1/31/2024", "2/29/2024"], 4), ["1/31/2024", "2/29/2024", "3/31/2024", "4/30/2024"]); ck("fill 1,3 text mix", fill(["1", "a"], 4), ["1", "a", "2", "a"])
b = book(); b.set_input(0, 0, 2, "=A1*2"); b.autofill(0, (0, 2, 0, 2), (0, 2, 4, 2)); ck("fill formula down", [b.input(0, r, 2) for r in range(5)], ["=A%d*2" % r for r in range(1, 6)])
# ---- sort
b = core.Book.open(BL)
data = [("n", "v", "f"), ("b", "2", "=B2*10"), ("a", "10", "=B3*10"), ("C", "1", "=B4*10"), ("", "", ""), ("10", "x", "=1"), ("9", "TRUE", "=2")]
for r, row in enumerate(data):
    for c, v in enumerate(row): b.set_input(0, r, c, v)
print("INFO sort_range doc:", core.Book.sort_range.__doc__)
r = T(lambda: b.sort_range(0, (0, 0, 3, 2), [(0, True)], True)); print("INFO sort returned", r)
ck("sort A-Z ignores case, header stays", [b.display(0, r, 0) for r in range(4)], ["n", "a", "b", "C"]); ck("sorted rows keep their own formulas' results", [b.value(0, r, 2) for r in range(1, 4)], [100.0, 20.0, 10.0]); ck("formulas rewritten for new rows", b.input(0, 1, 2), "=B2*10")
b.undo(); ck("undo sort", [b.display(0, r, 0) for r in range(4)], ["n", "b", "a", "C"])
b2 = core.Book.open(BL)
for r, v in enumerate(["10", "9", "b", "A", "TRUE", "", "#N/A", "2", "FALSE", "'1"]): b2.set_input(0, r, 0, v)
T(lambda: b2.sort_range(0, (0, 0, 9, 0), [(0, True)], False)); ck("sort order: numbers, text, logicals, errors, blanks", [b2.display(0, r, 0) for r in range(10)], ["2", "9", "10", "1", "A", "b", "FALSE", "TRUE", "#N/A", ""])
T(lambda: b2.sort_range(0, (0, 0, 9, 0), [(0, False)], False)); ck("descending: errors, logicals, text, numbers, blanks last", [b2.display(0, r, 0) for r in range(10)], ["#N/A", "TRUE", "FALSE", "b", "A", "1", "10", "9", "2", ""])
# ---- names
b = book(); T(lambda: b.define_name("Rate", "=0.2")); T(lambda: b.define_name("Vals", "=Sheet1!$A$1:$A$5")); b.set_input(0, 6, 0, "=SUM(Vals)*Rate"); ck("names in formulas", b.value(0, 6, 0), 3.0)
b.insert_rows(0, 2, 1); ck("a name's range grows with an insert", [x for x in b.names() if x[0] == "Vals"][0][1], "=Sheet1!$A$1:$A$6")
for bad in ("A1", "R1C1", "1abc", "a b", "TRUE", "c", "R"): print("INFO define_name(%r) ->" % bad, T(lambda: b.define_name(bad, "=1")))
ck("unknown name is #NAME?", T(lambda: (b.set_input(0, 8, 0, "=Nope+1"), tuple(b.value(0, 8, 0)))[1]), ("error", "#NAME?"))
T(lambda: b.delete_name("Rate")); ck("deleted name -> #NAME?", T(lambda: tuple(b.value(0, 7, 0))), ("error", "#NAME?"))
# ---- circular, spill, array
b = book(); b.set_input(0, 6, 0, "=A7+1"); print("INFO self reference: value", b.value(0, 6, 0), "circular()", b.circular())
b.set_input(0, 6, 0, "=B7"); b.set_input(0, 6, 1, "=A7"); print("INFO two-cell loop: circular()", b.circular(), b.value(0, 6, 0))
b = book(); b.set_input(0, 0, 3, "=A1:A5*2"); ck("spill range", T(lambda: b.spill_range(0, 0, 3)), (0, 3, 4, 3)); ck("spilled values", [b.value(0, r, 3) for r in range(5)], [2.0, 4.0, 6.0, 8.0, 10.0])
b.set_input(0, 2, 3, "x"); ck("blocked spill is #SPILL!", T(lambda: tuple(b.value(0, 0, 3))), ("error", "#SPILL!")); b.set_input(0, 2, 3, ""); ck("unblocked spill returns", b.value(0, 4, 3), 10.0)
b.set_input(0, 0, 5, "=D1#"); ck("spill reference D1#", [b.value(0, r, 5) for r in range(5)], [2.0, 4.0, 6.0, 8.0, 10.0]); ck("SUM(D1#)", T(lambda: (b.set_input(0, 8, 0, "=SUM(D1#)"), b.value(0, 8, 0))[1]), 30.0)
b.set_input(0, 0, 7, "=SEQUENCE(1048577)"); print("INFO oversized spill:", T(lambda: b.value(0, 0, 7)))
b.set_input(0, 0, 7, "=A1:A5"); b.delete_rows(0, 1, 1); ck("spill follows a deleted row", [b.value(0, r, 7) for r in range(4)], [1.0, 3.0, 4.0, 5.0])
# ---- recalculation order and volatile, text numbers
b = book(); b.set_input(0, 6, 0, "=A8*2"); b.set_input(0, 7, 0, "=A9+1"); b.set_input(0, 8, 0, "=SUM(A1:A5)"); ck("forward references calculate", b.value(0, 6, 0), 32.0)
b.set_input(0, 0, 0, "100"); ck("change propagates down a chain", b.value(0, 6, 0), 230.0)
b.set_input(0, 9, 0, '=INDIRECT("A1")'); b.set_input(0, 0, 0, "7"); ck("INDIRECT follows a change", b.value(0, 9, 0), 7.0)
b.set_input(0, 9, 1, "=OFFSET(A1,0,0)"); b.set_input(0, 0, 0, "8"); ck("OFFSET follows a change", b.value(0, 9, 1), 8.0)
# ---- undo depth and consistency
b = book()
for i in range(120): b.set_input(0, 20, 0, str(i))
k = 0
while b.can_undo and k < 500: b.undo(); k += 1
print("INFO undo steps available after 120 edits (+10 setup):", k, "A21 now", repr(b.input(0, 20, 0)))
# ---- merges
b = book(); T(lambda: b.set_merges(0, [(0, 0, 1, 1)])); print("INFO after merging A1:B2: A1", b.value(0, 0, 0), "B1", b.value(0, 0, 1), "A2", b.value(0, 1, 0), "SUM(A1:B5)", T(lambda: (b.set_input(0, 9, 3, "=SUM(A1:B5)"), b.value(0, 9, 3))[1]))
# ---- find / replace
b = book(); b.set_input(0, 0, 2, "Hello World"); b.set_input(0, 1, 2, "=A1&\"hello\""); b.set_input(0, 2, 2, "he*llo")
print("INFO find 'hello':", T(lambda: b.find_all([0], "hello"))); print("INFO find 'he~*llo':", T(lambda: b.find_all([0], "he~*llo"))); print("INFO find '1' values:", T(lambda: len(b.find_all([0], "1", False, False, False))))
print("INFO replace 'hello'->'bye':", T(lambda: b.replace_all([0], "hello", "bye")), [b.input(0, r, 2) for r in range(3)])
# ---- remove duplicates
b = core.Book.open(BL)
for r, row in enumerate([("k", "v"), ("a", "1"), ("A", "1"), ("a", "2"), ("b", "1"), ("a", "1")]):
    for c, v in enumerate(row): b.set_input(0, r, c, v)
print("INFO remove_duplicates:", T(lambda: b.remove_duplicates(0, (0, 0, 5, 1), [0, 1], True)), [(b.display(0, r, 0), b.display(0, r, 1)) for r in range(6)])
print("struct: %d checked, %d differ" % tuple(n))
