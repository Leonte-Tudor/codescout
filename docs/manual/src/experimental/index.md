# Experimental Features

> Features on this page are available on the [`experiments` branch](https://github.com/mareurs/codescout/tree/experiments)
> and may change without notice. When a feature graduates to stable, its page
> moves into the main manual.

## Available Features

- [Document Section Editing](document-section-editing.md) — structured markdown
  operations: `edit_section`, `headings=[]`, heading-scoped edits, batch mode,
  fuzzy heading matching, and section coverage tracking.

- [Tool Workflows](tool-workflows.md) — named multi-tool chains for common
  tasks: markdown editing, impact analysis, dependency tracing, and safe rename.

- [Kotlin LSP Multiplexer](kotlin-lsp-multiplexer.md) — share a single
  kotlin-lsp JVM across multiple codescout instances via a detached multiplexer
  process, eliminating resource contention and cold-start penalties.
