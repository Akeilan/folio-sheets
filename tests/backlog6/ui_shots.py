exec(open("/tmp/b6/uiharn.py").read())
import subprocess
focus()
for i in range(b.sheet_count):
    try:
        b.set_sheet_state(i, "visible")
    except Exception as e: info("unhide", str(e))
win.tabs.rebuild() if hasattr(win.tabs, "rebuild") else None
for i in range(b.sheet_count):
    g.set_sheet(i); win.sheet_changed(); pump(0.6); subprocess.run(["import", "-window", "root", "/tmp/b6/shot_%d.png" % i])
done()
