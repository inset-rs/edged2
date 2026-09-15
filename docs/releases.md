# Releases

Version tags such as `v0.1.0` trigger `.github/workflows/release.yml`. The tag must match `workspace.package.version` in Cargo.toml. Update Cargo.lock with the version change and commit both before tagging.

The workflow builds on an Apple Silicon macOS runner, signs and notarizes through Inset CLI, verifies the signature and stapled ticket, and publishes a ZIP plus SHA-256 checksums to GitHub Releases. Tags containing a prerelease suffix produce a prerelease. It does not publish Rust crates.

## Repository secrets

Add these under **Settings → Secrets and variables → Actions → Repository secrets**:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application certificate exported as a password-protected .p12, including its private key |
| `APPLE_CERTIFICATE_PASSWORD` | Password chosen when exporting that .p12 |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Jianying Li (G83H824X6L)` |
| `APPLE_ID` | Apple account used for notarization |
| `APPLE_PASSWORD` | An Apple app-specific password for that account |
| `APPLE_TEAM_ID` | `G83H824X6L` |

In Keychain Access, export the Developer ID Application identity with its private key as a .p12. Encode that file and copy the value without printing it:

```sh
base64 -i /path/to/DeveloperID.p12 | tr -d '\n' | pbcopy
```

Paste it into `APPLE_CERTIFICATE`. Do not commit the .p12, its encoded content, or any passwords. The certificate export password and Apple app-specific password are different credentials.

GitHub supplies `GITHUB_TOKEN` automatically; the workflow grants it permission to create the release. No personal access token is needed.

## Triggering a release

After the workflow and desired version are committed and pushed:

```sh
git tag v0.1.0
git push origin v0.1.0
```

Use the version recorded in Cargo.toml. A tag mismatch fails before signing. Check the Actions run for notarization failures; an unsigned or unstapled app is never uploaded.

The asset is named `Edged-2-v0.1.0-macOS-arm64.zip` for that example. The website's /download resolver recognizes the macOS ZIP. There is currently no Intel or universal build in this workflow.
