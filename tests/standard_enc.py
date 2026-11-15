"""Test helper: a workbook in the older Standard encryption (Excel 2007: AES-128, SHA-1),
written straight from MS-OFFCRYPTO 2.3.4.5-2.3.4.9 so that Sheet's reader can be checked
against msoffcrypto-tool's on a file Sheet's own writer didn't make."""
import hashlib, os, struct, sys
from cryptography.hazmat.backends import default_backend
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes


def standard(xlsx, password, write_streams):
    salt = os.urandom(16)
    h = hashlib.sha1(salt + password.encode("utf-16-le")).digest()
    for i in range(50000):
        h = hashlib.sha1(struct.pack("<I", i) + h).digest()
    h = hashlib.sha1(h + struct.pack("<I", 0)).digest()
    x1 = hashlib.sha1(bytes(b ^ 0x36 for b in h.ljust(64, b"\0"))).digest()
    x2 = hashlib.sha1(bytes(b ^ 0x5C for b in h.ljust(64, b"\0"))).digest()
    key = (x1 + x2)[:16]
    enc = lambda d: Cipher(algorithms.AES(key), modes.ECB(), backend=default_backend()).encryptor().update(d)
    verifier = os.urandom(16)
    vh = hashlib.sha1(verifier).digest().ljust(32, b"\0")
    csp = "Microsoft Enhanced RSA and AES Cryptographic Provider".encode("utf-16-le") + b"\0\0"
    header = struct.pack("<IIIIIIII", 0x24, 0, 0x660E, 0x8004, 128, 0x18, 0, 0) + csp
    info = struct.pack("<HHI", 3, 2, 0x24) + struct.pack("<I", len(header)) + header
    info += struct.pack("<I", 16) + salt + enc(verifier) + struct.pack("<I", 20) + enc(vh)
    body = xlsx + b"\0" * ((16 - len(xlsx) % 16) % 16)
    pkg = struct.pack("<Q", len(xlsx)) + enc(body)
    return write_streams({"EncryptionInfo": info, "EncryptedPackage": pkg})
