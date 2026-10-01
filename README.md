# zixcel-document-output

A vendor-neutral boundary converting Markdown to verified local PDFs. It fixes input digest, classification ceiling, destination, renderer binary digest and time/size bounds before invoking Pandoc and Typst without a shell. It does not handle networking, authentication, submission or distribution.

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

## Quality gate

These checks run entirely within this crate without generating documents or external files.

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
