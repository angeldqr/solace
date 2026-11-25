# Code signing

Solace's Windows installers are signed through **SignPath Foundation**, which gives free code
signing to open source projects. Until the project is approved and the steps below are done, the
release workflow publishes unsigned builds, exactly as before.

## How a signed release is built

`.github/workflows/release.yml`, on a `vX.Y.Z` tag:

1. It runs the tests, then builds the app without bundling it (`tauri build --no-bundle`).
2. **Signing 1:** `Solace.exe` and the `solace.exe` CLI go to SignPath and come back signed.
3. `tauri bundle` packs the signed programs into the NSIS and MSI installers and writes the updater signatures.
4. **Signing 2:** the installers go to SignPath, come back signed, and their updater signatures are recomputed.
5. `scripts/release-manifest.mjs` writes `latest.json`, and everything is attached to a draft GitHub release.

Signing runs only when the repository variable `SIGNPATH_ORGANIZATION_ID` is set.

## One-time setup (project owner)

1. **Turn on two-factor authentication** on GitHub. SignPath requires it for every team member.
2. **Apply** at https://signpath.org (Open Source program) with the repository URL.
3. When approved, in SignPath:
   - Create the project `solace`, linked to this GitHub repository as a trusted build system.
   - Create a signing policy `release-signing`, with manual approval by the project owner.
   - Create two artifact configurations, `binaries` and `installers`, from the XML below.
   - Create a CI user and copy its API token.
4. In GitHub, under **Settings > Secrets and variables > Actions**:
   - secret `SIGNPATH_API_TOKEN`: the API token
   - variable `SIGNPATH_ORGANIZATION_ID`: your organization id
5. Push a new version tag. Each signing request waits for your approval in SignPath.

### Artifact configuration `binaries`

```xml
<?xml version="1.0" encoding="utf-8"?>
<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
  <parameters>
    <parameter name="version" required="true" />
  </parameters>
  <zip-file>
    <pe-file path="Solace.exe" product-name="Solace" product-version="${version}">
      <authenticode-sign />
    </pe-file>
    <pe-file path="solace.exe" product-name="Solace" product-version="${version}">
      <authenticode-sign />
    </pe-file>
  </zip-file>
</artifact-configuration>
```

### Artifact configuration `installers`

```xml
<?xml version="1.0" encoding="utf-8"?>
<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
  <zip-file>
    <pe-file path="*-setup.exe">
      <authenticode-sign />
    </pe-file>
    <msi-file path="*.msi">
      <authenticode-sign />
    </msi-file>
  </zip-file>
</artifact-configuration>
```

Check both against SignPath's documentation when you create them. Its editor validates the XML.
