# Run inside Folio Sheet (FOLIO_TEST_SCRIPT): the Backlog 4 items.
# Formulas tab's function menus, Advanced Filter, Consolidate, the Review tools, Custom Views,
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


# dialogs: a queue of callbacks that fill the dialog and return the response
DIALOG = []


def fake_run(d):
    pump(0.05)
    if DIALOG:
        return DIALOG.pop(0)(d)
    return Gtk.ResponseType.CANCEL


Gtk.Dialog.run = fake_run
Gtk.MessageDialog.run = lambda d: (DIALOG.pop(0)(d) if DIALOG else Gtk.ResponseType.OK)


def widgets(w, kind):
    out = []

    def walk(x):
        if isinstance(x, kind):
            out.append(x)
        if isinstance(x, Gtk.Container):
            for c in x.get_children():
                walk(c)
    walk(w)
    return out


g = win.grid
b = win.book
TMP = tempfile.mkdtemp()
from folio_sheets.core import Book  # noqa: E402
from folio_sheets import data_ui, clip_ui  # noqa: E402

# ---------------------------------------------------------------- S3-14 states and provinces
from folio_sheets import chartex_ui  # noqa: E402

rows = [("State", "Sales"), ("Guangdong", "40"), ("Kerala", "12"), ("Gauteng", "7"), ("Bali", "3"),
        ("Kaliningrad", "9")]
for r, row in enumerate(rows):
    for c, v in enumerate(row):
        b.set_input(g.sheet, r, c, v)
pump()
g.select_range((0, 0, len(rows) - 1, 1))
pump()
win.run("chartex:regionmap")
pump(0.2)
mi = g.sel_objects[0] if g.sel_objects else None
info = b.chartex_info(g.sheet, mi) if mi is not None else None
check("S3-14: provinces of five different countries make a map", info and info["kind"] == "regionmap", info)
check("S3-14: all five found, none unknown",
      info and info["map"]["found"] == 5 and not info["map"]["unknown"],
      info and info["map"])

# ---------------------------------------------------------------- S3-15 nothing fetched online
import socket  # noqa: E402

_blocked = []
_real = socket.socket


class NoSocket(socket.socket):
    def connect(self, *a, **k):
        _blocked.append(a)
        raise OSError("the map chart tried to reach the network")


socket.socket = NoSocket
try:
    png = os.path.join(TMP, "map.png")
    b.render_png(g.sheet, png, {"width": 640.0, "height": 420.0})
    made = os.path.exists(png) and os.path.getsize(png) > 2000
finally:
    socket.socket = _real
check("S3-15: a map chart draws with the network refused", made, _blocked)
check("S3-15: and nothing tried to connect", not _blocked, _blocked)

# ---------------------------------------------------------------- S3-17 Insert > Object > Create New
from folio_sheets import insert_extras  # noqa: E402

kinds = insert_extras.new_object_kinds()
check("S3-17: Create New offers the kinds this computer can make", len(kinds) >= 1,
      [k[0] for k in kinds])
if kinds:
    label, template, stem, ext = kinds[0]
    name, body = insert_extras._new_object_body(template, stem, ext)
    oi = insert_extras._place_object(
        win, insert_extras._icon_png(name, name.encode() + b"\x00" + body, b"flEm"), "New " + name)
    f = insert_extras.object_file(win, oi)
    check("S3-17: the new object is embedded whole", f and f[0] == name and f[1] == body, f and f[0])
    check("S3-17: and it is a real document, not an empty file", f and len(f[1]) > 1000, f and len(f[1]))
    out = insert_extras.open_object(win, oi, run=False)
    check("S3-17: it opens back out as a file of its own kind", out and out.endswith(ext) and
          open(out, "rb").read() == body, out)
    # it survives a save and reopen
    p = os.path.join(TMP, "withobject.xlsx")
    win.save_to(p)
    again = Book.open(p).picture_bytes(g.sheet, oi)
    check("S3-17: and it is still there in the saved workbook",
          again and insert_extras.png_chunk(bytes(again[0]), b"flEm") is not None)

# ---------------------------------------------------------------- S3-16 Signature Line with a certificate
# A certificate made here, so the test needs nothing from outside.
P12 = os.path.join(TMP, "signer.p12")
HAVE_CRYPTO = True
try:
    from cryptography import x509
    from cryptography.hazmat.primitives import hashes, serialization
    from cryptography.hazmat.primitives.serialization import pkcs12 as _pkcs12
    from cryptography.hazmat.primitives.asymmetric import rsa
    from cryptography.x509.oid import NameOID
    import datetime as _dt

    _key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    _sub = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "Ron Shillings"),
                      x509.NameAttribute(NameOID.ORGANIZATION_NAME, "Folio")])
    _cert = (x509.CertificateBuilder().subject_name(_sub).issuer_name(_sub)
             .public_key(_key.public_key()).serial_number(x509.random_serial_number())
             .not_valid_before(_dt.datetime.utcnow() - _dt.timedelta(days=1))
             .not_valid_after(_dt.datetime.utcnow() + _dt.timedelta(days=365))
             .sign(_key, hashes.SHA256()))
    with open(P12, "wb") as fh:
        fh.write(_pkcs12.serialize_key_and_certificates(
            b"signer", _key, _cert, None,
            serialization.BestAvailableEncryption(b"secret")))
except Exception as _e:
    HAVE_CRYPTO = False
    print("no cryptography here: %s" % _e, flush=True)

if HAVE_CRYPTO:
    win.run("insert-sheet")
    pump()
    SS = g.sheet
    for r, row in enumerate([("Item", "Amount"), ("Freight", "1200"), ("Fuel", "640")]):
        for c, v in enumerate(row):
            b.set_input(SS, r, c, v)
    pump()
    DIALOG.append(lambda d: ([e.set_text(t) for e, t in zip(d.entries, ("Ron Shillings", "Owner", "ron@example.org", ""))],
                             Gtk.ResponseType.OK)[1])
    win.run("signature-line")
    pump(0.2)
    si = g.sel_objects[0]
    check("S3-16: a signature line to sign", insert_extras.png_chunk(insert_extras._pic(win, si), b"flSg") is not None)

    # the wrong password is refused, and says so
    ok, why = insert_extras.sign_with_certificate(win, si, P12, "wrong")
    check("S3-16: a wrong password is refused", not ok and "couldn't be opened" in why, why)

    ok, who = insert_extras.sign_with_certificate(win, si, P12, "secret")
    check("S3-16: signed with the certificate", ok and who == "Ron Shillings", (ok, who))
    si = g.sel_objects[0]
    r = insert_extras.check_signature(win, si)
    check("S3-16: and the signature checks out", r and r["valid"] and r["unchanged"] and not r["expired"], r)
    check("S3-16: it names the certificate it was signed with",
          r and r["signer"] == "Ron Shillings" and r["issuer"] == "Ron Shillings" and r["algorithm"] == "RSA-SHA256", r)

    # change the sheet underneath it: the signature must notice
    b.set_input(SS, 1, 1, "9999")
    pump()
    r2 = insert_extras.check_signature(win, si)
    check("S3-16: changing the sheet is noticed", r2 and r2["valid"] and not r2["unchanged"], r2)
    check("S3-16: and it says why", r2 and "changed since it was signed" in r2["why"], r2 and r2["why"])
    b.set_input(SS, 1, 1, "1200")          # what was actually there: Freight 1200
    pump()
    check("S3-16: putting it back makes it good again",
          (insert_extras.check_signature(win, si) or {}).get("unchanged"))

    # a signature that was tampered with does not pass
    pic = insert_extras._pic(win, si)
    rec = insert_extras.png_chunk(pic, insert_extras.SIG_CHUNK).decode().split("\n")
    rec[1] = "Someone Else"
    bad = insert_extras._png_add_chunk(
        insert_extras.signature_png("Ron Shillings", "Owner", "ron@example.org", "Someone Else", "01/01/2026"),
        insert_extras.SIG_CHUNK, "\n".join(rec).encode())
    x, y, w, h = b.objects(SS)[si]["rect"]
    bi = b.insert_picture(SS, bad, x + 300, y, w, h)
    rb = insert_extras.check_signature(win, bi)
    check("S3-16: a signature with the name swapped does not pass", rb and not rb["valid"], rb)

# ---------------------------------------------------------------- S4-02 Transform Data on combined files
from folio_sheets import getdata_ui as G, query_ui  # noqa: E402

FOLD = os.path.join(TMP, "monthly")
os.makedirs(FOLD, exist_ok=True)
for _m, _rows in (("jan", "Pens,10\nInk,4\n"), ("feb", "Pens,7\nPaper,0\n")):
    with open(os.path.join(FOLD, _m + ".csv"), "w") as _f:
        _f.write("Item,Count\n" + _rows)

# the menu offers it now, and is not greyed out
_d = G.FolderDialog(win, FOLD)
pump()
_items = [it for it in widgets(_d.buttons.combine_button.get_popup(), Gtk.MenuItem)]
_labels = [(it.get_label(), it.get_sensitive()) for it in _items]
check("S4-02: Combine & Transform Data is offered and can be chosen",
      ("Combine & Transform Data", True) in _labels, _labels)
_d.destroy()

# combining really does hand the files to the step editor, as one table
q = {"kind": "folder", "path": FOLD, "combine": True, "name": "Monthly"}
base = G.read_source(q)
check("S4-02: the files come in together", base.headers == ["Source.Name", "Item", "Count"]
      and len(base.rows) == 4, (base.headers, len(base.rows)))

# steps a person would add in the editor: drop a column, filter, change a type
q["steps"] = [{"op": "remove", "cols": ["Source.Name"]},
              {"op": "filter", "col": "Count", "cmp": "gt", "value": "0"},
              {"op": "type", "col": "Count", "kind": "whole"}]
out = G.read(q)
check("S4-02: the chosen column is gone", out.headers == ["Item", "Count"], out.headers)
check("S4-02: the filter dropped the empty row", len(out.rows) == 3, [r[0] for r in out.rows])
check("S4-02: and the type was changed to a number", out.kinds[1] == "number", out.kinds)
check("S4-02: with whole-number values", all(float(r[1]) == round(float(r[1])) for r in out.rows),
      [r[1] for r in out.rows])

# the steps are kept with the query, so Refresh does them again
CHOICE = {"v": "combine-transform"}
q["name"] = G.unique_query_name(b, "Monthly")
sheet_before = len(b.sheet_names()) if hasattr(b, "sheet_names") else None
where = G.finish(win, dict(q), out, "load")
pump(0.2)
check("S4-02: the transformed table lands in the workbook", where is not None, where)
if where:
    t = b.tables(where[0])[where[1]]
    check("S4-02: with only the columns that were kept", t["columns"] == ["Item", "Count"], t["columns"])
    # a new file arrives: refresh runs the same steps over it
    with open(os.path.join(FOLD, "mar.csv"), "w") as _f:
        _f.write("Item,Count\nGlue,5\nTape,0\n")
    win.run("refresh-all")
    pump(0.3)
    loc = G.locate(b, q["name"])
    check("S4-02: refresh re-reads the folder and applies the steps again",
          loc and loc[2]["ref"][2] == 4, loc and loc[2]["ref"])

# ---------------------------------------------------------------- S3-18 Data Model as Excel's own
from folio_sheets import model_ui, pivot_ui  # noqa: E402

win.run("insert-sheet")
pump()
DM = g.sheet
for r, row in enumerate([("Region", "Amount"), ("East", "10"), ("West", "20"), ("East", "5"), ("North", "7")]):
    for c, v in enumerate(row):
        b.set_input(DM, r, c, v)
for r, row in enumerate([("Region", "Manager"), ("East", "Ann"), ("West", "Bob"), ("North", "Cy")]):
    for c, v in enumerate(row):
        b.set_input(DM, r, 4 + c, v)
b.create_table(DM, (0, 0, 4, 1), True, "TableStyleMedium2")
b.create_table(DM, (0, 4, 3, 5), True, "TableStyleMedium2")
_tn = [t["name"] for t in b.tables(DM)]
SALES, REGIONS = _tn[0], _tn[1]

# with nothing in the model yet, From Data Model says so instead of opening an empty dialog
_said = {"v": None}
_realinfo = win.dialogs.info
win.dialogs.info = lambda msg, title="", *a, **k: _said.__setitem__("v", msg)
pivot_ui.create(win, "model")
win.dialogs.info = _realinfo
check("S3-18: From Data Model says when the model is empty",
      _said["v"] and "nothing in this workbook's Data Model" in _said["v"], _said["v"])


def _rel(d):
    if d.get_title() == "Manage Relationships":
        DIALOG.insert(0, lambda dd: (dd.combos[0].set_active_id(SALES), dd.combos[1].set_active_id("Region"),
                                     dd.combos[2].set_active_id(REGIONS), dd.combos[3].set_active_id("Region"),
                                     Gtk.ResponseType.OK)[-1])
        d.new()
    return Gtk.ResponseType.CLOSE


DIALOG.append(_rel)
win.run("data-relationships")
pump()
_m = model_ui.load(win)
check("S3-18: two tables related", _m["rels"] == [[SALES, "Region", REGIONS, "Region"]], _m)

# and now From Data Model opens the dialog with the model already chosen — it used to refuse
_seen = {}


def _frommodel(d):
    for w in widgets(d, Gtk.RadioButton):
        if w.get_label() == "Use this workbook's Data Model":
            _seen["model_on"] = w.get_active()
            _seen["model_usable"] = w.get_sensitive()
    return Gtk.ResponseType.OK


DIALOG.append(_frommodel)
pivot_ui.create(win, "model")
pump(0.3)
check("S3-18: From Data Model opens the dialog, not a refusal", _seen.get("model_usable"), _seen)
check("S3-18: with the Data Model already chosen", _seen.get("model_on"), _seen)
pi = pivot_ui.current(win)
check("S3-18: and it makes a PivotTable", pi is not None)
if pi is not None:
    _p = b.pivot(pi)
    _names = [f["name"] for f in _p["fields"]]
    check("S3-18: whose fields come from both related tables",
          ("%s.Manager" % REGIONS) in _names and ("%s.Amount" % SALES) in _names, _names)
    # Show Details gives the rows behind a value, on a Data Model PivotTable too
    _lay = pivot_ui.layout_of(_p)
    _fm = _names.index("%s.Manager" % REGIONS)
    _fa = _names.index("%s.Amount" % SALES)
    _p2 = pivot_ui.with_layout(_p, pivot_ui.place(_p, pivot_ui.place(_p, _lay, _fm, "rows"), _fa, "values"))
    b.set_pivot(pi, _p2)
    pump(0.2)
    _sheets_before = len(b.sheet_names()) if hasattr(b, "sheet_names") else None
    # a value cell inside the pivot: the lowest-right cell that still belongs to it
    _cells = [(r, c) for r in range(24) for c in range(12)
              if b.pivot_at(g.sheet, r, c) == pi and b.display(g.sheet, r, c)]
    _target = _cells[-1] if _cells else None
    check("S3-18: the PivotTable is on the sheet to drill into", _target is not None, len(_cells))
    if _target:
        pivot_ui.show_details(win, _target[0], _target[1])
    pump(0.3)
    _sheets_after = len(b.sheet_names()) if hasattr(b, "sheet_names") else None
    check("S3-18: Show Details drills to the rows behind a value",
          _sheets_before is None or _sheets_after > _sheets_before, (_sheets_before, _sheets_after))

# Drill Down says what is true of this Data Model rather than claiming it ought to work
_said2 = {"v": None}
_realinfo2 = win.dialogs.info
win.dialogs.info = lambda msg, title="", *a, **k: _said2.__setitem__("v", msg)
win.run("pivot-drill-down")
win.dialogs.info = _realinfo2
check("S3-18: Drill Down explains there are no hierarchy levels, and points at Show Details",
      _said2["v"] and "no levels to move through" in _said2["v"] and "Show Details" in _said2["v"],
      _said2["v"])

print("%d checks, %d failed" % (len(checks), sum(1 for _, ok in checks if not ok)), flush=True)
print("ERRORS IN CALLBACKS: %d" % len(ERRORS))
for e in ERRORS[:5]:
    print(e)
sys.stdout.flush()
os._exit(1 if ERRORS or any(not ok for _, ok in checks) else 0)
