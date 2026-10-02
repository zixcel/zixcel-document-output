# zixcel-document-output

Extract text and metadata from a PDF, inspect its pages, and write a report; or render Markdown into a PDF.

## What you can do

- Use the Rust API for text extraction, metadata and page analysis.
- Generate a JSON analysis report with a separate write capability.
- Render Markdown through the configured Pandoc and Typst toolchain.

## Current scope

Rust extraction and analysis are implemented. OCR, page editing and PDF merging are outside this release. Reading and report writing use separate capabilities.

Package distribution is not activated by this documentation. Use the checked-in source and the declared dependency versions; published availability must be verified separately.

## Getting started

Install Rust 1.97 or newer and make the declared dependencies available. Use the configured private registry when a dependency is not distributed publicly. Run from this repository:

```sh
cargo test --locked
```

## Inspect a PDF

Provide the relative PDF name and its exact SHA-256 in a request file:

```json
{"input_ref":"input.pdf","input_digest":"<64 lowercase SHA-256 hex characters>","page_index":null}
```

After building the executable, inspect all pages with read permission only:

```sh
zixcel-document-output pdf-analyze request.json --read-root /path/to/inputs
```

To save the report, explicitly supply a separate write capability:

```sh
zixcel-document-output pdf-analyze request.json --read-root /path/to/inputs \
  --output analysis.json --write-root /path/to/reports
```

Reading does not modify the PDF. Report writing refuses an existing destination. Encrypted PDFs are rejected; extraction quality depends on font encodings and Unicode maps. See the interface reference for size, page and decompression limits.

## Documentation and source

[Interface reference](docs/interface-reference.md)

[Usage guide](docs/getting-started.md)

[Examples](examples) · [Implementation and public interfaces](src) · [Verification cases](tests) · [Contributing](CONTRIBUTING.md) · [Security reporting](SECURITY.md) · [License](LICENSE) · [Attribution notices](NOTICE)
