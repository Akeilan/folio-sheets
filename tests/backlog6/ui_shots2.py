exec(open("/tmp/b6/uiharn.py").read())
import subprocess
focus(); g.set_sheet(3); win.sheet_changed(); g.select_cell(20, 22); pump(0.6); subprocess.run(["import", "-window", "root", "/tmp/b6/shot_c2.png"])
g.set_sheet(0); win.sheet_changed(); win.run("zoom:200") if False else None
st = g.state(); st.zoom = 2.0; g.select_cell(0, 0); g.refresh(); pump(0.6); subprocess.run(["import", "-window", "root", "/tmp/b6/shot_z.png"])
done()
