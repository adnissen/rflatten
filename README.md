`rflatten` is a cross-platform program which recursively moves all files from subdirectories to the root directory, effectively flattening the directory structure. Files already in the root directory are not moved. Empty subdirectories are removed after flattening.

If filename conflicts occur, files are automatically renamed with a numeric suffix (e.g., file_1.txt, file_2.txt).

By default, a confirmation prompt is shown with the number of files that will be moved.

```
cargo install rflatten
```

```
rflatten [OPTIONS] <DIRECTORY>
```

## Options

| Option | Description |
|--------|-------------|
| `<DIRECTORY>` | Directory to flatten (required) |
| `-n, --depth <MAX_DEPTH>` | Maximum depth to traverse. By default, all subdirectory levels are processed. |
| `-y, --yes` | Skip confirmation prompt and proceed immediately. |
| `-q, --quiet` | Quiet mode - suppress all output except errors. Confirmation prompt is automatically skipped. |
| `-i, --include <INCLUDE>` | Include only directories that begin with any of these values. Comma-separated, case-insensitive prefix matching. Cannot be used with --exclude. |
| `-e, --exclude <EXCLUDE>` | Exclude directories that begin with any of these values. Comma-separated, case-insensitive prefix matching. Cannot be used with --include. |
| `-h, --help` | Print help information |
| `-V, --version` | Print version information |

## Examples

```bash
# Basic usage
rflatten /path/to/directory

# Skip confirmation prompt
rflatten -y /path/to/directory

# Quiet mode (no output except errors)
rflatten -q /path/to/directory

# Only flatten first level subdirectories
rflatten --depth 1 /path/to/directory

# Flatten up to 2 levels deep
rflatten -n 2 /path/to/directory

# Only flatten the "src" directory
rflatten --include src /path/to/directory

# Flatten multiple specific directories
rflatten -i src,tests /path/to/directory

# Fuzzy match (flatten both "docs" and "documentation")
rflatten --include doc /path/to/directory

# Exclude the "src" directory
rflatten --exclude src /path/to/directory

# Exclude multiple directories
rflatten -e src,tests /path/to/directory

# Combined options
rflatten -n 2 -e tests -y /path/to/directory
```

## Pattern Matching

The `--include` and `--exclude` options use case-insensitive prefix matching:

- `doc` matches `docs`, `documentation`, `DOCS`, etc.
- `test` matches `tests`, `testing`, `test_files`, etc.

Patterns are matched against top-level directory names only.
