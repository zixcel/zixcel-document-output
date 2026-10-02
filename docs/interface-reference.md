# zixcel-document-output interface reference

Use the [usage guide](getting-started.md) for the first steps. This reference preserves the current interface details and operational limits. Run command examples from the repository root, after preparing the exact declared dependencies and registered configuration.

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

## Local templates

Specify both `template_ref` (a root-relative path) and `template_digest` (SHA-256) to use a Pandoc Typst template; omission preserves the default layout. Templates are limited to 1 MiB; path boundaries, symlinks and digest are verified before rendering. Callers own layout and document content; the package contains no project-specific wording. Use trusted self-contained templates; recursive pinning of referenced resources is not implemented.
