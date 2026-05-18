# bm25

[![CI](https://github.com/satoric-tech/bm25-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/satoric-tech/bm25-cli/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/bm25-cli.svg)](https://crates.io/crates/bm25-cli) [![License](https://img.shields.io/badge/license-MIT-007ec6?style=flat-square)](LICENSE)

BM25 search over stdin, a file, a directory, or a URL. No index, no config.

---

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/satoric-tech/bm25-cli/main/install.sh | sh
```

Or via cargo:

```sh
cargo install bm25-cli
```

---

## Usage

**Pipe text** — ranks paragraphs by relevance:

```sh
cat notes.txt | bm25 "deployment rollback"
curl https://example.com/page | bm25 "query" --html
```

**File** — same as pipe, but pass the path directly:

```sh
bm25 "authentication error" ./docs/api.md
```

**Directory** — walks files, returns ranked file paths:

```sh
bm25 "payment handler" ./src
bm25 "auth middleware" ./src --no-ignore
```

**URL** — fetches the page, extracts article content, ranks paragraphs:

```sh
bm25 "default timeout" https://docs.aws.amazon.com/lambda/latest/dg/configuration-timeout.html
```

---

## Options

### Arguments

| Argument | Description |
|---|---|
| `QUERY` | Search query (Lucene syntax supported) |
| `URI` | File path, directory, or URL to search |

### Search

| Flag | Description | Default |
|---|---|---|
| `--tokenizer NAME` | Tokenizer to use: `simple`, `whitespace`, `raw` | `simple` |
| `--filter FILTER,...` | Filters to apply: `stem`, `ascii-fold`, `remove-long` | none |
| `--lang LANG` | Stemmer language, used with `--filter stem` | none |

### Chunking

Applies to stdin, file, and URL modes only.

| Flag | Description | Default |
|---|---|---|
| `--min N` | Skip chunks shorter than N chars | `64` |
| `--max N` | Re-split on `\n` if a chunk exceeds N chars | `2048` |

### Input

| Flag | Description |
|---|---|
| `--html` | Treat piped stdin as HTML — extract article before searching |
| `--no-ignore` | Ignore `.gitignore` / `.ignore` rules (directory mode only) |

### Output

| Flag | Description | Default |
|---|---|---|
| `-m, --max-count N` | Stop after N results | `25` |
| `--json` | Output as JSON lines — `{"score":…,"text":…}` or `{"score":…,"path":…}` | off |

---

## Tokenizers

| Name | Splits on |
|---|---|
| `simple` (default) | Non-alphanumeric characters |
| `whitespace` | Whitespace only |
| `raw` | No splitting — whole input as one token |

---

## Filters

Filters are applied in canonical order: `lowercase` (always) → `ascii-fold` → `remove-long` → `stem`.

| Filter | Description |
|---|---|
| `stem` | Language stemmer via Snowball — use with `--lang` |
| `ascii-fold` | Fold unicode to ASCII: `é→e`, `ü→u`, etc. |
| `remove-long` | Drop tokens longer than 40 chars |

Examples:

```sh
bm25 "authentication" ./src --filter stem --lang english
bm25 "café résumé" ./docs --filter stem,ascii-fold --lang french
bm25 "UUID" ./src --filter remove-long
```

---

## Languages

Used with `--filter stem --lang LANG`:

`arabic` `danish` `dutch` `english` `finnish` `french` `german` `greek` `hungarian` `italian` `norwegian` `portuguese` `romanian` `russian` `spanish` `swedish` `tamil` `turkish`

---

## Query syntax

| Syntax | Meaning |
|---|---|
| `payment invoice` | Either term |
| `+payment +invoice` | Both required |
| `payment -invoice` | payment but not invoice |
| `"payment handler"` | Exact phrase |
| `"payment handler"~2` | Phrase with slop |
| `pay*` | Prefix match |
| `payment^2 invoice^0.5` | Boost term weight |

---

## License

MIT — built by [Satoric, Inc.](https://satoric.com)
