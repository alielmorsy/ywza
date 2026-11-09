import shutil
import subprocess
from pathlib import Path

COPY = {
    "name": "copy",
    "description": "Copy a file or a whole folder between computers, scp style. A path is "
    '"device:path" on a paired device from list_devices (e.g. "vm:/home/vm/folder"), or a plain '
    'path on this PC (e.g. "E:\\files"). If the destination is an existing folder the source '
    "goes inside it. On a remote device, relative paths and ~ are under its user's home.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "src": {"type": "string", "description": "Source file or folder."},
            "dst": {"type": "string", "description": "Destination path."},
        },
        "required": ["src", "dst"],
    },
}


def _local(path, devices):
    """The local path if `path` is on this PC, else None."""
    dev, sep, rest = path.partition(":")
    if sep and dev == "local":
        return rest
    if sep and len(dev) > 1 and dev in devices:
        return None
    return path


def copy(src, dst, devices, cli=None):
    """Returns (ok, text)."""
    ls, ld = _local(src, devices), _local(dst, devices)
    if ls is not None and ld is not None:
        s, d = Path(ls), Path(ld)
        if d.is_dir():
            d = d / s.name
        if s.is_dir():
            shutil.copytree(s, d, dirs_exist_ok=True)
        else:
            shutil.copy2(s, d)
        return True, f"copied to {d}"
    p = subprocess.run(
        [cli or "ywza", "cp", "-r", src, dst],
        capture_output=True,
        text=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
    )
    text = (p.stdout + p.stderr).strip()
    return p.returncode == 0, text or f"ywza cp exited with {p.returncode}"
