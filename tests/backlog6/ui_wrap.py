import os, sys
_exit = os._exit
_name = os.path.basename(os.environ["AUDIT"])[:-3]
def _save_then_exit(code):
    try:
        win.book.save("/tmp/b6/aud/%s.xlsx" % _name)
        for i, w in enumerate(app.windows):
            if w is not win and w.book is not None and w.book is not win.book: w.book.save("/tmp/b6/aud/%s_w%d.xlsx" % (_name, i))
    except BaseException as e:
        print("SAVE-AT-EXIT FAILED", _name, type(e).__name__, str(e)[:200], flush=True)
    _exit(code)
os._exit = _save_then_exit
os.environ["FOLIO_TEST_SCRIPT"] = os.environ["AUDIT"]
exec(compile(open(os.environ["AUDIT"]).read(), os.environ["AUDIT"], "exec"))
