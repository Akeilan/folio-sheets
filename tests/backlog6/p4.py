import sys
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open(sys.argv[2])
b.insert_sheet(1, "Sheet2"); b.insert_sheet(2, "Sheet3"); b.insert_sheet(3, "My Sheet")
for s in range(4): b.set_input(s, 0, 0, str(s + 1)); b.set_input(s, 1, 0, str(10 * (s + 1)))
def v(f, s=0, r=5, c=5):
    try: b.set_input(s, r, c, "=" + f); x = b.value(s, r, c); return tuple(x) if isinstance(x, list) else x
    except BaseException as e: return "EXC " + str(e)[:80]
for f in ["SUM(Sheet1:Sheet3!A1)", "SUM(Sheet1:Sheet3!A1:A2)", "AVERAGE(Sheet2:Sheet3!A1)", "SUM('Sheet1:My Sheet'!A1)", "COUNT(Sheet1:Sheet3!A:A)", "MAX(Sheet1:Sheet3!A2)", "Sheet2!A1+'My Sheet'!A2", "SUM(Sheet2!A:A)", "Sheet9!A1", "SUM(Sheet1!A1:Sheet1!A2)", "A1:A2 A2:A3", "SUM(A1,Sheet2!A1)",
          "IFERROR(VLOOKUP(1,Sheet2!A:A,1,0),\"none\")", "INDIRECT(\"Sheet2!A2\")", "INDIRECT(\"'My Sheet'!A1\")", "SUM(INDIRECT(\"Sheet\"&2&\"!A1:A2\"))", "CELL(\"filename\")", "ROW(Sheet2!A7)", "SHEET(Sheet3!A1)", "SHEET(\"My Sheet\")", "SHEETS(Sheet1:Sheet3!A1)"]:
    print("%-48s %r" % (f, v(f)))
