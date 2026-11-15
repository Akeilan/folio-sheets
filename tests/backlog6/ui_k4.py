exec(open("/tmp/b6/uiharn.py").read())
focus(); S = g.sheet
seen = []
win.connect("key-press-event", lambda w, ev: seen.append((Gdk.keyval_name(ev.keyval), int(ev.state))) and False)
_run = win.run
ran = []
win.run = lambda c, *a, **k: (ran.append(c), _run(c, *a, **k))[1]
at(0, 0); DLG.clear(); xdo("key", "minus"); pump(0.2); info("after xdotool key minus", dict(editing=bool(g.editing()), text=g.editor.text() if g.editing() else None, dlg=DLG, ran=ran, keys=seen[-3:]))
if g.editing(): key("Escape")
at(1, 0); seen.clear(); ran.clear(); DLG.clear(); xdo("type", "-5"); pump(0.2); info("after xdotool type -5", dict(editing=bool(g.editing()), text=g.editor.text() if g.editing() else None, dlg=DLG, ran=ran, keys=seen))
if g.editing(): key("Escape")
# a synthetic GDK event, no X involved
at(2, 0); ran.clear(); ev = Gdk.Event.new(Gdk.EventType.KEY_PRESS); ev.keyval = Gdk.KEY_minus; ev.state = 0; ev.window = win.get_window()
h = win.on_key(win, ev) if hasattr(win, "on_key") else None
info("synthetic '-' through win.on_key", dict(handled=h, editing=bool(g.editing()), text=g.editor.text() if g.editing() else None, ran=ran))
info("key handlers on win", [a for a in dir(win) if a.startswith("on_key")])
done()
