import sys
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open(sys.argv[2]); S = 0
def v(f):
    b.set_input(S, 7, 7, "=" + f); return b.value(S, 7, 7)
for f in ['TEXT(123456789012,"General")', 'TEXT(0.000012345,"General")', '""&123456789012', '""&0.000012345', 'TEXT(1234567890123456,"General")', '""&0.1', '""&1E-5', '""&123456789.123456789', 'TEXT(0.123,".00")', 'TEXT(5.5,"#.00")', 'TEXT(0.5,"#.##")', 'TEXT(5,"#.##")', 'TEXT(0,"#.##")', 'TEXT(0.5,".0#")']:
    try: print(f, "->", repr(v(f)))
    except BaseException as e: print(f, "-> EXC", str(e)[:80])
# a cell with the format
for txt, code in [("0.123", ".00"), ("123456789012", "General"), ("0.000012345", "General"), ("1234567890123", "General"), ("0.1234567890123", "General"), ("12345678.12345", "General")]:
    b.set_input(S, 30, 0, txt)
    try:
        b.set_number_format(S, [(30, 0, 30, 0)], code); print("cell", txt, code, "->", repr(b.display(S, 30, 0)))
    except BaseException as e: print("cell", txt, code, "-> EXC", str(e)[:80])
b.set_col_width(S, 0, 0, 30)
for txt in ["123456789012", "0.000012345", "1234567890123", "0.1234567890123", "12345678.12345"]:
    b.set_number_format(S, [(30, 0, 30, 0)], "General"); b.set_input(S, 30, 0, txt); print("wide cell", txt, "->", repr(b.display(S, 30, 0)))
