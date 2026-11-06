#!/usr/bin/env python3
import sys
from pathlib import Path

_pkg_dir = str(Path(__file__).resolve().parent)
if _pkg_dir not in sys.path:
    sys.path.insert(0, _pkg_dir)

from ywza_mcp import Devices, Ywza, YwzaError, config_dir
from ywza_mcp.__main__ import main

if __name__ == "__main__":
    sys.exit(main())
