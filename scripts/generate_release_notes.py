#!/usr/bin/env python3
"""
Generates rich release notes for GitHub Releases, including:
1. Release changelog notes extracted from CHANGELOG.md
2. Tutorial-style guide on how to upgrade or install hyprcross (via Cargo and Standalone)
3. A clean markdown table of all binaries with clickable direct download links and SHA256 checksums
"""

import os
import re
import sys

def main():
    # 1. Determine release tag and version
    tag = os.environ.get("GITHUB_REF_NAME", "").strip()
    
    # Fallback to Cargo.toml if no tag provided (e.g. running locally or on main branch)
    if not tag or not tag.startswith("v"):
        try:
            with open("Cargo.toml", "r", encoding="utf-8") as f:
                for line in f:
                    if line.strip().startswith("version ="):
                        m = re.search(r'version\s*=\s*"([^"]+)"', line)
                        if m:
                            tag = f"v{m.group(1)}"
                            break
        except Exception:
            pass

    if not tag:
        tag = "v2027.0.0-rc.1"

    version = tag.lstrip("v")
    base_url = f"https://github.com/runepm/hyprcross/releases/download/{tag}"

    # 2. Extract changelog section for this version
    changelog_section = ""
    if os.path.exists("CHANGELOG.md"):
        try:
            with open("CHANGELOG.md", "r", encoding="utf-8") as f:
                content = f.read()
            pattern = rf"^##\s*\[?{re.escape(version)}\]?[^\n]*\n(.*?)(?=\n##\s|\Z)"
            match = re.search(pattern, content, re.MULTILINE | re.DOTALL)
            if match:
                changelog_section = match.group(1).strip()
        except Exception as e:
            print(f"Warning: Failed to parse CHANGELOG.md: {e}", file=sys.stderr)

    if not changelog_section:
        changelog_section = f"### What's Changed in hyprcross {tag}\n\nSee full changelog in [CHANGELOG.md](https://github.com/runepm/hyprcross/blob/main/CHANGELOG.md)."

    # 3. Define all supported release targets and packages (x86 & ARM for Linux, macOS, and Windows)
    targets = [
        ("Linux (glibc)", "x86_64", "x86_64-unknown-linux-gnu", "tar.gz", "Standard 64-bit Linux (Ubuntu, Debian, Fedora, Arch, CentOS)"),
        ("Linux (musl static)", "x86_64", "x86_64-unknown-linux-musl", "tar.gz", "Statically linked Linux binary (Alpine Linux, scratch containers)"),
        ("Linux ARM64 (glibc)", "aarch64", "aarch64-unknown-linux-gnu", "tar.gz", "64-bit ARM Linux (Raspberry Pi 4/5 64-bit, AWS Graviton, server ARM)"),
        ("Linux ARM64 (musl static)", "aarch64", "aarch64-unknown-linux-musl", "tar.gz", "Statically linked 64-bit ARM Linux (Alpine ARM64)"),
        ("macOS Apple Silicon", "Apple Silicon (M1–M4)", "aarch64-apple-darwin", "tar.gz", "Apple Silicon Macs (arm64) running macOS 11+"),
        ("macOS Intel", "x86_64", "x86_64-apple-darwin", "tar.gz", "Intel-based Macs (x86_64) running macOS 10.12+"),
        ("Windows (64-bit)", "x86_64", "x86_64-pc-windows-gnu", "zip", "64-bit Windows standalone executable (Windows 10/11, Server)"),
        ("Windows (32-bit)", "x86", "i686-pc-windows-gnu", "zip", "32-bit Windows standalone executable (x86)"),
    ]

    table_rows = []
    for platform, arch, triple, ext, desc in targets:
        filename = f"hyprcross-{triple}.{ext}"
        download_url = f"{base_url}/{filename}"
        sha_url = f"{download_url}.sha256"
        table_rows.append(
            f"| **{platform}** | `{triple}` | [**Download {filename}**]({download_url}) | [SHA-256]({sha_url}) | {desc} |"
        )

    table_markdown = "\n".join(table_rows)

    is_prerelease = any(suffix in tag.lower() for suffix in ["-rc", "-beta", "-alpha"])
    cargo_install_version_flag = f" --version {version}" if is_prerelease else ""

    notes = f"""{changelog_section}

---

## How to Upgrade & Install hyprcross

### Option 1: Upgrade via Cargo (Recommended for Rust Developers)

To upgrade your existing installation (or install fresh) with Cargo, use the `--force` flag so Cargo overwrites previous binaries:

```bash
# Upgrade hyprcross and the cargo-hyprcross subcommand plugin:
cargo install --locked hyprcross cargo-hyprcross{cargo_install_version_flag} --force
```

After updating, verify that you are running the new version:

```bash
hyprcross --version
cargo hyprcross --version
```

---

### Option 2: Pre-compiled Binaries (No Rust Toolchain Needed)

Download and extract the ready-to-run binary package for your operating system and architecture directly:

#### Linux & macOS (Shell)
```bash
# 1. Download and extract the archive for your architecture (example: Linux x86_64):
curl -fsSL "{base_url}/hyprcross-x86_64-unknown-linux-gnu.tar.gz" | tar -xz

# 2. Move hyprcross and cargo-hyprcross into your system PATH:
sudo install -m 755 hyprcross cargo-hyprcross /usr/local/bin/

# 3. Verify the upgrade:
hyprcross --version
cargo hyprcross --version
```

#### Windows (PowerShell)
```powershell
# 1. Download the archive for 64-bit Windows:
Invoke-WebRequest -Uri "{base_url}/hyprcross-x86_64-pc-windows-gnu.zip" -OutFile "hyprcross.zip"

# 2. Extract the binaries to your user Cargo bin directory (or any directory in your PATH):
Expand-Archive -Path "hyprcross.zip" -DestinationPath "$HOME\\.cargo\\bin" -Force

# 3. Verify the upgrade:
hyprcross --version
```

---

## Download Binaries

Click any link in the **Direct Download** column to immediately download the pre-compiled archive. Each package contains standalone `hyprcross` and `cargo-hyprcross` binaries.

| Platform | Target Triple | Direct Download Link | Checksum | Description |
| :--- | :--- | :--- | :--- | :--- |
{table_markdown}

> [!TIP]
> To verify an archive's integrity after downloading:
> - **Linux:** `sha256sum -c hyprcross-<target>.tar.gz.sha256`
> - **macOS:** `shasum -a 256 -c hyprcross-<target>.tar.gz.sha256`
> - **Windows (PowerShell):** `Get-FileHash hyprcross-<target>.zip -Algorithm SHA256`
"""

    output_path = sys.argv[1] if len(sys.argv) > 1 else "notes.md"
    with open(output_path, "w", encoding="utf-8") as f:
        f.write(notes)
    print(f"Generated release notes at {output_path}")

if __name__ == "__main__":
    main()
