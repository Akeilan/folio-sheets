"""What differs between Linux and Windows, in one place for every Folio app. Each app carries its
own copy (folio_os.py beside its other modules); this one in Folio Studio is the original.

On Linux everything here does what the apps did before: xdg-open, xdg-email, /usr/share, CUPS.
On Windows (the suite built with MSYS2, see Folio Studio's windows/ folder) the same calls go to
the shell, MAPI and the folders the installer puts beside the program."""
import os
import shutil
import subprocess
import sys

WINDOWS = sys.platform == "win32"
# a native engine's file name: a Python extension module is a .so on Linux, a .pyd on Windows
EXT_MODULE = ".pyd" if WINDOWS else ".so"


def root():
    """Windows: the folder Folio Studio is installed in (FOLIO_ROOT, set by the launcher).
    Linux: None."""
    return os.environ.get("FOLIO_ROOT") or None


def file_uri(path):
    """A file's address (file:///...). On Linux the same as "file://" + quote(path)."""
    import pathlib
    return pathlib.Path(os.path.abspath(path)).as_uri()


def uri_path(uri):
    """The file a file:// address names, or None for any other address."""
    if not uri or not uri.startswith("file:"):
        return None
    from urllib.parse import unquote, urlparse
    p = urlparse(uri)
    path = unquote(p.path)
    if WINDOWS:
        if p.netloc and p.netloc not in ("", "localhost"):
            return "\\\\" + p.netloc + path.replace("/", "\\")    # a share: \\server\\folder
        if len(path) > 2 and path[0] == "/" and path[2] == ":":
            path = path[1:]
        return path.replace("/", "\\")
    return path


def share_dirs(name):
    """Where an app's own files are: FOLIO_SHARE when set, <root>/share/<name> on Windows, then
    /usr/share/<name> and /usr/local/share/<name>. Only the ones that exist."""
    out = []
    if os.environ.get("FOLIO_SHARE"):
        out.append(os.environ["FOLIO_SHARE"])
    if root():
        out.append(os.path.join(root(), "share", name))
    out += [os.path.join("/usr/share", name), os.path.join("/usr/local/share", name)]
    return [d for d in out if os.path.isdir(d)]


def dict_dirs():
    """Folders with spelling, thesaurus and hyphenation dictionaries (Hunspell and MyThes files)."""
    out = []
    if root():
        for d in ("hunspell", "mythes", "hyphen", "myspell"):
            out.append(os.path.join(root(), "share", d))
    out += ["/usr/share/hunspell", "/usr/share/myspell/dicts", "/usr/share/mythes", "/usr/share/hyphen"]
    return [d for d in out if os.path.isdir(d)]


def user_name(default="Author"):
    """The signed-in person's full name where the system has one, else their account name."""
    if not WINDOWS:
        try:
            import pwd
            n = pwd.getpwuid(os.getuid()).pw_gecos.split(",")[0].strip()
            if n:
                return n
        except Exception:       # noqa: BLE001
            pass
    else:
        try:
            import ctypes
            size = ctypes.c_ulong(256)
            buf = ctypes.create_unicode_buffer(256)
            # NameDisplay (3): the full name, when the account has one
            if ctypes.windll.secur32.GetUserNameExW(3, buf, ctypes.byref(size)) and buf.value.strip():
                return buf.value.strip()
        except Exception:       # noqa: BLE001
            pass
    return os.environ.get("USER") or os.environ.get("USERNAME") or default


def open_path(path):
    """Open a file, a folder or a web address in what the desktop uses for it. True if started."""
    try:
        if WINDOWS:
            os.startfile(path)                      # noqa: S606 - the shell's own "open"
            return True
        if shutil.which("xdg-open"):
            subprocess.Popen(["xdg-open", path])
            return True
    except OSError:
        return False
    try:
        from gi.repository import Gio
        uri = path if "://" in path or path.startswith("mailto:") else Gio.File.new_for_path(path).get_uri()
        return Gio.AppInfo.launch_default_for_uri(uri, None)
    except Exception:       # noqa: BLE001
        return False


def show_in_folder(path):
    """The folder a file is in, with the file picked where the file manager can do that."""
    if WINDOWS:
        try:
            subprocess.Popen(["explorer", "/select,", os.path.normpath(path)])
            return True
        except OSError:
            return False
    return open_path(os.path.dirname(path) or ".")


def can_mail():
    """Whether a new message with an attachment can be started on this computer."""
    if WINDOWS:
        return _mapi() is not None
    return bool(shutil.which("xdg-email"))


def mail_file(path, subject="", body="", to=""):
    """A new message in the computer's mail program with `path` attached (None: no attachment),
    ready to address and send. True if the mail program took it."""
    if WINDOWS:
        return _mapi_send(path, subject, body, to)
    if not shutil.which("xdg-email"):
        return False
    args = ["xdg-email"]
    if subject:
        args += ["--subject", subject]
    if body:
        args += ["--body", body]
    if path:
        args += ["--attach", path]
    if to:
        args.append(to)
    try:
        subprocess.Popen(args)
        return True
    except OSError:
        return False


def _mapi():
    if not WINDOWS:
        return None
    try:
        import ctypes
        return ctypes.WinDLL("mapi32.dll")
    except OSError:
        return None


def _mapi_send(path, subject, body, to=""):
    """Simple MAPI: the default mail program (Outlook, Thunderbird, Windows Mail through its MAPI
    handler) opens a message with the file attached and lets the person send it."""
    lib = _mapi()
    if lib is None:
        return False
    import ctypes
    from ctypes import wintypes

    class FileDesc(ctypes.Structure):
        _fields_ = [("ulReserved", wintypes.ULONG), ("flFlags", wintypes.ULONG), ("nPosition", wintypes.ULONG),
                    ("lpszPathName", wintypes.LPWSTR), ("lpszFileName", wintypes.LPWSTR), ("lpFileType", ctypes.c_void_p)]

    class Recip(ctypes.Structure):
        _fields_ = [("ulReserved", wintypes.ULONG), ("ulRecipClass", wintypes.ULONG), ("lpszName", wintypes.LPWSTR),
                    ("lpszAddress", wintypes.LPWSTR), ("ulEIDSize", wintypes.ULONG), ("lpEntryID", ctypes.c_void_p)]

    class Message(ctypes.Structure):
        _fields_ = [("ulReserved", wintypes.ULONG), ("lpszSubject", wintypes.LPWSTR), ("lpszNoteText", wintypes.LPWSTR),
                    ("lpszMessageType", wintypes.LPWSTR), ("lpszDateReceived", wintypes.LPWSTR),
                    ("lpszConversationID", wintypes.LPWSTR), ("flFlags", wintypes.ULONG), ("lpOriginator", ctypes.c_void_p),
                    ("nRecipCount", wintypes.ULONG), ("lpRecips", ctypes.POINTER(Recip)), ("nFileCount", wintypes.ULONG),
                    ("lpFiles", ctypes.POINTER(FileDesc))]
    try:
        send = lib.MAPISendMailW
    except AttributeError:
        return False
    files = (FileDesc * 1)()
    n = 0
    if path:
        files[0] = FileDesc(0, 0, 0xFFFFFFFF, os.path.abspath(path), os.path.basename(path), None)
        n = 1
    recips = (Recip * 1)()
    nr = 0
    if to:
        recips[0] = Recip(0, 1, to, "SMTP:" + to, 0, None)      # MAPI_TO
        nr = 1
    msg = Message(0, subject or None, body or None, None, None, None, 0, None, nr, recips if nr else None,
                  n, files if n else None)
    MAPI_LOGON_UI, MAPI_DIALOG = 0x1, 0x8
    try:
        r = send(0, 0, ctypes.byref(msg), MAPI_LOGON_UI | MAPI_DIALOG, 0)
    except OSError:
        return False
    return r in (0, 1)          # 1: the person closed the message without sending: still handled


def process_alive(pid):
    """Whether a process with this number is running. (os.kill(pid, 0) asks that on Linux; on
    Windows the same call ends the process, so it is never used there.)"""
    try:
        pid = int(pid)
    except (TypeError, ValueError):
        return False
    if pid <= 0:
        return False
    if WINDOWS:
        try:
            import ctypes
            k = ctypes.windll.kernel32
            h = k.OpenProcess(0x1000, False, pid)        # PROCESS_QUERY_LIMITED_INFORMATION
            if not h:
                return False
            code = ctypes.c_ulong()
            ok = k.GetExitCodeProcess(h, ctypes.byref(code))
            k.CloseHandle(h)
            return bool(ok) and code.value == 259        # STILL_ACTIVE
        except Exception:       # noqa: BLE001
            return False
    try:
        os.kill(pid, 0)
        return True
    except PermissionError:
        return True
    except OSError:
        return False


def end_process(pid):
    """Ask a process of our own to stop (SIGTERM on Linux, TerminateProcess on Windows)."""
    try:
        if WINDOWS:
            subprocess.run(["taskkill", "/PID", str(int(pid)), "/T", "/F"], capture_output=True, timeout=10)
        else:
            import signal
            os.kill(int(pid), signal.SIGTERM)
        return True
    except (OSError, ValueError, subprocess.SubprocessError):
        return False


def prefers_dark():
    """Windows: whether apps are set to Dark (Settings > Personalisation > Colours). None where
    the system doesn't say, and always None on Linux (the apps ask the desktop themselves)."""
    if not WINDOWS:
        return None
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize") as k:
            v, _t = winreg.QueryValueEx(k, "AppsUseLightTheme")
            return int(v) == 0
    except OSError:
        return None


def office_program():
    """LibreOffice's soffice, for the old formats it alone converts, or None."""
    p = shutil.which("soffice") or shutil.which("libreoffice")
    if p or not WINDOWS:
        return p
    for base in (os.environ.get("ProgramFiles"), os.environ.get("ProgramFiles(x86)")):
        if base:
            c = os.path.join(base, "LibreOffice", "program", "soffice.exe")
            if os.path.exists(c):
                return c
    return None


def printers():
    """The printers this computer has, the default first. Linux: CUPS; Windows: the spooler."""
    if WINDOWS:
        return _win_printers()
    out = []
    try:
        r = subprocess.run(["lpstat", "-d"], capture_output=True, text=True, timeout=5)
        if ":" in r.stdout:
            d = r.stdout.split(":", 1)[1].strip()
            if d:
                out.append(d)
        r = subprocess.run(["lpstat", "-a"], capture_output=True, text=True, timeout=5)
        for line in r.stdout.splitlines():
            n = line.split(" ", 1)[0]
            if n and n not in out:
                out.append(n)
    except (OSError, subprocess.SubprocessError):
        pass
    return out


def _win_printers():
    try:
        import ctypes
        from ctypes import wintypes
        w = ctypes.WinDLL("winspool.drv")
        size = wintypes.DWORD(0)
        w.GetDefaultPrinterW(None, ctypes.byref(size))
        default = None
        if size.value:
            buf = ctypes.create_unicode_buffer(size.value)
            if w.GetDefaultPrinterW(buf, ctypes.byref(size)):
                default = buf.value

        class PI4(ctypes.Structure):
            _fields_ = [("pPrinterName", wintypes.LPWSTR), ("pServerName", wintypes.LPWSTR), ("Attributes", wintypes.DWORD)]
        need, count = wintypes.DWORD(0), wintypes.DWORD(0)
        flags = 0x2 | 0x4          # local and connected printers
        w.EnumPrintersW(flags, None, 4, None, 0, ctypes.byref(need), ctypes.byref(count))
        out = [default] if default else []
        if need.value:
            buf = ctypes.create_string_buffer(need.value)
            if w.EnumPrintersW(flags, None, 4, buf, need, ctypes.byref(need), ctypes.byref(count)):
                arr = ctypes.cast(buf, ctypes.POINTER(PI4))
                for i in range(count.value):
                    n = arr[i].pPrinterName
                    if n and n not in out:
                        out.append(n)
        return out
    except Exception:       # noqa: BLE001
        return []


def print_pdf(path, printer=None, copies=1, options=None):
    """Send a PDF to a printer. Linux: lp with CUPS options. Windows: the PDF program's own
    print command (the "printto" verb), once per copy. Returns an error in plain words, or None."""
    if WINDOWS:
        try:
            import ctypes
            sh = ctypes.windll.shell32
            for _ in range(max(1, int(copies))):
                if printer:
                    r = sh.ShellExecuteW(None, "printto", path, '"%s"' % printer, None, 0)
                else:
                    r = sh.ShellExecuteW(None, "print", path, None, None, 0)
                if r <= 32:
                    return ("Windows has no program set up to print PDF files. Install a PDF "
                            "viewer, or save as PDF and print from there.")
            return None
        except Exception as e:       # noqa: BLE001
            return str(e)
    args = ["lp"]
    if printer:
        args += ["-d", printer]
    args += ["-n", str(max(1, int(copies)))]
    for o in options or ():
        args += ["-o", o]
    args.append(path)
    try:
        r = subprocess.run(args, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        return "Printing isn't available: the system's print service (CUPS) isn't installed."
    return None if r.returncode == 0 else (r.stderr.strip() or "The printer didn't take the job.")


def detached(args, **kw):
    """Start a program that outlives this one."""
    if WINDOWS:
        kw.setdefault("creationflags", 0x00000008 | 0x00000200)   # DETACHED_PROCESS, NEW_PROCESS_GROUP
    else:
        kw.setdefault("start_new_session", True)
    return subprocess.Popen(args, **kw)
