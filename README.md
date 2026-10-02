# zixcel-document-output

A vendor-neutral boundary for PDF inspection, reports, and Markdown-to-PDF rendering. It fixes input digest, classification ceiling, destination, renderer binary digest and time/size bounds before invoking Pandoc and Typst without a shell. 

```bash
cargo run --offline -- doctor
cargo run --offline -- capabilities
cargo run --offline -- validate examples/config.toml
cargo run --offline -- plan examples/config.toml
cargo run --offline -- render examples/config.toml --root /path/to/workspace
```

Input/output paths must be relative. Absolute paths, `..` and symlink boundary crossings are rejected. Replacing output requires `expected_output_digest`. Repeating identical output succeeds, making retries after uncertain state idempotent.

Library use combines `parse_config`, `build_plan` and `render`. Configuration is limited to 1 MiB and 32 nesting levels. Other output formats, template ownership, secrets and external transmission are separate responsibilities. The crate currently uses `publish = false` for local validation.

`semantic/package.sem` defines the sem-lang PDF format and PDF-generation effect. Consumers select byte-identical `package.sem` installed from distributions at `packages/zixcel/document/output` by exact digest instead of scanning this directory. Japanese and English labels follow the same terms in `semantic/language/pdf.sem`; a separate capability document binds the `document/render/pdf` execution contract.

## PDF inspection and reports

The Rust library provides text extraction, Info metadata decoding, page classification, and JSON reports. `PdfReadCapability` authorizes a read root; `PdfWriteCapability` separately authorizes a report destination. Inspection never modifies the source PDF.

```json
{"input_ref":"input.pdf","input_digest":"<64 lowercase SHA-256 hex characters>","page_index":null}
```

```bash
zixcel-document-output pdf-analyze request.json --read-root /path/to/inputs
zixcel-document-output pdf-analyze request.json --read-root /path/to/inputs \
  --output analysis.json --write-root /path/to/reports
```

`page_index` is zero-based; omission or null selects all pages. Without an output argument, the report is emitted to standard output. Saving a report requires the separate write root and refuses to overwrite an existing file. Paths must be relative, confined to their authorized roots, and free of symlinks. Input size is limited to 8 MiB, decompressed stream size to 8 MiB, page count to 1,000, and serialized reports to 64 MiB. Encrypted PDFs are rejected.

This replaces the legacy Python extraction and reporting interface. A synthetic four-page comparison verifies text after whitespace normalization and decoded metadata. Image classification recognizes nested Form XObjects and counts image-only pages independently, correcting the legacy classification and counter defects. OCR, page editing, and merging are outside the inspection interface. Extraction quality depends on the PDF font encodings and Unicode maps.

## Quality gate

Run the package tests in an independent checkout:

```bash
cargo test --locked
cargo fmt --check
```

The workspace verification command is also available to callers:

```bash
# WONDERLAND_ROOT is the workspace checkout root.
"$WONDERLAND_ROOT/bin/verify-repositories" --rust --tier standard
```

## Local templates

Specify both `template_ref` (a root-relative path) and `template_digest` (SHA-256) to use a Pandoc Typst template; omission preserves the default layout. Templates are limited to 1 MiB; path boundaries, symlinks and digest are verified before rendering. Callers own layout and document content; the package contains no project-specific wording. Use trusted self-contained templates; recursive pinning of referenced resources is not implemented.

## Package integration

The package is an independently consumable unit. Callers reference its documented
interface through a versioned dependency and own application-specific composition
and integration.

## License

Apache-2.0. Copyright 2026 HAT Inc. See [LICENSE](LICENSE) and [NOTICE](NOTICE). External dependencies retain their respective licenses.
