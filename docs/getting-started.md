# Using zixcel-document-output

Extract text and metadata from a PDF, inspect its pages, and write a report; or render Markdown into a PDF.

## Before you start

Rust extraction and analysis are implemented. OCR, page editing and PDF merging are outside this release. Reading and report writing use separate capabilities.

## First steps

Run from the repository root:

```sh
cargo test --locked
```

## How to assess the result

- Use the Rust API for text extraction, metadata and page analysis.
- Generate a JSON analysis report with a separate write capability.
- Render Markdown through the configured Pandoc and Typst toolchain.

A passing source-level check establishes only what that check observes. Keep missing configuration, unavailable services and unverified deployment paths visible.

## Continue reading

[Repository overview](../README.md)
