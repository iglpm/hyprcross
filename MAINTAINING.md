# Maintaining hyprcross

How the `runepm/hyprcross` repository has to be set up on GitHub for its workflows to work:
CI, the per-target images on GHCR, the weekly image updates, releases and the wiki. Most of it
is one-time setup; [After setup](#after-setup) lists what recurs.

## The workflows, and what each needs

| Workflow | Runs on | Needs |
| :--- | :--- | :--- |
| [CI](.github/workflows/ci.yml) | pushes and pull requests to `main` | hyprlnk readable (below) |
| [Images](.github/workflows/images.yml) | changes to `docker/`, every Monday, `v*` tags, by hand | hyprlnk readable; packages that may be public |
| [Image updates](.github/workflows/images-update.yml) | every Monday, by hand | `HYPRCROSS_BOT_TOKEN` to test and merge its pull requests |
| [Release](.github/workflows/release.yml) | `v*` tags | hyprlnk readable |
| [Sync Docs to Wiki](.github/workflows/wiki-sync.yml) | changes to `docs/` and `README.md` | a public repository with an initialised wiki |
| Dependabot | weekly (Cargo, the hyprlnk submodule), monthly (Actions) | hyprlnk readable, for the submodule and its pull requests' CI |

## 1. Decide what is public

hyprcross is built against [hyprlnk](https://github.com/runepm/hyprlnk), a submodule at
`hyprlnk/` pinned to one commit (its URL, `../hyprlnk.git`, is relative: SSH for SSH clones,
HTTPS for HTTPS ones), and installs its images from `ghcr.io/runepm/hyprcross/<target>`
without logging in. So, for anyone outside the organisation to use it:

- **hyprcross and hyprlnk public.** Building hyprcross from source (`git clone --recursive`,
  the README's install) needs both, and release downloads need hyprcross public.
- **The image packages public.** This is separate from the repository's visibility (step 4).

While the repositories stay private, the steps below still make every workflow work for the
organisation, with two costs to know about: Actions minutes are metered (macOS minutes count
10×, Windows 2×, so one CI run is roughly 100 billable minutes), and the wiki sync skips
itself. Linux arm64 runners (`ubuntu-24.04-arm`, used for the aarch64 Linux release) may also
be unavailable or billed on private repositories, depending on the plan.

## 2. Secrets

Settings → Secrets and variables → Actions → New repository secret.

**`HYPRLNK_TOKEN`**: needed while hyprlnk is private. A workflow's own `GITHUB_TOKEN` reads
only its own repository, so fetching the submodule fails without it. The workflows check out
hyprcross and its submodule with this one token, so it must read both. Create a fine-grained
personal access token (Settings → Developer settings → Fine-grained tokens):

- Resource owner: `runepm`. Repository access: only `runepm/hyprcross` and `runepm/hyprlnk`.
- Permissions: Contents → Read-only.
- The organisation may have to allow fine-grained tokens, or approve this one (Organisation
  settings → Personal access tokens).

Add it **twice**: as an Actions secret, and as a Dependabot secret (Secrets and variables →
Dependabot), since workflows started by Dependabot's pull requests only see the latter. Once
hyprlnk is public, delete both: the workflows fall back to `GITHUB_TOKEN`.

**`HYPRCROSS_BOT_TOKEN`**: optional, for the Image updates workflow. Pull requests and pushes
made with `GITHUB_TOKEN` start no workflows, so without this token the weekly "update image
tools" pull request sits with no checks until a person closes and reopens it (which starts
them) and merges it. With it, the workflow waits for the pull request's checks and merges it
when all pass. Use a fine-grained token of a bot account (or your own) on `runepm/hyprcross`
with Contents and Pull requests: Read and write. Note its expiry date somewhere.

`RUNIFIED_TOKEN` is optional: the wiki sync uses it when set, and `GITHUB_TOKEN` otherwise.

**Dependabot and the private submodule:** for its weekly hyprlnk pull request Dependabot must
read hyprlnk itself. Organisation settings → Code security → Global settings → "Grant
Dependabot access to private repositories": add `runepm/hyprlnk`.

## 3. Repository settings

**Settings → General**

- Default branch: `main` (the Images workflow publishes only from it).
- Pull Requests: allow squash merging (the image update bot merges with `--squash`);
  "Automatically delete head branches" is convenient.
- Features → Wikis: on, if you want the docs mirrored there (step 6).

**Settings → Actions → General**

- Actions permissions: allow the actions the workflows use: `actions/*`, `docker/*`,
  `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `peter-evans/create-pull-request`,
  `softprops/action-gh-release`, or allow all.
- Workflow permissions: the default ("Read repository contents") is right. Each workflow asks
  for what it needs (`packages: write`, `contents: write`, `pull-requests: write`).
- "Allow GitHub Actions to create and approve pull requests": turn on if you run without
  `HYPRCROSS_BOT_TOKEN`; the image update pull request is then opened with `GITHUB_TOKEN`. If
  the option is greyed out, the organisation's Actions settings disable it.

**Settings → Rules → Rulesets → New branch ruleset** (for `main`, optional but recommended)

- Require a pull request before merging.
- Require status checks: `Test (ubuntu-24.04)`, `Test (macos-latest)`,
  `Test (windows-latest)`, `Image scripts`. These run on every pull request. Don't require the
  Images jobs: they run only when `docker/` changes, and a required check that never reports
  blocks the pull request.
- If you require reviews, add the account behind `HYPRCROSS_BOT_TOKEN` to the bypass list, or
  the image update pull requests will wait for a review.

## 4. Container packages (GHCR)

The Images workflow pushes `ghcr.io/runepm/hyprcross/<target>` (30 packages) with
`GITHUB_TOKEN`. Each image's `org.opencontainers.image.source` label links its package to the
repository, which gives the workflow write access to it.

1. **Before the first publish:** Organisation settings → Packages → Package creation: allow
   **Public** packages.
2. **After the first run** of the Images workflow: open each package (Organisation → Packages,
   or the repository's sidebar), then Package settings → Danger zone → Change visibility →
   Public. Do this per package in the web UI. New packages may come out private even from a
   public repository.
3. Check from a machine that isn't logged in to GHCR:

   ```sh
   curl -s -o /dev/null -w "%{http_code}\n" \
     "https://ghcr.io/token?scope=repository:runepm/hyprcross/aarch64-unknown-linux-gnu:pull"
   # 200: public. 403: private (or not published yet).
   ```

   While a package is private, hyprcross falls back to cross-rs' image for that target and
   says so (`note: using cross-rs' image for this target instead`).

Besides the images, each package holds a `buildcache` tag (the registry layer cache).
Keep it: deleting it makes the next run rebuild the musl toolchains from source.

## 5. First publish, in order

The three repositories depend on each other, so push them in this order:

1. **hyprlnk** first: commit its fixes (in Rune's `hyprlnk/`) and push. The image smoke tests
   for ARM glibc and dynamically linked musl need them (`.tbss` layout, `.eh_frame` symbols).
2. **Move hyprcross' submodule to that commit**: it is pinned to `821a194`, from before them.

   ```sh
   cd hyprcross/hyprlnk && git fetch && git checkout origin/main && cd ..
   git add hyprlnk
   ```

3. **hyprcross**. The push starts CI and, since it adds `docker/`, the Images workflow for all
   30 images. The first Images run is the slow one: seven musl toolchains are built from
   source (10–15 minutes each, in parallel); later runs reuse them from the registry cache.
   Then do step 4.2 and 4.3.
4. **rune**: its changes (`rune-cas`, `rune-common`, `rune-scheduler`; hyprcross moved from
   its workspace's members to `exclude`) and the new submodule commits of `hyprlnk` and
   `hyprcross`.

Then check:

```sh
hyprcross images pull -t aarch64-unknown-linux-gnu   # pulls from ghcr.io/runepm/hyprcross
hyprcross targets                                    # every target with an image, and how it builds here
```

## 6. Wiki

The sync workflow copies `README.md` (as the wiki's Home) and `docs/*.md` to the wiki on each
push that changes them. It skips itself while the repository is private, and until the wiki
exists: create its first page by hand once (Wiki → Create the first page → Save). After that,
edit the docs in the repository, not in the wiki: the next sync overwrites wiki edits.

## 7. Releases

1. Set `version` in `Cargo.toml`, `crates/cargo-hyprcross/Cargo.toml` (and `Cargo.lock`,
   with `cargo check`) and document the changes in `CHANGELOG.md`.
2. Tag and push: `git tag v2027.0.0-rc.1 && git push origin v2027.0.0-rc.1`.

The Release workflow cross-compiles `hyprcross` and `cargo-hyprcross` across the full matrix
(Linux glibc/musl x86-64/aarch64/armv7/riscv64, macOS aarch64/x86-64, Windows MSVC/GNU, FreeBSD)
using hyprcross itself on self-hosted Linux runners. It automatically detects pre-release tags
(`-rc.x`, `-beta`, `-alpha`) to publish beta GitHub Releases, packages checksummed archives,
and publishes both `hyprcross` and `cargo-hyprcross` to crates.io using `CRATESIO_TOKEN`.


## After setup

- **Mondays:** the Images workflow refreshes every image with the week's Ubuntu updates and
  pushes it as `latest` and `<date>`. Image updates opens a pull request when wasi-sdk,
  Wasmtime or musl-cross-make have new releases (and merges it, with the bot token).
- **By hand**, deliberately: the Android NDK and FreeBSD versions (`docker/scripts/`) and the
  Ubuntu base (`FROM ubuntu:24.04` in every Dockerfile; a newer one raises the glibc that
  programs built in the images need, which [docs/targets.md](docs/targets.md) states).
- **Moving hyprlnk:** Dependabot opens a pull request when hyprlnk's `main` has new commits
  (or by hand: check the commit out in `hyprlnk/`, then `git add hyprlnk`). Such a pull
  request runs CI and every image's smoke test, since hyprlnk links them. If hyprlnk's own
  dependencies changed, CI's `--locked` build fails until `Cargo.lock` follows: run `cargo
  update -p hyprlnk` and push the result to the pull request's branch.
- **Rebuilding images by hand:** Actions → Images → Run workflow, with targets separated by
  spaces, or empty for all.
- **A failed image:** the job's log shows the image build and then the smoke test (hyprcross
  building, and where it can, testing `docker/test/smoke` in it). [docker/README.md](docker/README.md)
  shows how to build and test the same image locally.
