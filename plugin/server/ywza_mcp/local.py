import json
import os
import select
import socket
import struct
import sys
from pathlib import Path


class YwzaError(Exception):
    pass


def config_dir():
    """Resolve config directory across platforms."""
    if os.environ.get("YWZA_CONFIG_DIR"):
        return Path(os.environ["YWZA_CONFIG_DIR"])
    if sys.platform == "win32" and os.environ.get("ProgramData"):
        service = Path(os.environ["ProgramData"]) / "ywza"
    elif sys.platform.startswith("linux"):
        service = Path("/var/lib/ywza")
    else:
        service = None
    if service and os.access(service / "agent.ticket", os.R_OK):
        return service
    if sys.platform == "win32":
        base = Path(os.environ["APPDATA"])
    elif sys.platform == "darwin":
        base = Path.home() / "Library" / "Application Support"
    else:
        base = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return base / "ywza"


def read_local_tcp(dir_path=None):
    base = Path(dir_path) if dir_path else config_dir()
    path = base / "local-tcp.json"
    try:
        return json.loads(path.read_text())
    except OSError as e:
        raise YwzaError(f"cannot read {path}: is ywza-agent running? ({e})") from e


class LocalClient:
    """TCP client connecting to the local ywza-agent."""

    def __init__(self, dir_path=None, client_name="ywza-py"):
        self.dir = Path(dir_path) if dir_path else config_dir()
        self.client_name = client_name
        self.sock = None
        self.rf = None
        self.wf = None
        self.dead = None

    def connect(self):
        self.close()
        info = read_local_tcp(self.dir)
        sock = socket.create_connection(("127.0.0.1", info["port"]), timeout=10)
        sock.settimeout(None)
        self.sock = sock
        self.rf = sock.makefile("rb")
        self.wf = sock.makefile("wb")

        def dead():
            if not select.select([sock], [], [], 0)[0]:
                return False
            try:
                return sock.recv(1, socket.MSG_PEEK) == b""
            except OSError:
                return True

        self.dead = dead

        # Send handshake
        body = json.dumps({
            "type": "hello",
            "client_name": self.client_name,
            "pairing_code": info["token"],
        }).encode()
        self.wf.write(struct.pack("<I", len(body)) + body)
        self.wf.flush()

        hdr = self.rf.read(4)
        if len(hdr) < 4:
            raise ConnectionError("agent connection closed")
        (n,) = struct.unpack("<I", hdr)
        payload = self.rf.read(n)
        if len(payload) < n:
            raise ConnectionError("agent connection closed")
        welcome = json.loads(payload)
        if welcome.get("type") != "welcome":
            raise YwzaError(f"expected welcome, got {welcome}")

        return self.rf, self.wf, self.close, self.dead

    def close(self):
        if self.sock:
            for f in (self.rf, self.wf):
                if f:
                    try:
                        f.close()
                    except OSError:
                        pass
            try:
                self.sock.close()
            except OSError:
                pass
            self.sock = None
            self.rf = self.wf = self.dead = None


def connect_local(dir_path=None, client_name="ywza-py"):
    client = LocalClient(dir_path=dir_path, client_name=client_name)
    return client.connect()
