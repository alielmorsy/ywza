import subprocess


class BridgeTransport:
    """Subprocess transport running `ywza --device <device> bridge`."""

    def __init__(self, device, cli=None):
        self.device = device
        self.cli = cli or "ywza"
        self.proc = None
        self.rf = None
        self.wf = None

    def connect(self):
        self.close()
        p = subprocess.Popen(
            [self.cli, "--device", self.device, "bridge"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        self.proc = p
        self.rf = p.stdout
        self.wf = p.stdin
        return self.rf, self.wf, self.close, self.is_dead

    def close(self):
        if self.proc:
            for f in (self.rf, self.wf):
                if f:
                    try:
                        f.close()
                    except OSError:
                        pass
            try:
                self.proc.kill()
            except OSError:
                pass
            self.proc = None
            self.rf = self.wf = None

    def is_dead(self):
        return self.proc is not None and self.proc.poll() is not None


def connect_bridge(device, cli=None):
    transport = BridgeTransport(device, cli=cli)
    return transport.connect()
