import itertools
import json
import struct
import sys
import threading
from pathlib import Path

from .bridge import connect_bridge
from .local import YwzaError, config_dir, connect_local

BROADCAST = {"event", "approval_request", "approval_resolved"}


def log(*a):
    print("ywza:", *a, file=sys.stderr, flush=True)


class Ywza:
    """One connection to one agent: the local one, or the paired `device`.
    Thread-safe; requests run one at a time."""

    def __init__(self, client_name="ywza-py", dir=None, device=None):
        self.client_name = client_name
        self.dir = Path(dir) if dir else config_dir()
        self.device = device
        self.rf = self.wf = self.closer = None
        self.dead = None
        self.ids = itertools.count(1)
        self.lock = threading.Lock()

    def connect(self):
        self.close()
        if self.device:
            self.rf, self.wf, self.closer, self.dead = connect_bridge(self.device)
            return
        self.rf, self.wf, self.closer, self.dead = connect_local(self.dir, self.client_name)

    def close(self):
        if self.closer:
            for f in (self.rf, self.wf):
                if f:
                    try:
                        f.close()
                    except OSError:
                        pass
            try:
                self.closer()
            except OSError:
                pass
            self.rf = self.wf = self.closer = self.dead = None

    def _send(self, msg):
        body = json.dumps(msg).encode()
        self.wf.write(struct.pack("<I", len(body)) + body)
        self.wf.flush()

    def _read_exact(self, n):
        buf = self.rf.read(n)
        if len(buf) < n:
            raise ConnectionError(
                "agent connection closed"
                + (f" ({self.device}: see the bridge's stderr)" if self.device else "")
            )
        return buf

    def _recv(self):
        (n,) = struct.unpack("<I", self._read_exact(4))
        return json.loads(self._read_exact(n))

    def request(self, msg, done=lambda m: m.get("type") not in BROADCAST, retry=True):
        """Sends `msg` and returns first accepted frame. Reconnects on dropped connection."""
        with self.lock:
            for attempt in (0, 1) if retry else (1,):
                try:
                    if self.wf and self.dead and self.dead():
                        self.close()
                    if not self.wf:
                        self.connect()
                    self._send(msg)
                    while True:
                        m = self._recv()
                        if m.get("type") == "approval_request":
                            log(f"waiting for approval in the ywza app: {m['tool']}: {m['summary']}")
                        if m.get("type") == "error":
                            raise YwzaError(m["message"])
                        if done(m):
                            return m
                except (ConnectionError, OSError):
                    self.close()
                    if attempt:
                        raise
        raise AssertionError("unreachable")

    def tools(self):
        """[{name, description, parameters (JSON Schema)}]"""
        return self.request({"type": "list_tools"})["list"]

    def call(self, name, args=None, **kw):
        """Runs one tool. Returns {ok, text, png_base64?}; never raises for a failed tool."""
        cid = next(self.ids)
        msg = {"type": "call_tool", "call": cid, "name": name, "args": {**(args or {}), **kw}}
        r = self.request(msg, lambda m: m.get("type") == "tool_result" and m["call"] == cid, retry=False)
        return {k: r[k] for k in ("ok", "text", "png_base64") if k in r}

    def approve(self, request, allow=True, remember=False):
        with self.lock:
            self._send({"type": "approve", "request": request, "allow": allow, "remember": remember})


class Devices:
    """Every agent this machine can reach: "local" (this PC) and each paired device.
    Connections open on first use and stay open."""

    def __init__(self, client_name="ywza-py", dir=None):
        self.client_name = client_name
        self.dir = Path(dir) if dir else config_dir()
        self.conns = {}

    def names(self):
        local = ["local"] if (self.dir / "agent.ticket").exists() else []
        return local + sorted(p.stem for p in (self.dir / "devices").glob("*.json"))

    def default(self):
        names = self.names()
        return "local" if "local" in names else names[0] if len(names) == 1 else None

    def get(self, name=None):
        names = self.names()
        name = name or self.default()
        if name is None:
            raise YwzaError(f"pass `device`, one of: {', '.join(names) or 'none paired'}")
        if name not in names:
            raise YwzaError(f"unknown device `{name}`; known: {', '.join(names) or 'none'}")
        if name not in self.conns:
            dev = None if name == "local" else name
            self.conns[name] = Ywza(self.client_name, self.dir, device=dev)
        return self.conns[name]

    def tools(self):
        """Tool specs from the first reachable device (all agents share one tool set)."""
        errors = []
        for name in dict.fromkeys([self.default(), *self.names()]):
            if name:
                try:
                    return self.get(name).tools()
                except (YwzaError, OSError) as e:
                    errors.append(f"{name}: {e}")
        raise YwzaError("no device reachable. " + "; ".join(errors))


__all__ = ["Ywza", "Devices", "YwzaError", "config_dir"]
