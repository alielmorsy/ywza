import base64
import json
import sys
from pathlib import Path

from . import Devices
from .server import serve_mcp

DOC = """Drive ywza agents' tools from Python or from your own AI over MCP.

One MCP server reaches every agent this machine can talk to:
- "local": this PC's agent, straight over its 127.0.0.1 port (crates/ywza-agent/src/local.rs)
  with port + token from <config_dir>/local-tcp.json.
- each paired device (`ywza pair <code> --name NAME`, listed by `ywza devices`): over the
  encrypted iroh link (LAN, internet, 4G) through `ywza --device NAME bridge`. The `ywza`
  binary comes from --cli PATH, $YWZA_CLI, or PATH.
Every tool takes an optional `device`; `list_devices` lists them. Standard library only.

    ywza-mcp [--cli PATH]                     # MCP server on stdio
    ywza-mcp devices                          # list devices
    ywza-mcp [--device NAME] tools            # list tool names
    ywza-mcp [--device NAME] call list_windows
    ywza-mcp call wait '{"ms": 500}'

    from ywza_mcp import Devices
    d = Devices()
    print(d.get("linux-vm").call("list_windows")["text"])
"""


def main(argv=None):
    if argv is None:
        argv = sys.argv[1:]
    else:
        argv = list(argv)

    def opt(flag):
        if flag not in argv:
            return None
        k = argv.index(flag)
        value = argv[k + 1]
        del argv[k : k + 2]
        return value

    cli, device = opt("--cli"), opt("--device")
    if not argv:
        serve_mcp(Devices("mcp", cli=cli))
        return 0
    devs = Devices("ywza-py", cli=cli)
    sys.stdout.reconfigure(errors="replace")
    if argv[0] == "devices":
        print("\n".join(devs.names()))
        return 0
    y = devs.get(device)
    if argv[0] == "tools":
        for t in y.tools():
            print(f"{t['name']:16} {t['description'].splitlines()[0]}")
        return 0
    if argv[0] == "call" and len(argv) in (2, 3):
        r = y.call(argv[1], json.loads(argv[2]) if len(argv) == 3 else {})
        print(r["text"])
        if r.get("png_base64"):
            Path("ywza-screenshot.png").write_bytes(base64.b64decode(r["png_base64"]))
            print("(image saved to ywza-screenshot.png)")
        return 0 if r["ok"] else 1
    print(DOC, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
