import sys, subprocess, time
CASES = {
 "deep brackets 300": "=" + "(" * 300 + "1" + ")" * 300, "deep brackets 5000": "=" + "(" * 5000 + "1" + ")" * 5000, "unclosed brackets 5000": "=" + "(" * 5000,
 "nested SUM 400": "=" + "SUM(" * 400 + "1" + ")" * 400, "nested SUM 5000": "=" + "SUM(" * 5000 + "1" + ")" * 5000, "5000 unary minus": "=" + "-" * 5000 + "1", "3000 percent": "=1" + "%" * 3000,
 "3000 additions": "=A1" + "+A1" * 3000, "30000 additions": "=A1" + "+A1" * 30000, "nested IF 200": "=" + "IF(1," * 200 + "1" + ",0)" * 200, "nested braces": "=" + "{" * 100, "40k string": "=\"" + "a" * 40000 + "\"",
 "40k text": "a" * 40000, "400 digits": "9" * 400, "tiny": "0." + "0" * 400 + "1", "long concat": "=" + "&".join(["\"x\""] * 5000), "power tower": "=" + "^".join(["2"] * 3000), "many args": "=SUM(" + ",".join(["1"] * 20000) + ")",
 "LET chain": "=LET(" + "".join("x%d,%s," % (i, "x%d+1" % (i - 1) if i else "1") for i in range(500)) + "x499)", "lambda recursion": "=LET(f,LAMBDA(f,n,IF(n=0,0,f(f,n-1)+1)),f(f,100000))", "lambda recursion 500": "=LET(f,LAMBDA(f,n,IF(n=0,0,f(f,n-1)+1)),f(f,500))",
 "REPT bomb": "=REPT(REPT(\"x\",32767),32767)", "REPT len": "=LEN(REPT(\"ab\",16383))", "SEQUENCE huge": "=SUM(SEQUENCE(1048576,100))", "SEQUENCE 1e9": "=SEQUENCE(1e9)", "RANDARRAY huge": "=SUM(RANDARRAY(5000,5000))", "MMULT 500": "=SUM(MMULT(SEQUENCE(500,500),SEQUENCE(500,500)))",
 "MINVERSE 300": "=SUM(MINVERSE(RANDARRAY(300,300)))", "MAKEARRAY big": "=SUM(MAKEARRAY(2000,2000,LAMBDA(r,c,r*c)))", "REDUCE 1e6": "=REDUCE(0,SEQUENCE(1000000),LAMBDA(a,b,a+b))", "TEXTJOIN whole column": "=LEN(TEXTJOIN(\",\",FALSE,A:A))", "whole sheet sum": "=SUM(1:1048576)",
 "COUNTIF whole sheet": "=COUNTIF(A:XFD,1)", "SUMPRODUCT whole cols": "=SUMPRODUCT(A:A,B:B)", "array whole column": "=SUM(A:A*2)", "FACT 1e308": "=FACT(1e308)", "COMBIN big": "=COMBIN(1e9,5e8)", "PERMUT big": "=PERMUT(1e9,1e9)", "BINOM big": "=BINOM.DIST(5e8,1e9,0.5,TRUE)",
 "POISSON big": "=POISSON.DIST(1e9,1e9,TRUE)", "HYPGEOM big": "=HYPGEOM.DIST(1e6,2e6,3e6,1e7,TRUE)", "CRITBINOM big": "=BINOM.INV(1e9,0.5,0.5)", "NEGBINOM big": "=NEGBINOM.DIST(1e9,1e9,0.5,TRUE)", "GCD big": "=GCD(1e15,7)", "LCM big": "=LCM(1e15,999999999999989)", "BASE big": "=BASE(1e15,2,255)",
 "ROMAN": "=ROMAN(3999,4)", "WORKDAY big": "=WORKDAY(1,1e9)", "NETWORKDAYS big": "=NETWORKDAYS(1,2958465)", "EDATE big": "=EDATE(1,1e9)", "DATE big": "=DATE(9999,1e9,1e9)", "IRR hard": "=IRR({-1,1e300,-1e300})", "RATE hard": "=RATE(1e9,-1,1e9)", "XIRR hard": "=XIRR({-1,2},{1,2958465})",
 "self ref": "=J14", "INDIRECT self": "=INDIRECT(\"J14\")", "OFFSET self": "=OFFSET(J14,0,0)", "spill into itself": "=J14:J20", "SUBSTITUTE bomb": "=LEN(SUBSTITUTE(REPT(\"a\",32767),\"a\",REPT(\"b\",32767)))", "TEXT long": "=TEXT(1," + "\"" + "0" * 30000 + "\")", "date text": "=DATEVALUE(REPT(\"1/\",10000))",
 "CONVERT": "=CONVERT(1e308,\"km\",\"ang\")", "DEC2BIN": "=DEC2BIN(1e300)", "BITLSHIFT": "=BITLSHIFT(1,1e9)", "nul": "a\x00b", "newlines": "\n\n\n", "odd unicode": "=\"😀\"&UNICHAR(55357)" if False else "=UNICHAR(55357)", "UNICHAR big": "=UNICHAR(1114112)", "CHAR 0": "=CHAR(0)",
}
if len(sys.argv) > 3:
    sys.path.insert(0, sys.argv[1]); import folio_sheets_core as core
    b = core.Book.open(sys.argv[2]); S = 0
    for i in range(5): b.set_input(S, i, 0, str(i + 1)); b.set_input(S, i, 1, str(i * 2))
    t = CASES[sys.argv[3]]; t0 = time.time()
    try: b.set_input(S, 13, 9, t); v = b.value(S, 13, 9); d = b.display(S, 13, 9); r = repr(v)[:60]
    except Exception as e: r = "refused: " + str(e)[:60]
    except BaseException as e: r = "PANIC: " + str(e)[:90]
    print("%.2f s  %s" % (time.time() - t0, r)); sys.exit(0)
for name in CASES:
    try:
        r = subprocess.run([sys.executable, sys.argv[0], sys.argv[1], sys.argv[2], name], capture_output=True, text=True, timeout=30)
        out = r.stdout.strip().splitlines()
        tag = "CRASH rc=%d %s" % (r.returncode, (r.stderr.strip().splitlines() or [""])[-1][:100]) if r.returncode != 0 else (out[-1] if out else "?")
        flag = "!!" if r.returncode != 0 or "PANIC" in tag or (out and float(out[-1].split()[0]) > 2.0) else "  "
        print("%s %-24s %s" % (flag, name, tag), flush=True)
    except subprocess.TimeoutExpired: print("!! %-24s HANG > 30 s" % name, flush=True)
