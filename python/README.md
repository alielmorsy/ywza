# ywza-mcp

MCP server and Python client for ywza agents. Standard library only.

## Install

```bash
uv tool install ./python          # puts `ywza-mcp` on PATH
uv add ./python                   # or use it as a library in your project
```

Run without installing:

```bash
uvx --from ./python ywza-mcp devices
```

## MCP

```bash
claude mcp add ywza -- ywza-mcp
claude mcp add ywza -- ywza-mcp --cli /path/to/ywza   # if `ywza` is not on PATH
```

## CLI

```bash
ywza-mcp devices
ywza-mcp tools
ywza-mcp --device linux-vm call list_windows
ywza-mcp call wait '{"ms": 500}'
```

## Library

```python
from ywza_mcp import Devices, Ywza

print(Ywza().call("list_windows")["text"])
print(Devices().get("linux-vm").call("list_windows")["text"])
```
