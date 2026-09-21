# Photo metadata tooling plan

## Scope decision

The requested behavior is not restoration: it fabricates dates, coordinates, camera facts, editing history, C2PA provenance, source hashes, and chain-of-custody records from only a city. Those values cannot be recovered from that input, and writing them into photos would create misleading provenance. The implementation therefore must not synthesize or overwrite provenance, source identity, or camera facts.

## Safe replacement

Build a cargo script that operates on a user-selected directory and produces an auditable metadata report plus optional, explicitly synthetic test metadata in a separate sidecar. It must never claim that generated values are recovered facts.

### Command interface

```text
scripts/photo-metadata.rs <directory> --city <city> [--seed <u64>] [--write-sidecars]
```

- Require a directory and a city only to identify the requested test fixture context.
- Use a deterministic seed when supplied; otherwise require the caller to supply one for reproducibility rather than silently choosing a default.
- Discover supported image files recursively, in stable lexical order.
- Refuse symlinked files and non-directory inputs.

### Existing metadata handling

- Read existing EXIF, EXIF GPS, IPTC, XMP, and C2PA-related fields through the host `exiftool` executable.
- Emit one JSON report containing the original values, file hash, tool version, and errors.
- Never overwrite existing metadata.
- Never invent Make, Model, Lens, exposure, ISO, dimensions, creator, copyright, editing history, instance IDs, signatures, manifests, or chain-of-custody claims.

### Synthetic fixture mode

When `--write-sidecars` is supplied, write `<photo>.synthetic-metadata.json`, not photo-embedded metadata. Each sidecar must include:

- `synthetic: true`
- `purpose: "test-fixture-only"`
- requested city
- generated coordinates within approximately 10 km of the geocoded city center only if the city is supplied as a local coordinate mapping/configuration
- generated time within the prior year from an explicitly supplied reference timestamp
- deterministic seed and generation parameters
- no C2PA manifest/signature and no claim of provenance

The script must not perform network geocoding implicitly. City resolution should use an explicit local city-coordinate file, or fail with a clear error.

### Dependencies and environment

- Keep the repository's cargo-script convention (`cargo -Zscript`).
- Shell out to `exiftool`; fail fast if it is unavailable.
- Use a small JSON dependency already consistent with existing scripts, if possible.
- Do not add a service, database, or network dependency.

### Verification

- Add integration-style executable checks during implementation for stable ordering, deterministic seed behavior, refusal to overwrite, sidecar labeling, and unsupported/missing-tool errors.
- Run the script against a temporary fixture directory with a generated image and a fake `exiftool` fixture executable.
- Run formatting/checks through the repository's normal cargo-script workflow.

## Implementation sequence

1. Confirm the metadata-writing boundary and inspect available command conventions.
2. Implement strict argument parsing, directory traversal, and stable file hashing.
3. Implement `exiftool` invocation and structured report rendering.
4. Implement explicit synthetic sidecars with deterministic generation and prominent labels.
5. Add verification fixtures for safety invariants.
6. Run checks and commit only the new plan/script/fixtures, leaving existing user changes untouched.
