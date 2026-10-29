"""Folio Sheet (called Folio Sheets before 0.19)."""
import os
import shutil

VERSION = "0.19.0"
APP_NAME = "Folio Sheet"
APP_ID = "org.folio.Sheet"


def _share():
    env = os.environ.get("FOLIO_SHEET_SHARE") or os.environ.get("FOLIO_SHEETS_SHARE")
    if env:
        return env
    tree = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "packaging", "root", "usr", "share")
    for d in ("/usr/share/folio-sheet", "/usr/share/folio-sheets", os.path.join(tree, "folio-sheet")):
        if os.path.isdir(d):
            return d
    return "/usr/share/folio-sheet"


SHARE = _share()
CONFIG_NAME = "folio-sheet"
OLD_CONFIG_NAME = "folio-sheets"


def config_dir():
    """~/.config/folio-sheet. On first start after the rename the old ~/.config/folio-sheets is
    copied over (settings, recent files, custom dictionary, themes); the old folder is kept."""
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    new = os.path.join(base, CONFIG_NAME)
    old = os.path.join(base, OLD_CONFIG_NAME)
    if not os.path.exists(new) and os.path.isdir(old):
        try:
            shutil.copytree(old, new, symlinks=True)
        except Exception:
            pass
    return new
