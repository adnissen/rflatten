```
RFLATTEN(1)                      User Commands                     RFLATTEN(1)

NAME
       rflatten - flatten subdirectories by moving all files to the root
       directory

SYNOPSIS
       rflatten [OPTIONS] <DIRECTORY>

DESCRIPTION
       rflatten recursively moves all files from subdirectories to the root
       directory, effectively flattening the directory structure. Files
       already in the root directory are not moved. Empty subdirectories are
       removed after flattening.

       If filename conflicts occur, files are automatically renamed with a
       numeric suffix (e.g., file_1.txt, file_2.txt).

OPTIONS
       <DIRECTORY>
              Directory to flatten (required)

       -n, --depth <MAX_DEPTH>
              Maximum depth to traverse. By default, all subdirectory levels
              are processed. Use this to limit how deep into the directory
              structure the tool will traverse.

       -y, --yes
              Skip confirmation prompt and proceed immediately with the
              flatten operation.

       -i, --include <INCLUDE>
              Include only directories that fuzzy-match these patterns.
              Accepts comma-separated values. Uses case-insensitive substring
              matching. Cannot be used together with --exclude.

       -e, --exclude <EXCLUDE>
              Exclude directories that fuzzy-match these patterns. Accepts
              comma-separated values. Uses case-insensitive substring
              matching. Cannot be used together with --include.

       -h, --help
              Print help information

EXAMPLES
       Basic usage:
              rflatten /path/to/directory

       Skip confirmation prompt:
              rflatten -y /path/to/directory

       Only flatten first level subdirectories:
              rflatten --depth 1 /path/to/directory

       Flatten up to 2 levels deep:
              rflatten -n 2 /path/to/directory

       Only flatten the "src" directory:
              rflatten --include src /path/to/directory

       Flatten multiple specific directories:
              rflatten -i src,tests /path/to/directory

       Fuzzy match (flatten both "docs" and "documentation"):
              rflatten --include doc /path/to/directory

       Exclude the "src" directory:
              rflatten --exclude src /path/to/directory

       Exclude multiple directories:
              rflatten -e src,tests /path/to/directory

       Combined options:
              rflatten -n 2 -e tests -y /path/to/directory

PATTERN MATCHING
       The --include and --exclude options use case-insensitive fuzzy
       (substring) matching. For example:

       • "doc" matches "docs", "documentation", "DOCS", etc.
       • "test" matches "tests", "testing", "test_files", etc.

       Patterns are matched against top-level directory names only.

EXIT STATUS
       0      Successful operation
       1      Error occurred (invalid directory, permission denied, etc.)

AUTHOR
       Written by Andrew Nissen

rflatten 0.1.0                    2025-11-05                       RFLATTEN(1)
```