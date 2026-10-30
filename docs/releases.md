# Publishing a release

The [Release workflow](../.github/workflows/release.yml) builds native packages
and uploads them to [GitHub Releases](https://github.com/hakanngul/RustLoadTesting/releases).
It runs when a version tag is pushed. You can also run it manually for an
existing tag from **Actions > Release > Run workflow**.

## Packages

| Platform | Runner | Download |
| --- | --- | --- |
| Linux x64 | Ubuntu 22.04 | `RustLoad-v<VERSION>-linux-x86_64.tar.gz` |
| Windows x64 | Windows Server 2022 | `RustLoad-v<VERSION>-windows-x86_64.zip` |
| macOS Apple Silicon | macOS 15, ARM64 | `RustLoad-v<VERSION>-macos-arm64.dmg` |
| macOS Intel | macOS 15, x64 | `RustLoad-v<VERSION>-macos-x86_64.dmg` |

Every release includes `SHA256SUMS.txt`. Linux packages need a graphical session,
OpenGL, and X11/Wayland with xkbcommon runtime libraries. They are built against
Ubuntu 22.04's glibc; older distributions are not supported by these packages.
The macOS deployment target is 12.0. Windows packages use a static MSVC runtime.
Linux ARM64, Windows ARM64, and 32-bit builds are not part of this matrix.

The macOS DMG contains RustLoad.app and an Applications shortcut. The app has an
ad-hoc signature, not an Apple Developer ID signature or notarization. macOS may
block its first launch. After reviewing the download, use **System Settings >
Privacy & Security > Open Anyway** if needed. Windows packages have no publisher
certificate and may trigger a SmartScreen warning. Signing and notarization need
their own credentials and are not configured here.

## Create a version

1. Set the release version in `Cargo.toml`. Run `cargo check --locked` to check
   it. If Cargo reports that the lockfile needs updating, run `cargo check` and
   include the updated `Cargo.lock` in the version commit. Keep the pinned
   toolchain in `rust-toolchain.toml` and the workflows consistent.
2. Run the format, Clippy, test, and build commands in the README. Complete native
   UI acceptance on the platforms you intend to support; a successful build does
   not establish that the desktop works correctly on every platform.
3. Commit and push the version change to `main`.
4. Create and push the matching tag. For the current `0.1.0` package:

   ```sh
   git tag -a v0.1.0 -m "RustLoad v0.1.0"
   git push origin v0.1.0
   ```

5. Follow the **Release** run in GitHub Actions. When all four builds pass, the
   release and its downloads appear in the repository's Releases section.

The tag must match the Cargo package version exactly, with a leading `v`.
For example, Cargo version `0.2.0-rc.1` uses tag `v0.2.0-rc.1`; the workflow
marks that release as a prerelease. Build-metadata tags containing `+` are not
accepted. Do not move or reuse a tag to distribute a different source version.

## What the workflow checks

The prepare job validates the tag and its checked-out `Cargo.toml`. Each native
runner checks formatting, runs strict all-target Clippy and the test suite, and
builds a release binary for its target. macOS packaging validates the app's plist
and ad-hoc signature, then creates and verifies the DMG.

The publish job runs only after every build succeeds. It checks that exactly the
four expected packages exist, calculates SHA-256 checksums, and creates the GitHub
Release with installation instructions and generated change notes. Build jobs
have read-only repository permissions; only the publish job has `contents: write`.
The automatic `GITHUB_TOKEN` is sufficient; no personal access token is needed.

Rerun a failed job from Actions, or dispatch the workflow manually with the same
existing tag. If that release already exists, its matching assets are replaced;
its notes and draft/prerelease settings are left as they are. A failed platform
build prevents publication; already-uploaded Actions artifacts expire after seven
days. Publishing to GitHub Releases is separate from those temporary artifacts.

## Verify a download

On Linux, place the packages and checksum file in one directory and run:

```sh
sha256sum --check SHA256SUMS.txt
```

On macOS, use `shasum -a 256 <download.dmg>`. On Windows, use
`Get-FileHash .\<download.zip> -Algorithm SHA256` in PowerShell. Compare the value
with the corresponding line in `SHA256SUMS.txt`.

For local macOS packaging, run `bash scripts/package-macos.sh`, or pass
`aarch64-apple-darwin` / `x86_64-apple-darwin` as the optional first argument.
This builds an ad-hoc signed `dist/RustLoad.app`. The script reads its version from
Cargo metadata. DMG creation is handled by the workflow.
