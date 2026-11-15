# Backlog 6 probe: nothing a user can type may panic the engine.
import sys, random, itertools, time, functools
print = functools.partial(print, flush=True)
sys.path.insert(0, sys.argv[1])
import folio_sheets_core as core
b = core.Book.open(sys.argv[2]); S = 0
for i in range(5): b.set_input(S, i, 0, str(i + 1)); b.set_input(S, i, 1, "t%d" % i)
random.seed(6)
seen = {}
def tryit(kind, fn, arg):
    try: fn(arg)
    except Exception: pass
    except BaseException as e:
        k = (kind, str(e)[:70])
        if k not in seen: seen[k] = arg
# 1. number format codes
codes = [".00", ".", "#.", ".#", "0.", ",", "#,", ",#", "0,0", "%", "E+", "0E+0", "0.E+0", ".0E+0", "0.0E", "E+0", "0E", "[", "]", "[]", "[Red", "[h", "[h]", "[hh]:", "[$", "[$-", "[$-409", "[$€", "[>", "[>]", "[>1", "[=0]", "[<>5]0", "\"", "\"abc", "\\", "0\\", "_", "0_", "*", "0*", "@", "@@", "?/", "/?", "?/?", "#/", "0/0", "# /", "# ?/", " ?/?", "?/0", "0/", "/", "//", "# ?/???????????", ";", ";;", ";;;", ";;;;", ";;;;;", "0;", ";0", "0;0;0;0;0", "General;", "General;General", "Gen", "general", "GENERAL", "d", "m", "y", "h", "s", "dd", "ddddd", "mmmmmm", "yyy", "yyyyy", "hhh", "sss", "ss.", "ss.0", "ss.000000", ".0", "s.00", "h:mm:ss.0000", "AM/PM", "A/P", "am/pm", "AM/P", "h AM", "[s].00", "[m]:ss", "[hmm]", "m/d/", "/d/", "e", "ee", "g", "gg", "ggg", "b", "bb", "bbbb", "0 0", "0  ", " ", "", "#", "##", "?", "??", "0?#", "#0#", "0.0.0", "0..0", "0,,", ",,0", "0,,,,,,,,,,", "#,##0,,,,,,,,", "0%%", "%0", "0.00%%%%%%", "000000000000000000000000000000", "0." + "0" * 40, "#" * 300, "0" * 300, "[Color1]0", "[Color56]0", "[Color57]0", "[Color0]0", "[Colour]", "[Black][Red]0", "[$-F800]", "[$-F400]", "[$-x-sysdate]", "[$-x-systime]h:mm", "[DBNum1]0", "[ENG]", "0\"", "\"\"", "\"a\"\"b\"", "0;[Red]", "[Red];[Blue];[Green];[Yellow]", "[>100]0;[<0]0", "[>100][<50]0", "0.0E+00E+00", "0E+0E", "E", "e+", "0e-", "0.#E+#", "##0.0E+0", "#E+0", "?E+?", "0.0 E+0", "@\"x\"@", "0@", "@0", "*", "**", "* ", "*0", "0*-", "_(", "_", "__", "0_)_)", "\\\\", "\\", "\\0", "h\\h", "[h]\\:mm", "T", "0T", "yyyy-mm-ddThh:mm", "mm:ss", "mm", "m:ss", "[mm]:ss.00", "h:m:s", "hmmss", "dmy", "ymd", "ddd, mmm", "aaa", "aaaa", "[$-411]ggge", "0 %", "0 %", "0‰", "₹0", "0.0​", "\u0000", "0\n0", "\t", "😀0", "0😀", "[$😀]0", "[>1e999]0", "[>-]0", "[>.]0", "[<=]", "[<>]", "[==1]0", "[>1][Red]0", "[Red][>1]0;0"]
alphabet = list("0#?.,%E+-/;[]\"\\_*@ dmyhsAMP$:()ge") + ["[Red]", "[h]", "[$-409]", "General", "E+0", "0.0", "AM/PM", "\"x\""]
for _ in range(6000): codes.append("".join(random.choice(alphabet) for _ in range(random.randint(1, 9))))
vals = [0, -0.0, 1, -1, 0.5, -0.5, 0.123, 1234.567, -1234.567, 1e-10, 1e15, 1e100, -1e100, 1e308, 5e-324, 45351.75, 60, 0.999999, 2958465.9999, 2958466, -45351, 1e20, 0.05, 99.995, 9.9999999, 123456789.123456789, float("inf"), float("nan")]
for c in codes:
    for v in vals: tryit("format", lambda a: core.format_number(a[0], a[1]), (v, c))
    tryit("format-text", lambda a: (b.set_input(S, 9, 9, "x"), b.set_number_format(S, [(9, 9, 9, 9)], a), b.display(S, 9, 9)), c)
print("number formats: %d codes x %d values" % (len(codes), len(vals)))
# 2. formulas
pieces = ["1", "2.5", "\"a\"", "TRUE", "A1", "$A$1", "A1:A5", "A:A", "1:1", "B2", "Sheet1!A1", "'Sheet1'!A1", "{1,2;3,4}", "{1,2", "#REF!", "#N/A", "#DIV/0!", "+", "-", "*", "/", "^", "&", "=", "<>", "<=", ">=", "<", ">", "%", "(", ")", ",", ";", ":", " ", "!", "@", "#", "$", "'", "\"", "{", "}", "[", "]", ".", "..", "1E", "1E+", "E5", "1.2.3", "SUM(", "IF(", "LAMBDA(", "LET(", "INDEX(", "OFFSET(", "INDIRECT(", "TEXT(", "SEQUENCE(", "x", "_x", "A", "A0", "XFD1048576", "XFE1", "A1048577", "R1C1", "RC", "R[1]C[-1]", "Table1[", "[@x]", "Table1[[#All],[a]]", "A1#", "@A1", "--", "++", "1 2", "A1 A2", "A1:A2:A3", "A1::A2", "A1:", ":A1", "$", "$A", "A$", "$$A1", "\n", "\t", " ", "é", "😀", "\x00", "0x1", "1e999", "1e-999", "99999999999999999999999999", "0.0000000000000000000000001", ".", "1.", ".5", "1..2", "1,2", "SUM(A1:A5)", "TRUE()", "PI()", "NA()", "RAND()", "NOW()"]
fs = []
for _ in range(60000): fs.append("".join(random.choice(pieces) for _ in range(random.randint(1, 8))))
for a, c2 in itertools.product(pieces, repeat=2): fs.append(a + c2)
for f in fs:
    tryit("check_formula", core.check_formula, f); tryit("ref_spans", core.ref_spans, f); tryit("set_input", lambda a: b.set_input(S, 9, 9, "=" + a), f); tryit("shift_formula", lambda a: b.shift_formula(a, -5, -5), f)
print("formula strings:", len(fs))
# 3. functions with odd arguments
args = ["", "1", "-1", "0", "1e308", "-1e308", "1e-320", "\"\"", "\"a\"", "TRUE", "A1:A5", "B1:B5", "{1,2;3,4}", "{}", "#N/A", "1/0", "A1:A5,", ",", "1,2", "1,2,3", "1,2,3,4", "1,2,3,4,5", "A1:A5,A1:A5", "A1:A5,B1:B5", "\"a\",\"b\"", "A1:A5,\">2\"", "-1,-1", "0,0", "1e9,1e9", "A:A", "1:1048576", "A1:XFD1", "SEQUENCE(1000)", "SEQUENCE(1,1000)", "9.99e307,9.99e307", "\"x\",0", "0,\"x\"", "2.5,2.5", "-0.5,-0.5", "1,-1", "-1,1", "1,0", "0,1", "1,1,1", "0,0,0", "-1,-1,-1", "1e15", "45351", "-45351", "2958466", "0.5", "\"1/1/2024\"", "\"abc\",2,3", "\"abc\",-1", "\"abc\",0", "\"abc\",1e10", "\"\",1,1", "A1,A1,A1,A1,A1,A1"]
names = core.function_names(); cnt = 0
slow = []
for fn in names:
    for a in args:
        cnt += 1; t0 = time.time(); tryit("fn", lambda x: (b.set_input(S, 9, 9, x), b.value(S, 9, 9), b.display(S, 9, 9)), "=%s(%s)" % (fn, a)); dt = time.time() - t0
        if dt > 1.5: slow.append((round(dt, 1), "=%s(%s)" % (fn, a))); print("SLOW", round(dt, 1), "=%s(%s)" % (fn, a))
b.set_input(S, 9, 9, "")
print("function calls:", cnt)
# 4. typed input
ins = ["", " ", "'", "''", "=", "==", "+", "-", "+-", "=+", "=-", "1e", "1e+", "e5", "1/", "/1", "1/2/", "1//2", "1/2/3/4", "00/00/0000", "99/99/9999", "1/1/0", "0/0/0", "12/31/99999", ":", "1:", ":1", "1:1:1:1", "99:99:99", "24:00", "-1:00", "1:60", "1:00 XM", "AM", "1 AM PM", "%", "%%", "1%%", "%1", "$", "$$", "$1$", "1$", "(", ")", "()", "(1", "1)", "((1))", "-(1)", ",", "1,", ",1", "1,,2", ".", "..", "-.", "+.", "1.2.3", "１", "٣", "1 2", "1e9999", "-1e9999", "9" * 400, "0." + "0" * 400 + "1", "#", "#N/A N/A", "#REF", "TRUE FALSE", "1 1/0", "1 0/1", "0 0/0", "1 1/", "1 /2", "- 1", "1 -", "1e-", "0x", "nan", "inf", "-inf", "NaN", "Infinity", "\x00", "\n", "a\x00b", "=" * 100, "'" * 100, "=A1" + "+A1" * 3000, "=" + "(" * 300 + "1" + ")" * 300, "=" + "(" * 5000, "=" + "SUM(" * 400 + "1" + ")" * 400, "=" + "-" * 5000 + "1", "=" + "{" * 100, "=1" + "%" * 3000, "=\"" + "a" * 40000 + "\"", "=A1:A5 " * 50]
for s in ins: tryit("input", lambda a: (b.set_input(S, 9, 9, a), b.value(S, 9, 9), b.display(S, 9, 9), b.input(S, 9, 9)), s)
print("typed inputs:", len(ins))
print("PANICS: %d distinct" % len(seen))
for (kind, msg), arg in sorted(seen.items()): print("  ", kind, "|", msg, "| e.g.", repr(arg)[:110])
