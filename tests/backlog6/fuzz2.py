# one function per process, so a hang or a crash is pinned to its call
import sys, subprocess, time, json
if len(sys.argv) > 4:
    sys.path.insert(0, sys.argv[1]); import folio_sheets_core as core
    b = core.Book.open(sys.argv[2]); S = 0
    for i in range(5): b.set_input(S, i, 0, str(i + 1)); b.set_input(S, i, 1, "t%d" % i)
    fn = sys.argv[3]; args = json.loads(sys.argv[4])
    for a in args:
        f = "=%s(%s)" % (fn, a); print("TRY " + f, flush=True); t0 = time.time()
        try: b.set_input(S, 9, 9, f); b.value(S, 9, 9); b.display(S, 9, 9)
        except Exception: pass
        except BaseException as e: print("PANIC %s | %s" % (f, str(e)[:90]), flush=True)
        dt = time.time() - t0
        if dt > 1.0: print("SLOW %.1f %s" % (dt, f), flush=True)
    sys.exit(0)
sys.path.insert(0, sys.argv[1]); import folio_sheets_core as core
args = ["", "1", "-1", "0", "1e308", "-1e308", "1e-320", "\"\"", "\"a\"", "TRUE", "A1:A5", "B1:B5", "{1,2;3,4}", "#N/A", "1/0", "A1:A5,", ",", "1,2", "1,2,3", "1,2,3,4", "1,2,3,4,5", "A1:A5,A1:A5", "A1:A5,B1:B5", "\"a\",\"b\"", "A1:A5,\">2\"", "-1,-1", "0,0", "1e9,1e9", "A:A", "1:1048576", "A1:XFD1", "SEQUENCE(1000)", "9.99e307,9.99e307", "\"x\",0", "0,\"x\"", "2.5,2.5", "-0.5,-0.5", "1,-1", "-1,1", "1,0", "0,1", "1,1,1", "0,0,0", "-1,-1,-1", "1e15", "45351", "-45351", "2958466", "0.5", "\"1/1/2024\"", "\"abc\",2,3", "\"abc\",-1", "\"abc\",0", "\"abc\",1e10", "\"\",1,1", "A1,A1,A1,A1,A1,A1", "1e9", "1e9,2", "2,1e9", "1e9,1e9,1e9", "1e6,1e6", "170,170", "1e5", "1,1e9,1", "0.5,1e9", "1e9,0.5,0.5", "5,1e9,0.5,TRUE"]
for fn in core.function_names():
    try:
        r = subprocess.run([sys.executable, sys.argv[0], sys.argv[1], sys.argv[2], fn, json.dumps(args)], capture_output=True, text=True, timeout=25)
        out = r.stdout.splitlines()
        for l in out:
            if l.startswith(("PANIC", "SLOW")): print(l, flush=True)
        if r.returncode != 0: print("CRASH rc=%d in %s after %s | %s" % (r.returncode, fn, out[-1] if out else "", r.stderr.strip().splitlines()[-1][:120] if r.stderr.strip() else ""), flush=True)
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or b"").decode().splitlines() if isinstance(e.stdout, bytes) else (e.stdout or "").splitlines()
        print("HANG  >25 s in %s" % (out[-1] if out else fn), flush=True)
print("done")
