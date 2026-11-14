from concurrent.futures import ThreadPoolExecutor

from .local import YwzaError

FANOUT = {
    "name": "fanout",
    "description": "Run one tool on several computers at once and get one section per device "
    '("[vm] ok" and its output, or "[pc] error: why"). Images are dropped. Use it to check the '
    'same thing everywhere, e.g. tool "find_app" to see which computer can launch an app.',
    "inputSchema": {
        "type": "object",
        "properties": {
            "tool": {"type": "string", "description": "Tool name, e.g. find_app."},
            "args": {"type": "object", "description": "Arguments of that tool."},
            "devices": {
                "type": "array",
                "items": {"type": "string"},
                "description": 'Names from list_devices; ["all"] is every one of them.',
            },
        },
        "required": ["tool", "devices"],
    },
}


def section(device, ok, text, image=False):
    note = "\n(image omitted)" if image else ""
    return f"[{device}] ok\n{text}{note}" if ok else f"[{device}] error: {text}{note}"


def _one(devs, name, tool, args):
    try:
        r = devs.get(name).call(tool, dict(args))
    except (YwzaError, OSError) as e:
        return section(name, False, str(e)), False
    return section(name, r["ok"], r["text"], bool(r.get("png_base64"))), r["ok"]


def fanout(devs, tool, args, devices):
    """Returns (ok, text); ok when at least one device succeeded."""
    if not isinstance(devices, list):
        devices = []
    if devices == ["all"]:
        devices = devs.names()
    devices = list(dict.fromkeys(d for d in devices if isinstance(d, str)))
    if not tool or not devices:
        return False, "fanout needs `tool` and at least one device"
    with ThreadPoolExecutor(max_workers=len(devices)) as pool:
        results = list(pool.map(lambda d: _one(devs, d, tool, args or {}), devices))
    return any(ok for _, ok in results), "\n\n".join(text for text, _ in results)
