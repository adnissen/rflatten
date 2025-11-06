use clap::Parser;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Helper function to display paths without Windows UNC prefix (\\?\)
fn display_path(path: &Path) -> String {
    let path_str = path.display().to_string();

    // Strip the Windows UNC prefix if present
    #[cfg(target_os = "windows")]
    {
        if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            return stripped.to_string();
        }
    }

    path_str
}

#[derive(Parser)]
#[command(name = "rflatten")]
#[command(about = "Flatten subdirectories by moving all files to the root directory", long_about = None)]
#[command(arg_required_else_help = true)]
struct Cli {
    /// Directory to flatten
    #[arg(required = true)]
    directory: PathBuf,

    /// Maximum depth to traverse (default: unlimited)
    #[arg(short = 'n', long = "depth")]
    max_depth: Option<usize>,

    /// Skip confirmation prompt
    #[arg(short = 'y', long = "yes")]
    skip_confirmation: bool,

    /// Quiet mode - suppress all output except errors
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Include only directories that start with these patterns (comma-separated)
    #[arg(short = 'i', long = "include", value_delimiter = ',')]
    include: Option<Vec<String>>,

    /// Exclude directories that start with these patterns (comma-separated)
    #[arg(short = 'e', long = "exclude", value_delimiter = ',')]
    exclude: Option<Vec<String>>,
}

/// Summary of files to be flattened
struct FileSummary {
    file_count: usize,
    top_level_dirs: std::collections::HashSet<String>,
}

/// Prefix match: checks if the target starts with the pattern (case-insensitive)
fn starts_with_pattern(target: &str, pattern: &str) -> bool {
    target.to_lowercase().starts_with(&pattern.to_lowercase())
}

/// Check if a top-level directory should be included based on include/exclude patterns
fn should_include_top_level_dir(
    dir_name: &str,
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
) -> bool {
    // Check include patterns
    if let Some(include_patterns) = include {
        return include_patterns.iter().any(|p| starts_with_pattern(dir_name, p));
    }

    // Check exclude patterns
    if let Some(exclude_patterns) = exclude {
        return !exclude_patterns.iter().any(|p| starts_with_pattern(dir_name, p));
    }

    // No filters, include everything
    true
}

/// Collect summary of files
fn collect_file_summary(
    dir: &Path,
    max_depth: Option<usize>,
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
) -> io::Result<FileSummary> {
    let mut summary = FileSummary {
        file_count: 0,
        top_level_dirs: std::collections::HashSet::new(),
    };

    collect_file_summary_recursive(
        dir,
        dir,
        max_depth,
        0,
        include,
        exclude,
        &mut summary,
        None,
    )?;

    Ok(summary)
}

fn collect_file_summary_recursive(
    root: &Path,
    current: &Path,
    max_depth: Option<usize>,
    current_depth: usize,
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
    summary: &mut FileSummary,
    top_level_dir: Option<String>,
) -> io::Result<()> {
    if let Some(max) = max_depth {
        if current_depth > max {
            return Ok(());
        }
    }

    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            // Determine the top-level directory name
            let new_top_level_dir = if current == root {
                // We're at the root, so this subdirectory is a top-level directory
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if we should include this top-level directory
                    if !should_include_top_level_dir(dir_name, include, exclude) {
                        continue; // Skip this entire subtree
                    }
                    Some(dir_name.to_string())
                } else {
                    continue;
                }
            } else {
                // We're in a subdirectory, inherit the top-level directory
                top_level_dir.clone()
            };

            // Recursively traverse subdirectories
            collect_file_summary_recursive(
                root,
                &path,
                max_depth,
                current_depth + 1,
                include,
                exclude,
                summary,
                new_top_level_dir,
            )?;
        } else if file_type.is_file() {
            // Only count files that are in subdirectories (not in root)
            if path.parent() != Some(root) {
                summary.file_count += 1;

                // Track the top-level directory
                if let Some(ref dir) = top_level_dir {
                    summary.top_level_dirs.insert(dir.clone());
                }
            }
        }
    }

    Ok(())
}

fn get_confirmation() -> io::Result<bool> {
    print!("Proceed? (Y/n): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_uppercase();

    Ok(input == "Y" || input == "YES")
}

/// Flatten directory
fn flatten_directory_by_traversal(
    root: &Path,
    max_depth: Option<usize>,
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
    quiet: bool,
) -> io::Result<usize> {
    let mut moved_count = 0;

    flatten_directory_by_traversal_recursive(
        root,
        root,
        max_depth,
        0,
        include,
        exclude,
        &mut moved_count,
        None,
        quiet,
    )?;

    Ok(moved_count)
}

fn flatten_directory_by_traversal_recursive(
    root: &Path,
    current: &Path,
    max_depth: Option<usize>,
    current_depth: usize,
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
    moved_count: &mut usize,
    top_level_dir: Option<String>,
    quiet: bool,
) -> io::Result<()> {
    if let Some(max) = max_depth {
        if current_depth > max {
            return Ok(());
        }
    }

    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            // Determine the top-level directory name
            let new_top_level_dir = if current == root {
                // We're at the root, so this subdirectory is a top-level directory
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if we should include this top-level directory
                    if !should_include_top_level_dir(dir_name, include, exclude) {
                        continue; // Skip this entire subtree
                    }
                    Some(dir_name.to_string())
                } else {
                    continue;
                }
            } else {
                // We're in a subdirectory, inherit the top-level directory
                top_level_dir.clone()
            };

            // Recursively traverse subdirectories
            flatten_directory_by_traversal_recursive(
                root,
                &path,
                max_depth,
                current_depth + 1,
                include,
                exclude,
                moved_count,
                new_top_level_dir,
                quiet,
            )?;
        } else if file_type.is_file() {
            // Only move files that are in subdirectories (not in root)
            if path.parent() != Some(root) {
                // Move the file to root
                let file_name = match path.file_name() {
                    Some(name) => name,
                    None => continue,
                };

                let mut dest = root.join(file_name);

                // Handle filename conflicts by appending a number
                let mut counter = 1;
                while dest.exists() {
                    let stem = Path::new(file_name)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("file");
                    let extension = Path::new(file_name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");

                    let new_name = if extension.is_empty() {
                        format!("{}_{}", stem, counter)
                    } else {
                        format!("{}_{}.{}", stem, counter, extension)
                    };

                    dest = root.join(new_name);
                    counter += 1;
                }

                match fs::rename(&path, &dest) {
                    Ok(_) => {
                        *moved_count += 1;
                        if !quiet {
                            println!("Moved: {} -> {}", display_path(&path), display_path(&dest));
                        }
                    }
                    Err(e) => {
                        eprintln!("Error moving {}: {}", display_path(&path), e);
                    }
                }
            }
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    // Validate that both include and exclude aren't used together
    if cli.include.is_some() && cli.exclude.is_some() {
        eprintln!("Error: Cannot use both --include and --exclude options at the same time");
        std::process::exit(1);
    }

    // Verify directory exists
    if !cli.directory.exists() {
        eprintln!("Error: Directory '{}' does not exist", display_path(&cli.directory));
        std::process::exit(1);
    }

    if !cli.directory.is_dir() {
        eprintln!("Error: '{}' is not a directory", display_path(&cli.directory));
        std::process::exit(1);
    }

    // Canonicalize the path to get the full absolute path
    let canonical_directory = cli.directory.canonicalize()?;

    // Collect summary of files to be moved (memory efficient - doesn't store all paths)
    let summary = collect_file_summary(
        &canonical_directory,
        cli.max_depth,
        &cli.include,
        &cli.exclude,
    )?;

    if summary.file_count == 0 {
        if !cli.quiet {
            println!("No files found in subdirectories to flatten.");
        }
        return Ok(());
    }

    // Show summary and get confirmation
    if !cli.quiet {
        println!(
            "Found {} file(s) to move to '{}'",
            summary.file_count,
            display_path(&canonical_directory)
        );

        if !summary.top_level_dirs.is_empty() {
            println!("Top-level directories to be flattened:");
            let mut dirs: Vec<_> = summary.top_level_dirs.iter().cloned().collect();
            dirs.sort();
            for dir in dirs {
                println!("  - {}", dir);
            }
        }
    }

    // Skip confirmation if -y or -q is provided
    if !cli.skip_confirmation && !cli.quiet {
        if !get_confirmation()? {
            println!("Flatten cancelled.");
            return Ok(());
        }
    }

    // Perform the flattening (re-traverses the filesystem)
    let moved_count = flatten_directory_by_traversal(
        &canonical_directory,
        cli.max_depth,
        &cli.include,
        &cli.exclude,
        cli.quiet,
    )?;

    if !cli.quiet {
        println!("\nSuccessfully moved {} file(s)", moved_count);
    }

    // Delete the now-empty top-level directories
    for dir in &summary.top_level_dirs {
        let dir_path = canonical_directory.join(dir);
        if dir_path.exists() && dir_path.is_dir() {
            match fs::remove_dir_all(&dir_path) {
                Ok(_) => {}
                Err(e) => eprintln!("Error removing directory {}: {}", dir, e),
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_structure(root: &Path) -> io::Result<()> {
        // Create a nested directory structure:
        // root/
        //   file0.txt (should not be moved - already in root)
        //   level1/
        //     file1.txt (depth 1)
        //     level2/
        //       file2.txt (depth 2)
        //       level3/
        //         file3.txt (depth 3)
        //         level4/
        //           file4.txt (depth 4)

        fs::write(root.join("file0.txt"), "root level")?;

        let level1 = root.join("level1");
        fs::create_dir(&level1)?;
        fs::write(level1.join("file1.txt"), "depth 1")?;

        let level2 = level1.join("level2");
        fs::create_dir(&level2)?;
        fs::write(level2.join("file2.txt"), "depth 2")?;

        let level3 = level2.join("level3");
        fs::create_dir(&level3)?;
        fs::write(level3.join("file3.txt"), "depth 3")?;

        let level4 = level3.join("level4");
        fs::create_dir(&level4)?;
        fs::write(level4.join("file4.txt"), "depth 4")?;

        Ok(())
    }

    fn create_multi_dir_structure(root: &Path) -> io::Result<()> {
        // Create structure with multiple top-level directories:
        // root/
        //   docs/
        //     readme.txt
        //   src/
        //     main.rs
        //   tests/
        //     test1.rs
        //   documentation/
        //     guide.txt

        let docs = root.join("docs");
        fs::create_dir(&docs)?;
        fs::write(docs.join("readme.txt"), "docs")?;

        let src = root.join("src");
        fs::create_dir(&src)?;
        fs::write(src.join("main.rs"), "src")?;

        let tests = root.join("tests");
        fs::create_dir(&tests)?;
        fs::write(tests.join("test1.rs"), "tests")?;

        let documentation = root.join("documentation");
        fs::create_dir(&documentation)?;
        fs::write(documentation.join("guide.txt"), "documentation")?;

        Ok(())
    }

    // Tests for starts_with_pattern
    #[test]
    fn test_starts_with_pattern() {
        assert!(starts_with_pattern("docs", "doc"));
        assert!(starts_with_pattern("documentation", "doc"));
        assert!(starts_with_pattern("DOCS", "doc"));
        assert!(starts_with_pattern("docs", "DOC"));
        assert!(!starts_with_pattern("src", "doc"));
        assert!(starts_with_pattern("src", "src"));
        assert!(starts_with_pattern("tests", "test"));
        // Test that it's prefix matching, not substring matching
        assert!(!starts_with_pattern("mydocs", "doc"));
        assert!(!starts_with_pattern("src", "rc"));
    }

    // Tests for should_include_top_level_dir
    #[test]
    fn test_should_include_no_filters() {
        assert!(should_include_top_level_dir("docs", &None, &None));
        assert!(should_include_top_level_dir("src", &None, &None));
        assert!(should_include_top_level_dir("tests", &None, &None));
    }

    #[test]
    fn test_should_include_with_include_filter() {
        let include = Some(vec!["src".to_string()]);
        assert!(!should_include_top_level_dir("docs", &include, &None));
        assert!(should_include_top_level_dir("src", &include, &None));
        assert!(!should_include_top_level_dir("tests", &include, &None));
    }

    #[test]
    fn test_should_include_with_multiple_include_filters() {
        let include = Some(vec!["src".to_string(), "test".to_string()]);
        assert!(!should_include_top_level_dir("docs", &include, &None));
        assert!(should_include_top_level_dir("src", &include, &None));
        assert!(should_include_top_level_dir("tests", &include, &None)); // matches "test"
    }

    #[test]
    fn test_should_include_with_exclude_filter() {
        let exclude = Some(vec!["src".to_string()]);
        assert!(should_include_top_level_dir("docs", &None, &exclude));
        assert!(!should_include_top_level_dir("src", &None, &exclude));
        assert!(should_include_top_level_dir("tests", &None, &exclude));
    }

    #[test]
    fn test_should_include_with_prefix_matching() {
        let include = Some(vec!["doc".to_string()]);
        assert!(should_include_top_level_dir("docs", &include, &None));
        assert!(should_include_top_level_dir("documentation", &include, &None));
        assert!(!should_include_top_level_dir("src", &include, &None));
        // Test that it's prefix matching, not substring matching
        assert!(!should_include_top_level_dir("mydocs", &include, &None));
    }

    // Tests for collect_file_summary
    #[test]
    fn test_collect_summary_unlimited_depth() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let summary = collect_file_summary(root, None, &None, &None).unwrap();

        // Should count all files except file0.txt (which is in root)
        assert_eq!(summary.file_count, 4);
        assert_eq!(summary.top_level_dirs.len(), 1);
        assert!(summary.top_level_dirs.contains("level1"));
    }

    #[test]
    fn test_collect_summary_max_depth_1() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let summary = collect_file_summary(root, Some(1), &None, &None).unwrap();

        // Should only count file1.txt (at depth 1)
        assert_eq!(summary.file_count, 1);
    }

    #[test]
    fn test_collect_summary_max_depth_2() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let summary = collect_file_summary(root, Some(2), &None, &None).unwrap();

        // Should count file1.txt and file2.txt (depths 1 and 2)
        assert_eq!(summary.file_count, 2);
    }

    #[test]
    fn test_collect_summary_max_depth_0() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let summary = collect_file_summary(root, Some(0), &None, &None).unwrap();

        // Should count no files (depth 0 means only look in root, but we don't count root files)
        assert_eq!(summary.file_count, 0);
    }

    #[test]
    fn test_collect_summary_with_include() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let include = Some(vec!["src".to_string()]);
        let summary = collect_file_summary(root, None, &include, &None).unwrap();

        assert_eq!(summary.file_count, 1);
        assert!(summary.top_level_dirs.contains("src"));
        assert!(!summary.top_level_dirs.contains("docs"));
    }

    #[test]
    fn test_collect_summary_with_prefix_include() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        // "doc" should match both "docs" and "documentation" (prefix match)
        let include = Some(vec!["doc".to_string()]);
        let summary = collect_file_summary(root, None, &include, &None).unwrap();

        assert_eq!(summary.file_count, 2);
        assert!(summary.top_level_dirs.contains("docs"));
        assert!(summary.top_level_dirs.contains("documentation"));
        assert!(!summary.top_level_dirs.contains("src"));
    }

    #[test]
    fn test_collect_summary_with_exclude() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let exclude = Some(vec!["src".to_string()]);
        let summary = collect_file_summary(root, None, &None, &exclude).unwrap();

        assert_eq!(summary.file_count, 3);
        assert!(!summary.top_level_dirs.contains("src"));
        assert!(summary.top_level_dirs.contains("docs"));
    }

    #[test]
    fn test_collect_summary_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let summary = collect_file_summary(root, None, &None, &None).unwrap();
        assert_eq!(summary.file_count, 0);
        assert_eq!(summary.top_level_dirs.len(), 0);
    }

    // Tests for flatten_directory_by_traversal
    #[test]
    fn test_flatten_no_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create subdirectory with files
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("test1.txt"), "content1").unwrap();
        fs::write(subdir.join("test2.txt"), "content2").unwrap();

        let moved_count = flatten_directory_by_traversal(root, None, &None, &None, false).unwrap();

        assert_eq!(moved_count, 2);
        assert!(root.join("test1.txt").exists());
        assert!(root.join("test2.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("test1.txt")).unwrap(),
            "content1"
        );
        assert_eq!(
            fs::read_to_string(root.join("test2.txt")).unwrap(),
            "content2"
        );
    }

    #[test]
    fn test_flatten_with_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a file in root
        fs::write(root.join("test.txt"), "root content").unwrap();

        // Create subdirectory with conflicting filename
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("test.txt"), "subdir content").unwrap();

        let moved_count = flatten_directory_by_traversal(root, None, &None, &None, false).unwrap();

        assert_eq!(moved_count, 1);
        // Original file should remain unchanged
        assert_eq!(
            fs::read_to_string(root.join("test.txt")).unwrap(),
            "root content"
        );

        // Conflicting file should be renamed
        assert!(root.join("test_1.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("test_1.txt")).unwrap(),
            "subdir content"
        );
    }

    #[test]
    fn test_flatten_multiple_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a file in root
        fs::write(root.join("test.txt"), "root").unwrap();

        // Create multiple subdirectories with the same filename
        let subdir1 = root.join("subdir1");
        fs::create_dir(&subdir1).unwrap();
        fs::write(subdir1.join("test.txt"), "content1").unwrap();

        let subdir2 = root.join("subdir2");
        fs::create_dir(&subdir2).unwrap();
        fs::write(subdir2.join("test.txt"), "content2").unwrap();

        let moved_count = flatten_directory_by_traversal(root, None, &None, &None, false).unwrap();

        assert_eq!(moved_count, 2);
        assert!(root.join("test.txt").exists());
        assert!(root.join("test_1.txt").exists());
        assert!(root.join("test_2.txt").exists());
    }

    #[test]
    fn test_flatten_with_max_depth() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let moved_count = flatten_directory_by_traversal(root, Some(2), &None, &None, false).unwrap();

        // Should only move files at depths 1 and 2
        assert_eq!(moved_count, 2);
        assert!(root.join("file1.txt").exists());
        assert!(root.join("file2.txt").exists());
        assert!(!root.join("file3.txt").exists());
        assert!(!root.join("file4.txt").exists());
    }

    #[test]
    fn test_flatten_with_include_filter() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let include = Some(vec!["src".to_string()]);
        let moved_count = flatten_directory_by_traversal(root, None, &include, &None, false).unwrap();

        // Should only move files from "src" directory
        assert_eq!(moved_count, 1);
        assert!(root.join("main.rs").exists());
        assert!(!root.join("readme.txt").exists());
        assert!(!root.join("test1.rs").exists());
    }

    #[test]
    fn test_flatten_with_exclude_filter() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let exclude = Some(vec!["src".to_string()]);
        let moved_count = flatten_directory_by_traversal(root, None, &None, &exclude, false).unwrap();

        // Should move all files except from "src" directory
        assert_eq!(moved_count, 3);
        assert!(!root.join("main.rs").exists());
        assert!(root.join("readme.txt").exists());
        assert!(root.join("test1.rs").exists());
        assert!(root.join("guide.txt").exists());
    }

    #[test]
    fn test_flatten_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let moved_count = flatten_directory_by_traversal(root, None, &None, &None, false).unwrap();
        assert_eq!(moved_count, 0);
    }

    // Tests for quiet mode
    #[test]
    fn test_flatten_quiet_mode_basic() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create subdirectory with files
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("test1.txt"), "content1").unwrap();
        fs::write(subdir.join("test2.txt"), "content2").unwrap();

        // Test with quiet mode enabled
        let moved_count = flatten_directory_by_traversal(root, None, &None, &None, true).unwrap();

        // Verify files were moved correctly despite quiet mode
        assert_eq!(moved_count, 2);
        assert!(root.join("test1.txt").exists());
        assert!(root.join("test2.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("test1.txt")).unwrap(),
            "content1"
        );
        assert_eq!(
            fs::read_to_string(root.join("test2.txt")).unwrap(),
            "content2"
        );
    }

    #[test]
    fn test_flatten_quiet_mode_with_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a file in root
        fs::write(root.join("test.txt"), "root content").unwrap();

        // Create subdirectory with conflicting filename
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("test.txt"), "subdir content").unwrap();

        // Test with quiet mode enabled
        let moved_count = flatten_directory_by_traversal(root, None, &None, &None, true).unwrap();

        // Verify conflict resolution works in quiet mode
        assert_eq!(moved_count, 1);
        assert_eq!(
            fs::read_to_string(root.join("test.txt")).unwrap(),
            "root content"
        );
        assert!(root.join("test_1.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("test_1.txt")).unwrap(),
            "subdir content"
        );
    }

    #[test]
    fn test_flatten_quiet_mode_with_depth() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        // Test with quiet mode and max depth
        let moved_count = flatten_directory_by_traversal(root, Some(2), &None, &None, true).unwrap();

        // Verify depth limiting works in quiet mode
        assert_eq!(moved_count, 2);
        assert!(root.join("file1.txt").exists());
        assert!(root.join("file2.txt").exists());
        assert!(!root.join("file3.txt").exists());
        assert!(!root.join("file4.txt").exists());
    }

    #[test]
    fn test_flatten_quiet_mode_with_include_filter() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let include = Some(vec!["src".to_string()]);
        // Test with quiet mode and include filter
        let moved_count = flatten_directory_by_traversal(root, None, &include, &None, true).unwrap();

        // Verify filtering works in quiet mode
        assert_eq!(moved_count, 1);
        assert!(root.join("main.rs").exists());
        assert!(!root.join("readme.txt").exists());
        assert!(!root.join("test1.rs").exists());
    }

    #[test]
    fn test_flatten_quiet_mode_with_exclude_filter() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let exclude = Some(vec!["src".to_string()]);
        // Test with quiet mode and exclude filter
        let moved_count = flatten_directory_by_traversal(root, None, &None, &exclude, true).unwrap();

        // Verify excluding works in quiet mode
        assert_eq!(moved_count, 3);
        assert!(!root.join("main.rs").exists());
        assert!(root.join("readme.txt").exists());
        assert!(root.join("test1.rs").exists());
        assert!(root.join("guide.txt").exists());
    }

    #[test]
    fn test_flatten_quiet_vs_normal_same_result() {
        // Verify that quiet mode produces the same file operations as normal mode
        let temp_dir1 = TempDir::new().unwrap();
        let root1 = temp_dir1.path();

        let temp_dir2 = TempDir::new().unwrap();
        let root2 = temp_dir2.path();

        // Create identical structures
        let subdir1 = root1.join("subdir");
        fs::create_dir(&subdir1).unwrap();
        fs::write(subdir1.join("file1.txt"), "content1").unwrap();
        fs::write(subdir1.join("file2.txt"), "content2").unwrap();

        let subdir2 = root2.join("subdir");
        fs::create_dir(&subdir2).unwrap();
        fs::write(subdir2.join("file1.txt"), "content1").unwrap();
        fs::write(subdir2.join("file2.txt"), "content2").unwrap();

        // Run with normal mode
        let count1 = flatten_directory_by_traversal(root1, None, &None, &None, false).unwrap();

        // Run with quiet mode
        let count2 = flatten_directory_by_traversal(root2, None, &None, &None, true).unwrap();

        // Verify same number of files moved
        assert_eq!(count1, count2);
        assert_eq!(count1, 2);

        // Verify same files exist in both directories
        assert!(root1.join("file1.txt").exists());
        assert!(root1.join("file2.txt").exists());
        assert!(root2.join("file1.txt").exists());
        assert!(root2.join("file2.txt").exists());

        // Verify same content
        assert_eq!(
            fs::read_to_string(root1.join("file1.txt")).unwrap(),
            fs::read_to_string(root2.join("file1.txt")).unwrap()
        );
        assert_eq!(
            fs::read_to_string(root1.join("file2.txt")).unwrap(),
            fs::read_to_string(root2.join("file2.txt")).unwrap()
        );
    }

    #[test]
    fn test_flatten_quiet_mode_outputs_errors() {
        // This test verifies that errors are still output even in quiet mode
        // Quiet mode should suppress informational output but NOT error messages
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a subdirectory with files
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("blocked.txt"), "will fail to move").unwrap();
        fs::write(subdir.join("success.txt"), "will move successfully").unwrap();

        // Create a DIRECTORY (not a file) in root with the same name as one of the files
        // This will cause fs::rename to fail for blocked.txt because you can't rename
        // a file to a path that already exists as a directory
        let blocking_dir = root.join("blocked.txt");
        fs::create_dir(&blocking_dir).unwrap();

        // Run with quiet mode enabled
        // The function should continue despite the error and return Ok
        let moved_count = flatten_directory_by_traversal(root, None, &None, &None, true).unwrap();

        // Verify only the successful file was moved (count should be 1, not 2)
        assert_eq!(moved_count, 1);

        // Verify success.txt was moved successfully
        assert!(root.join("success.txt").exists());
        assert_eq!(
            fs::read_to_string(root.join("success.txt")).unwrap(),
            "will move successfully"
        );

        // Verify blocked.txt was NOT moved (still in subdirectory)
        assert!(subdir.join("blocked.txt").exists());

        // Verify the blocking directory still exists
        assert!(blocking_dir.exists());
        assert!(blocking_dir.is_dir());

        // Note: This test verifies the error BEHAVIOR (file not moved, operation continues)
        // The actual error message "Error moving..." is written to stderr via eprintln!
        // In a real run with quiet mode, you would see:
        //   stderr: "Error moving /path/to/subdir/blocked.txt: ..."
        //   stdout: (empty - no "Moved:" messages due to quiet mode)
        // To verify stderr output, run: cargo test test_flatten_quiet_mode_outputs_errors -- --nocapture
    }
}
