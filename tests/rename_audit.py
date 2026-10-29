# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): X3-10, the rename from Folio Sheets — the window and
# its title say Folio Sheet, settings made under the old name carry over, the old folder stays.
import json
import os
import sys
import tempfile
import time
import traceback

from gi.repository import Gtk

checks = []
ERRORS = []
_hook = sys.excepthook
sys.excepthook = lambda t, v, tb: (ERRORS.append("".join(traceback.format_exception(t, v, tb))), _hook(t, v, tb))


def check(name, cond, detail=""):
    checks.append((name, bool(cond)))
    print(("PASS " if cond else "FAIL ") + name + ("" if cond else "  " + repr(detail)), flush=True)


def pump(t=0.05):
    end = time.time() + t
    while time.time() < end:
        while Gtk.events_pending():
            Gtk.main_iteration_do(False)
        time.sleep(0.005)


try:
    import folio_sheets as FS
    check("the app is Folio Sheet", FS.APP_NAME == "Folio Sheet" and FS.APP_ID == "org.folio.Sheet", (FS.APP_NAME, FS.APP_ID))
    pump(0.3)
    check("window title says Folio Sheet", win.get_title().endswith(" - Folio Sheet"), win.get_title())
    check("icon is folio-sheet", win.get_icon_name() == "folio-sheet", win.get_icon_name())
    # settings from before the rename come along
    base = tempfile.mkdtemp(prefix="fs-rename-")
    old = os.path.join(base, "folio-sheets")
    os.makedirs(os.path.join(old, "themes"))
    with open(os.path.join(old, "settings.json"), "w") as f:
        json.dump({"recent": ["/tmp/budget.xlsx"]}, f)
    with open(os.path.join(old, "custom.dic"), "w") as f:
        f.write("Folioism\n")
    keep = os.environ.get("XDG_CONFIG_HOME")
    os.environ["XDG_CONFIG_HOME"] = base
    try:
        d = FS.config_dir()
        check("settings folder is folio-sheet", d == os.path.join(base, "folio-sheet"), d)
        with open(os.path.join(d, "settings.json")) as f:
            check("recent files carried over", json.load(f).get("recent") == ["/tmp/budget.xlsx"])
        check("custom dictionary carried over", open(os.path.join(d, "custom.dic")).read() == "Folioism\n")
        check("themes folder carried over", os.path.isdir(os.path.join(d, "themes")))
        check("old folder kept", os.path.isfile(os.path.join(old, "settings.json")))
        with open(os.path.join(d, "settings.json"), "w") as f:
            json.dump({"recent": []}, f)
        FS.config_dir()
        with open(os.path.join(d, "settings.json")) as f:
            check("copied once only (later changes stay)", json.load(f).get("recent") == [])
        from folio_sheets import proofing
        check("dictionary path under the new folder", proofing.user_words_path() == os.path.join(d, "custom.dic"),
              proofing.user_words_path())
    finally:
        if keep is None:
            os.environ.pop("XDG_CONFIG_HOME", None)
        else:
            os.environ["XDG_CONFIG_HOME"] = keep
except Exception:
    ERRORS.append(traceback.format_exc())

print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
bad = [n for n, ok in checks if not ok]
print("%d checks, %d failed" % (len(checks), len(bad)))
os._exit(1 if bad or ERRORS else 0)
