"""Workbooks protected with a password to open (S6-30).

The original keeps such a workbook inside an OLE compound file: an EncryptionInfo stream
(how the key is made) and an EncryptedPackage stream (the .xlsx, encrypted). This reads the
Agile and the older Standard encryption and writes Agile (AES-256, SHA-512, 100,000 rounds),
as the original does now. AES comes from python3-cryptography, which Folio Sheet already
depends on; the compound file is read and written here.
"""
import base64
import hashlib
import hmac
import os
import struct
import xml.etree.ElementTree as ET

from cryptography.hazmat.backends import default_backend
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

OLE_MAGIC = bytes.fromhex("D0CF11E0A1B11AE1")
BLK_VERIFIER_INPUT = bytes.fromhex("fea7d2763b4b9e79")
BLK_VERIFIER_VALUE = bytes.fromhex("d7aa0f6d3061344e")
BLK_KEY = bytes.fromhex("146e0be7abacd0d6")
BLK_HMAC_KEY = bytes.fromhex("5fb2ad010cb9e1f6")
BLK_HMAC_VALUE = bytes.fromhex("a0677f02b22c8433")
SEGMENT = 4096


class WrongPassword(Exception):
    pass


class NotEncrypted(Exception):
    pass


# --------------------------------------------------------------------------- compound file

FREESECT, ENDOFCHAIN, FATSECT = 0xFFFFFFFF, 0xFFFFFFFE, 0xFFFFFFFD


def read_streams(data):
    """{path: bytes} for every stream in a compound file ("EncryptionInfo", "\\x06DataSpaces/Version"...)."""
    if data[:8] != OLE_MAGIC:
        raise NotEncrypted("not a compound file")
    shift = struct.unpack_from("<H", data, 30)[0]
    ssz = 1 << shift
    mini_shift = struct.unpack_from("<H", data, 32)[0]
    msz = 1 << mini_shift
    n_fat, first_dir = struct.unpack_from("<I", data, 44)[0], struct.unpack_from("<I", data, 48)[0]
    mini_cut = struct.unpack_from("<I", data, 56)[0]
    first_mini_fat, n_mini_fat = struct.unpack_from("<II", data, 60)
    first_difat, n_difat = struct.unpack_from("<II", data, 68)
    off = lambda s: (s + 1) * ssz
    difat = list(struct.unpack_from("<109I", data, 76))
    s = first_difat
    for _ in range(n_difat):
        if s in (FREESECT, ENDOFCHAIN):
            break
        vals = struct.unpack_from("<%dI" % (ssz // 4), data, off(s))
        difat.extend(vals[:-1])
        s = vals[-1]
    fat = []
    for s in difat[:n_fat]:
        if s == FREESECT:
            continue
        fat.extend(struct.unpack_from("<%dI" % (ssz // 4), data, off(s)))

    def chain(start, table):
        out, s, seen = [], start, set()
        while s not in (ENDOFCHAIN, FREESECT) and s < len(table) and s not in seen:
            seen.add(s)
            out.append(s)
            s = table[s]
        return out

    def read_chain(start):
        return b"".join(data[off(s):off(s) + ssz] for s in chain(start, fat))

    dirdata = read_chain(first_dir)
    entries = []
    for i in range(len(dirdata) // 128):
        e = dirdata[i * 128:(i + 1) * 128]
        nlen = struct.unpack_from("<H", e, 64)[0]
        name = e[:max(nlen - 2, 0)].decode("utf-16-le", "replace")
        typ = e[66]
        left, right, child = struct.unpack_from("<III", e, 68)
        start = struct.unpack_from("<I", e, 116)[0]
        size = struct.unpack_from("<Q", e, 120)[0] if shift > 9 else struct.unpack_from("<I", e, 120)[0]
        entries.append((name, typ, left, right, child, start, size))
    if not entries:
        raise NotEncrypted("empty compound file")
    root = entries[0]
    ministream = read_chain(root[5]) if root[5] not in (ENDOFCHAIN, FREESECT) else b""
    minifat = []
    if n_mini_fat:
        mf = read_chain(first_mini_fat)
        minifat = list(struct.unpack_from("<%dI" % (len(mf) // 4), mf))

    def read_stream(start, size):
        if size < mini_cut:
            return b"".join(ministream[s * msz:(s + 1) * msz] for s in chain(start, minifat))[:size]
        return read_chain(start)[:size]

    out = {}

    def walk(i, prefix, depth=0):
        if i in (FREESECT, 0xFFFFFFFF) or i >= len(entries) or depth > 64:
            return
        name, typ, left, right, child, start, size = entries[i]
        walk(left, prefix, depth + 1)
        walk(right, prefix, depth + 1)
        path = prefix + name
        if typ == 2:
            out[path] = read_stream(start, size)
        elif typ == 1:
            walk(child, path + "/", depth + 1)

    walk(root[4], "")
    return out


def write_streams(streams):
    """A compound file (512-byte sectors) holding {path: bytes}; storages come from the paths."""
    ssz, msz, cut = 512, 64, 4096
    # the tree of storages
    tree = {"": {"kids": {}, "data": None}}
    for path, data in streams.items():
        parts = path.split("/")
        node = tree[""]
        for p in parts[:-1]:
            node = node["kids"].setdefault(p, {"kids": {}, "data": None})
        node["kids"][parts[-1]] = {"kids": {}, "data": data}
    entries = []  # [name, type, left, right, child, start, size]

    def add(name, node):
        idx = len(entries)
        typ = 5 if idx == 0 else (2 if node["data"] is not None else 1)
        entries.append([name, typ, FREESECT, FREESECT, FREESECT, ENDOFCHAIN, 0, node])
        kids = sorted(node["kids"].items(), key=lambda kv: (len(kv[0]), kv[0].upper()))
        ids = [add(k, v) for k, v in kids]

        def bst(lo, hi):
            if lo > hi:
                return FREESECT
            mid = (lo + hi) // 2
            entries[ids[mid]][2] = bst(lo, mid - 1)
            entries[ids[mid]][3] = bst(mid + 1, hi)
            return ids[mid]

        entries[idx][4] = bst(0, len(ids) - 1)
        return idx

    add("Root Entry", tree[""])
    # the mini stream and its FAT
    mini, minifat = bytearray(), []
    big = []  # (entry index, data)
    for e in entries:
        node = e[7]
        if e[1] != 2:
            continue
        data = node["data"]
        e[6] = len(data)
        if len(data) < cut:
            if not data:
                e[5] = ENDOFCHAIN
                continue
            n = (len(data) + msz - 1) // msz
            first = len(minifat)
            minifat.extend(range(first + 1, first + n))
            minifat.append(ENDOFCHAIN)
            e[5] = first
            mini += data + b"\0" * (n * msz - len(data))
        else:
            big.append((e, data))
    sectors = []  # payload sectors in order; FAT built alongside
    fat = []

    def place(data):
        n = max(1, (len(data) + ssz - 1) // ssz)
        first = len(sectors)
        for k in range(n):
            sectors.append(data[k * ssz:(k + 1) * ssz].ljust(ssz, b"\0"))
            fat.append(first + k + 1 if k < n - 1 else ENDOFCHAIN)
        return first

    for e, data in big:
        e[5] = place(data)
    if mini:
        entries[0][5] = place(bytes(mini))
        entries[0][6] = len(mini)
    mf_start = place(struct.pack("<%dI" % len(minifat), *minifat).ljust(((len(minifat) * 4 + ssz - 1) // ssz) * ssz, b"\xff")) if minifat else ENDOFCHAIN
    n_mf = ((len(minifat) * 4 + ssz - 1) // ssz) if minifat else 0
    dirbytes = bytearray()
    for name, typ, left, right, child, start, size, _n in entries:
        nm = name.encode("utf-16-le") + b"\0\0"
        e = nm.ljust(64, b"\0") + struct.pack("<HBB", len(nm), typ, 1) + struct.pack("<III", left, right, child)
        e += b"\0" * 16 + b"\0" * 4 + b"\0" * 16 + struct.pack("<I", start) + struct.pack("<Q", size)
        dirbytes += e
    while len(dirbytes) % ssz:
        dirbytes += (b"\0" * 64 + struct.pack("<HBB", 0, 0, 0) + struct.pack("<III", FREESECT, FREESECT, FREESECT) + b"\0" * 36 + struct.pack("<IQ", 0, 0))
    dir_start = place(bytes(dirbytes))
    # the FAT sectors themselves (and, past 109 of them, DIFAT sectors), marked as such
    n_fat, n_difat = 1, 0
    while True:
        n_difat = 0 if n_fat <= 109 else (n_fat - 109 + 126) // 127
        total = len(sectors) + n_fat + n_difat
        if (total + 127) // 128 <= n_fat:
            break
        n_fat += 1
    fat_start = len(sectors)
    difat_start = fat_start + n_fat
    fat.extend([FATSECT] * n_fat)
    fat.extend([0xFFFFFFFC] * n_difat)  # DIFSECT
    fat.extend([FREESECT] * (n_fat * 128 - len(fat)))
    for k in range(n_fat):
        sectors.append(struct.pack("<128I", *fat[k * 128:(k + 1) * 128]))
    fat_ids = list(range(fat_start, fat_start + n_fat))
    difat = fat_ids[:109] + [FREESECT] * (109 - min(n_fat, 109))
    rest = fat_ids[109:]
    for k in range(n_difat):
        chunk = rest[k * 127:(k + 1) * 127]
        chunk += [FREESECT] * (127 - len(chunk))
        nxt = difat_start + k + 1 if k < n_difat - 1 else ENDOFCHAIN
        sectors.append(struct.pack("<128I", *(chunk + [nxt])))
    hdr = OLE_MAGIC + b"\0" * 16 + struct.pack("<HHHHH", 0x3E, 3, 0xFFFE, 9, 6) + b"\0" * 6
    hdr += struct.pack("<IIIII", 0, n_fat, dir_start, 0, cut) + struct.pack("<II", mf_start, n_mf) + struct.pack("<II", difat_start if n_difat else ENDOFCHAIN, n_difat)
    hdr += struct.pack("<109I", *difat)
    return hdr + b"".join(sectors)


# --------------------------------------------------------------------------- the encryption

def encrypted_file(path):
    """The file's bytes when it is a workbook with a password to open, else None."""
    try:
        with open(path, "rb") as f:
            if f.read(8) != OLE_MAGIC:
                return None
            data = OLE_MAGIC + f.read()
    except OSError:
        return None
    return data if is_encrypted(data) else None


def is_encrypted(data):
    if data[:8] != OLE_MAGIC:
        return False
    try:
        st = read_streams(data)
    except Exception:
        return False
    return "EncryptionInfo" in st and "EncryptedPackage" in st


def _aes(key, iv, data, encrypt):
    c = Cipher(algorithms.AES(key), modes.CBC(iv), backend=default_backend())
    op = c.encryptor() if encrypt else c.decryptor()
    return op.update(data) + op.finalize()


def _hash(name):
    return {"SHA512": hashlib.sha512, "SHA384": hashlib.sha384, "SHA256": hashlib.sha256, "SHA1": hashlib.sha1, "MD5": hashlib.md5}[name.upper().replace("-", "")]


def _fit(b, n, pad=b"\x36"):
    return b[:n] if len(b) >= n else b + pad * (n - len(b))


def _agile_key(password, salt, spin, H, blk, nbytes):
    h = H(salt + password.encode("utf-16-le")).digest()
    for i in range(spin):
        h = H(struct.pack("<I", i) + h).digest()
    return _fit(H(h + blk).digest(), nbytes)


def _decrypt_agile(info_xml, package, password):
    ns = {"e": "http://schemas.microsoft.com/office/2006/encryption", "p": "http://schemas.microsoft.com/office/2006/keyEncryptor/password"}
    root = ET.fromstring(info_xml)
    kd = root.find("e:keyData", ns)
    ek = root.find(".//p:encryptedKey", ns)
    if kd is None or ek is None:
        raise WrongPassword("unsupported encryption")
    H = _hash(ek.get("hashAlgorithm"))
    kbits = int(ek.get("keyBits"))
    salt = base64.b64decode(ek.get("saltValue"))
    spin = int(ek.get("spinCount"))
    k1 = _agile_key(password, salt, spin, H, BLK_VERIFIER_INPUT, kbits // 8)
    k2 = _agile_key(password, salt, spin, H, BLK_VERIFIER_VALUE, kbits // 8)
    k3 = _agile_key(password, salt, spin, H, BLK_KEY, kbits // 8)
    vin = _aes(k1, salt, base64.b64decode(ek.get("encryptedVerifierHashInput")), False)[:int(ek.get("saltSize"))]
    vval = _aes(k2, salt, base64.b64decode(ek.get("encryptedVerifierHashValue")), False)
    if H(vin).digest() != vval[:H().digest_size]:
        raise WrongPassword("The password is incorrect.")
    secret = _aes(k3, salt, base64.b64decode(ek.get("encryptedKeyValue")), False)[:int(kd.get("keyBits")) // 8]
    kd_salt = base64.b64decode(kd.get("saltValue"))
    KH = _hash(kd.get("hashAlgorithm"))
    bs = int(kd.get("blockSize"))
    size = struct.unpack_from("<Q", package, 0)[0]
    body = package[8:]
    out = bytearray()
    for i in range(0, len(body), SEGMENT):
        seg = body[i:i + SEGMENT]
        if len(seg) % bs:
            seg = seg[:len(seg) - len(seg) % bs]
        iv = _fit(KH(kd_salt + struct.pack("<I", i // SEGMENT)).digest(), bs)
        out += _aes(secret, iv, seg, False)
    return bytes(out[:size])


def _decrypt_standard(info, package, password):
    # Standard encryption (Excel 2007): AES-128 in ECB, SHA-1, 50,000 rounds
    hsize = struct.unpack_from("<I", info, 8)[0]
    header = info[12:12 + hsize]
    key_bits = struct.unpack_from("<I", header, 16)[0]
    v = info[12 + hsize:]
    salt = v[4:20]
    enc_verifier = v[20:36]
    enc_hash = v[40:72]
    h = hashlib.sha1(salt + password.encode("utf-16-le")).digest()
    for i in range(50000):
        h = hashlib.sha1(struct.pack("<I", i) + h).digest()
    h = hashlib.sha1(h + struct.pack("<I", 0)).digest()
    x1 = hashlib.sha1(bytes(b ^ 0x36 for b in h.ljust(64, b"\0"))).digest()
    x2 = hashlib.sha1(bytes(b ^ 0x5C for b in h.ljust(64, b"\0"))).digest()
    key = (x1 + x2)[:key_bits // 8]
    ecb = lambda d: Cipher(algorithms.AES(key), modes.ECB(), backend=default_backend()).decryptor().update(d)
    ver = ecb(enc_verifier)
    if hashlib.sha1(ver).digest() != ecb(enc_hash)[:20]:
        raise WrongPassword("The password is incorrect.")
    size = struct.unpack_from("<Q", package, 0)[0]
    body = package[8:]
    body = body[:len(body) - len(body) % 16]
    return ecb(body)[:size]


def decrypt(data, password):
    """The .xlsx inside an encrypted workbook. WrongPassword when the password doesn't open it."""
    st = read_streams(data)
    info, package = st.get("EncryptionInfo"), st.get("EncryptedPackage")
    if info is None or package is None:
        raise NotEncrypted("no encrypted package")
    major, minor = struct.unpack_from("<HH", info, 0)
    if (major, minor) == (4, 4):
        return _decrypt_agile(info[8:], package, password)
    if minor == 2 and major in (2, 3, 4):
        return _decrypt_standard(info, package, password)
    raise WrongPassword("This workbook uses an encryption Folio Sheet can't read.")


def _lp(s):
    b = s.encode("utf-16-le")
    return struct.pack("<I", len(b)) + b + b"\0" * ((4 - len(b) % 4) % 4)


def _dataspaces():
    ver = _lp("Microsoft.Container.DataSpaces") + struct.pack("<HHHHHH", 1, 0, 1, 0, 1, 0)
    entry = struct.pack("<I", 1) + struct.pack("<I", 0) + _lp("EncryptedPackage") + _lp("StrongEncryptionDataSpace")
    dsmap = struct.pack("<II", 8, 1) + struct.pack("<I", len(entry) + 4) + entry
    dsinfo = struct.pack("<II", 8, 1) + _lp("StrongEncryptionTransform")
    tid = _lp("{FF9A3F03-56EF-4613-BDD5-5A41C1D07246}")
    tinfo_body = struct.pack("<I", 1) + tid + _lp("Microsoft.Container.EncryptionTransform") + struct.pack("<HHHHHH", 1, 0, 1, 0, 1, 0)
    primary = struct.pack("<I", len(tid) + 8) + tinfo_body + struct.pack("<I", 0) + struct.pack("<III", 0, 0, 4)
    return {
        "\x06DataSpaces/Version": ver,
        "\x06DataSpaces/DataSpaceMap": dsmap,
        "\x06DataSpaces/DataSpaceInfo/StrongEncryptionDataSpace": dsinfo,
        "\x06DataSpaces/TransformInfo/StrongEncryptionTransform/\x06Primary": primary,
    }


def encrypt(xlsx, password, spin=100000):
    """An encrypted workbook (Agile: AES-256-CBC, SHA-512) holding the .xlsx bytes."""
    H = hashlib.sha512
    bs, kbytes = 16, 32
    secret = os.urandom(kbytes)
    kd_salt = os.urandom(16)
    # the package, in 4096-byte segments
    pkg = bytearray(struct.pack("<Q", len(xlsx)))
    for i in range(0, len(xlsx), SEGMENT):
        seg = xlsx[i:i + SEGMENT]
        seg = seg + b"\0" * ((bs - len(seg) % bs) % bs)
        iv = _fit(H(kd_salt + struct.pack("<I", i // SEGMENT)).digest(), bs)
        pkg += _aes(secret, iv, seg, True)
    pkg = bytes(pkg)
    # data integrity
    hkey = os.urandom(64)
    iv1 = _fit(H(kd_salt + BLK_HMAC_KEY).digest(), bs)
    enc_hkey = _aes(secret, iv1, hkey, True)
    hval = hmac.new(hkey, pkg, hashlib.sha512).digest()
    iv2 = _fit(H(kd_salt + BLK_HMAC_VALUE).digest(), bs)
    enc_hval = _aes(secret, iv2, hval, True)
    # the password's key encryptor
    salt = os.urandom(16)
    vin = os.urandom(16)
    k1 = _agile_key(password, salt, spin, H, BLK_VERIFIER_INPUT, kbytes)
    k2 = _agile_key(password, salt, spin, H, BLK_VERIFIER_VALUE, kbytes)
    k3 = _agile_key(password, salt, spin, H, BLK_KEY, kbytes)
    e_vin = _aes(k1, salt, vin, True)
    e_vval = _aes(k2, salt, H(vin).digest(), True)
    e_key = _aes(k3, salt, secret, True)
    b64 = lambda b: base64.b64encode(b).decode()
    xml = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\r\n'
           '<encryption xmlns="http://schemas.microsoft.com/office/2006/encryption" xmlns:p="http://schemas.microsoft.com/office/2006/keyEncryptor/password">'
           '<keyData saltSize="16" blockSize="16" keyBits="256" hashSize="64" cipherAlgorithm="AES" cipherChaining="ChainingModeCBC" hashAlgorithm="SHA512" saltValue="%s"/>'
           '<dataIntegrity encryptedHmacKey="%s" encryptedHmacValue="%s"/>'
           '<keyEncryptors><keyEncryptor uri="http://schemas.microsoft.com/office/2006/keyEncryptor/password">'
           '<p:encryptedKey spinCount="%d" saltSize="16" blockSize="16" keyBits="256" hashSize="64" cipherAlgorithm="AES" cipherChaining="ChainingModeCBC" hashAlgorithm="SHA512" '
           'saltValue="%s" encryptedVerifierHashInput="%s" encryptedVerifierHashValue="%s" encryptedKeyValue="%s"/>'
           '</keyEncryptor></keyEncryptors></encryption>') % (b64(kd_salt), b64(enc_hkey), b64(enc_hval), spin, b64(salt), b64(e_vin), b64(e_vval), b64(e_key))
    info = struct.pack("<HHI", 4, 4, 0x40) + xml.encode("utf-8")
    streams = dict(_dataspaces())
    streams["EncryptionInfo"] = info
    streams["EncryptedPackage"] = pkg
    return write_streams(streams)
