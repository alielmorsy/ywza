import json
import sys
from .files import COPY, copy
from .local import YwzaError

DEVICE_ARG = {
    "type": "string",
    "description": 'Computer to act on, from list_devices. Default: "local" (this PC).',
}
LIST_DEVICES = {
    "name": "list_devices",
    "description": 'List the computers you can drive: "local" (this PC) and every paired device '
    "(other PCs, VMs, laptops). Pass one as `device` to any other tool; ids such as windows "
    "and elements belong to the device they came from.",
    "inputSchema": {"type": "object", "properties": {}},
}


def mcp_result(devs, method, params):
    if method == "initialize":
        return {
            "protocolVersion": params.get("protocolVersion", "2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "ywza", "version": "0.1"},
            "instructions": "Tools that see and drive computers' desktops and terminals: this PC "
            "and every paired device (list_devices; pass `device` to target one). "
            "Start with list_windows, then get_elements on a window; act on element ids. "
            "Some actions wait for the user's approval and may be denied: do not retry those another way.",
        }
    if method == "ping":
        return {}
    if method == "tools/list":
        tools = [LIST_DEVICES, COPY]
        for t in devs.tools():
            schema = t["parameters"]
            props = {**schema.get("properties", {}), "device": DEVICE_ARG}
            tools.append({
                "name": t["name"],
                "description": t["description"],
                "inputSchema": {**schema, "properties": props},
            })
        return {"tools": tools}
    if method == "tools/call":
        args = dict(params.get("arguments") or {})
        if params["name"] == "list_devices":
            default = devs.default()
            lines = [n + (" (default)" if n == default else "") for n in devs.names()]
            return {
                "content": [{"type": "text", "text": "\n".join(lines) or "no devices"}],
                "isError": False,
            }
        if params["name"] == "copy":
            try:
                ok, text = copy(args.get("src", ""), args.get("dst", ""), devs.names())
            except OSError as e:
                ok, text = False, str(e)
            return {"content": [{"type": "text", "text": text}], "isError": not ok}
        try:
            y = devs.get(args.pop("device", None))
        except YwzaError as e:
            return {"content": [{"type": "text", "text": str(e)}], "isError": True}
        r = y.call(params["name"], args)
        content = [{"type": "text", "text": r["text"]}]
        if r.get("png_base64"):
            content.append({"type": "image", "data": r["png_base64"], "mimeType": "image/png"})
        return {"content": content, "isError": not r["ok"]}
    raise LookupError(method)


def handle(devs, req):
    """Handle one JSON-RPC message, returning response dict or None for notifications."""
    rid, method = req.get("id"), req.get("method", "")
    if rid is None:
        return None
    try:
        return {"jsonrpc": "2.0", "id": rid, "result": mcp_result(devs, method, req.get("params") or {})}
    except LookupError:
        err = {"code": -32601, "message": f"method not found: {method}"}
    except Exception as e:
        err = {"code": -32000, "message": f"{type(e).__name__}: {e}"}
    return {"jsonrpc": "2.0", "id": rid, "error": err}


def serve_mcp(devs):
    out = sys.stdout.buffer
    for line in sys.stdin.buffer:
        if not line.strip():
            continue
        try:
            req = json.loads(line)
        except ValueError:
            resp = {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "parse error"}}
        else:
            resp = handle(devs, req)
        if resp is not None:
            out.write(json.dumps(resp).encode() + b"\n")
            out.flush()
