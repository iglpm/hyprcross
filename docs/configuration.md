# Configuration

hyprcross reads `Hyprcross.toml`, `Cross.toml`, or `[package.metadata.cross]` while walking from
the current directory to the workspace root. `hyprcross init` writes a starter file.

```toml
[build]
default_target = "aarch64-unknown-linux-gnu"
builder = "auto"       # auto, igl, cargo
linker = "hyprlnk"     # hyprlnk, default

[build.env]
RUST_BACKTRACE = "1"

[target.aarch64-unknown-linux-gnu]
env = { OPENSSL_STATIC = "1" }
```

Command-line flags override configuration. Useful SDK overrides are:

| Variable | Meaning |
| :--- | :--- |
| `HYPRCROSS_SDK_DIR` | Directory containing managed target SDKs |
| `HYPRCROSS_SDK_PATH` | SDK path for the current invocation |
| `HYPRCROSS_SDK_PATH_<TARGET>` | SDK path for one target; replace punctuation with `_` and uppercase it |
| `HYPRCROSS_MACOS_SDK_PATH` | Full macOS SDK path |
| `HYPRCROSS_SDK_REPO` | SDK release repository |

Target environment entries are passed to Igel or Cargo. Builder arguments may appear after
`--`, for example `hyprcross test -t aarch64-unknown-linux-gnu -- test_name`.
