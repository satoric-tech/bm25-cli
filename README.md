# BM25 CLI

[![CI](https://github.com/satoric-tech/bm25-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/satoric-tech/bm25-cli/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/bm25-cli.svg)](https://crates.io/crates/bm25-cli) [![License](https://img.shields.io/badge/license-MIT-007ec6?style=flat-square)](LICENSE)

Fast BM25 full-text search over local files. Zero config, auto-indexed, gitignore-aware.

---

## Installation

```console
$ curl -fsSL https://raw.githubusercontent.com/satoric-tech/bm25-cli/main/install.sh | sh
```

Or via cargo:

```console
$ cargo install bm25-cli
```

---

## Commands

### `bm25 <query> [paths...] [options]`

Search one or more local paths or globs.

```console
$ bm25 "auth error handling" ./src              # directory
$ bm25 "schema data" "**/*.md" --context 300    # glob
$ bm25 "payment status" ./src ./docs            # multiple paths
```

**Options**

| Flag | Description | Default |
|------|-------------|---------|
| `--force` | Re-index the given sources | off |
| `--score` | Show relevance scores | off |
| `--json` | Output as JSON lines `{path, score, context?}` | off |
| `--no-ignore` | Do not respect `.gitignore` / `.ignore` rules | off |
| `--pagerank` | Re-rank results using Personalized PageRank (import graph) | off |
| `-l, --limit <N>` | Maximum number of results | `25` |
| `-c, --context <CHARS>` | Show a highlighted excerpt around matches | off |
| `-f, --fuzzy <DISTANCE>` | Fuzzy match at edit distance 1 or 2 | off |
| `-s, --since <WHEN>` | Only files modified within window (`7d`, `2w`, `2024-01-01`) | off |
| `-m, --max-filesize <SIZE>` | Skip files larger than this (`1M`, `500K`, `2G`) | off |
| `-j, --jobs <N>` | Number of threads (-1 for all CPUs) | `-1` |

> **Note**: Paths are indexed on first query and re-indexed automatically on subsequent queries.

---

### `bm25 sync`

Index sources for the first time or re-index existing ones.

```console
$ bm25 sync ./src
$ bm25 sync ./src ./docs
$ bm25 sync --all
```

**Options**

| Flag | Description | Default |
|------|-------------|---------|
| `--all` | Re-index all registered sources | off |
| `--no-ignore` | Do not respect `.gitignore` / `.ignore` rules | off |
| `-m, --max-filesize <SIZE>` | Skip files larger than this (`1M`, `500K`, `2G`) | off |
| `-j, --jobs <N>` | Number of threads (-1 for all CPUs) | `-1` |

---

### `bm25 list`

List all registered sources and when they were last indexed.

```console
$ bm25 list
/home/user/project/src
  42 docs
  added 5 days ago
  last synced just now
```

---

### `bm25 remove <path>`

Remove a source and purge all its documents from the index.

```console
$ bm25 remove /home/user/project/src
```

---

## Sources

Sources are registered and stored in `~/.bm25/`.

| Format | Example | Notes |
|--------|---------|-------|
| Directory | `.`, `./src`, `/home/user/project` | Respects `.gitignore`. Skips binaries. Non-UTF-8 files transcoded transparently. |
| Glob | `"**/*.md"`, `"src/**/*.py"` | Same file rules as directory. |

---

## Query syntax

| Syntax | Meaning |
|--------|---------|
| `payment invoice` | Either term (OR) |
| `+payment +invoice` | Both terms required (AND) |
| `payment AND invoice` | Both terms required |
| `payment OR invoice` | Either term (explicit) |
| `payment -invoice` | payment but not invoice |
| `"payment handling"` | Exact phrase |
| `"payment handling"~2` | Phrase with slop (terms within 2 positions) |
| `pay*` | Prefix match |
| `payment^2.0 invoice^0.4` | Boost term relevance |
| `*` | Match all documents |

---

## License

MIT — built by [Satoric, Inc.](https://satoric.com)
