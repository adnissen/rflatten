use clap::Parser;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

    /// Include only directories that fuzzy-match these patterns (comma-separated)
    #[arg(short = 'i', long = "include", value_delimiter = ',')]
    include: Option<Vec<String>>,

    /// Exclude directories that fuzzy-match these patterns (comma-separated)
    #[arg(short = 'e', long = "exclude", value_delimiter = ',')]
    exclude: Option<Vec<String>>,
}

/// Fuzzy match: checks if the pattern is a case-insensitive substring of the target
fn fuzzy_match(target: &str, pattern: &str) -> bool {
    target.to_lowercase().contains(&pattern.to_lowercase())
}

/// Filter files based on include/exclude patterns for top-level directories
fn filter_files_by_patterns(
    root: &Path,
    files: Vec<PathBuf>,
    include: &Option<Vec<String>>,
    exclude: &Option<Vec<String>>,
) -> Vec<PathBuf> {
    files
        .into_iter()
        .filter(|file| {
            // Get the top-level directory name
            if let Ok(relative_path) = file.strip_prefix(root) {
                if let Some(first_component) = relative_path.components().next() {
                    if let Some(dir_name) = first_component.as_os_str().to_str() {
                        // Check include patterns
                        if let Some(include_patterns) = include {
                            return include_patterns.iter().any(|p| fuzzy_match(dir_name, p));
                        }

                        // Check exclude patterns
                        if let Some(exclude_patterns) = exclude {
                            return !exclude_patterns.iter().any(|p| fuzzy_match(dir_name, p));
                        }
                    }
                }
            }
            true
        })
        .collect()
}

fn collect_files(dir: &Path, max_depth: Option<usize>) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_recursive(dir, dir, max_depth, 0, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    max_depth: Option<usize>,
    current_depth: usize,
    files: &mut Vec<PathBuf>,
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
            // Recursively traverse subdirectories
            collect_files_recursive(root, &path, max_depth, current_depth + 1, files)?;
        } else if file_type.is_file() {
            // Only collect files that are in subdirectories (not in root)
            if path.parent() != Some(root) {
                files.push(path);
            }
        }
    }

    Ok(())
}

fn get_confirmation() -> io::Result<bool> {
    print!("Proceed with flatten? (Y/n): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_uppercase();

    Ok(input == "Y" || input == "YES")
}

fn flatten_directory(root: &Path, files: Vec<PathBuf>) -> io::Result<()> {
    let mut moved_count = 0;

    for file_path in files {
        let file_name = match file_path.file_name() {
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

        match fs::rename(&file_path, &dest) {
            Ok(_) => {
                moved_count += 1;
                println!("Moved: {} -> {}", file_path.display(), dest.display());
            }
            Err(e) => {
                eprintln!("Error moving {}: {}", file_path.display(), e);
            }
        }
    }

    println!("\nSuccessfully moved {} file(s)", moved_count);
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
        eprintln!("Error: Directory '{}' does not exist", cli.directory.display());
        std::process::exit(1);
    }

    if !cli.directory.is_dir() {
        eprintln!("Error: '{}' is not a directory", cli.directory.display());
        std::process::exit(1);
    }

    // Canonicalize the path to get the full absolute path
    let canonical_directory = cli.directory.canonicalize()?;

    // Collect files to be moved
    let mut files = collect_files(&canonical_directory, cli.max_depth)?;

    // Filter files based on include/exclude patterns
    files = filter_files_by_patterns(&canonical_directory, files, &cli.include, &cli.exclude);

    if files.is_empty() {
        println!("No files found in subdirectories to flatten.");
        return Ok(());
    }

    // Collect unique top-level directories that will be flattened
    let mut top_level_dirs = std::collections::HashSet::new();
    for file in &files {
        if let Ok(relative_path) = file.strip_prefix(&canonical_directory) {
            if let Some(first_component) = relative_path.components().next() {
                if let Some(dir_name) = first_component.as_os_str().to_str() {
                    top_level_dirs.insert(dir_name.to_string());
                }
            }
        }
    }

    // Show summary and get confirmation
    println!("Found {} file(s) to move to '{}'", files.len(), canonical_directory.display());

    if !top_level_dirs.is_empty() {
        println!("Top-level directories to be flattened:");
        let mut dirs: Vec<_> = top_level_dirs.clone().into_iter().collect();
        dirs.sort();
        for dir in dirs {
            println!("  - {}", dir);
        }
    }

    if !cli.skip_confirmation {
        if !get_confirmation()? {
            println!("Flatten cancelled.");
            return Ok(());
        }
    }

    // Perform the flattening
    flatten_directory(&canonical_directory, files)?;

    // Delete the now-empty top-level directories
    for dir in top_level_dirs {
        let dir_path = canonical_directory.join(&dir);
        if dir_path.exists() && dir_path.is_dir() {
            match fs::remove_dir_all(&dir_path) {
                Ok(_) => {},
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

    #[test]
    fn test_collect_files_unlimited_depth() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();

        // Should collect all files except file0.txt (which is in root)
        assert_eq!(files.len(), 4);

        let file_names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();

        assert!(file_names.contains(&"file1.txt".to_string()));
        assert!(file_names.contains(&"file2.txt".to_string()));
        assert!(file_names.contains(&"file3.txt".to_string()));
        assert!(file_names.contains(&"file4.txt".to_string()));
    }

    #[test]
    fn test_collect_files_max_depth_1() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let files = collect_files(root, Some(1)).unwrap();

        // Should only collect file1.txt (at depth 1)
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].file_name().unwrap().to_str().unwrap(),
            "file1.txt"
        );
    }

    #[test]
    fn test_collect_files_max_depth_2() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let files = collect_files(root, Some(2)).unwrap();

        // Should collect file1.txt and file2.txt (depths 1 and 2)
        assert_eq!(files.len(), 2);

        let file_names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();

        assert!(file_names.contains(&"file1.txt".to_string()));
        assert!(file_names.contains(&"file2.txt".to_string()));
        assert!(!file_names.contains(&"file3.txt".to_string()));
        assert!(!file_names.contains(&"file4.txt".to_string()));
    }

    #[test]
    fn test_collect_files_max_depth_3() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let files = collect_files(root, Some(3)).unwrap();

        // Should collect files at depths 1, 2, and 3
        assert_eq!(files.len(), 3);

        let file_names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();

        assert!(file_names.contains(&"file1.txt".to_string()));
        assert!(file_names.contains(&"file2.txt".to_string()));
        assert!(file_names.contains(&"file3.txt".to_string()));
        assert!(!file_names.contains(&"file4.txt".to_string()));
    }

    #[test]
    fn test_collect_files_max_depth_0() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_test_structure(root).unwrap();

        let files = collect_files(root, Some(0)).unwrap();

        // Should collect no files (depth 0 means only look in root, but we don't collect root files)
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_flatten_directory_no_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create subdirectory with files
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("test1.txt"), "content1").unwrap();
        fs::write(subdir.join("test2.txt"), "content2").unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 2);

        flatten_directory(root, files).unwrap();

        // Check files were moved to root
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
    fn test_flatten_directory_with_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a file in root
        fs::write(root.join("test.txt"), "root content").unwrap();

        // Create subdirectory with conflicting filename
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("test.txt"), "subdir content").unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 1);

        flatten_directory(root, files).unwrap();

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
    fn test_flatten_directory_multiple_conflicts() {
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

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 2);

        flatten_directory(root, files).unwrap();

        // Should have test.txt, test_1.txt, and test_2.txt
        assert!(root.join("test.txt").exists());
        assert!(root.join("test_1.txt").exists());
        assert!(root.join("test_2.txt").exists());
    }

    #[test]
    fn test_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_only_root_files() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        fs::write(root.join("file1.txt"), "content1").unwrap();
        fs::write(root.join("file2.txt"), "content2").unwrap();

        let files = collect_files(root, None).unwrap();
        // Should not collect files that are already in root
        assert_eq!(files.len(), 0);
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

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("docs", "doc"));
        assert!(fuzzy_match("documentation", "doc"));
        assert!(fuzzy_match("DOCS", "doc"));
        assert!(fuzzy_match("docs", "DOC"));
        assert!(!fuzzy_match("src", "doc"));
        assert!(fuzzy_match("src", "src"));
        assert!(fuzzy_match("tests", "test"));
    }

    #[test]
    fn test_include_single_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 4);

        let include = Some(vec!["src".to_string()]);
        let filtered = filter_files_by_patterns(root, files, &include, &None);

        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].to_str().unwrap().contains("main.rs"));
    }

    #[test]
    fn test_include_multiple_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 4);

        let include = Some(vec!["src".to_string(), "test".to_string()]);
        let filtered = filter_files_by_patterns(root, files, &include, &None);

        assert_eq!(filtered.len(), 2);
        let file_names: Vec<String> = filtered
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(file_names.contains(&"main.rs".to_string()));
        assert!(file_names.contains(&"test1.rs".to_string()));
    }

    #[test]
    fn test_include_fuzzy_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 4);

        // "doc" should match both "docs" and "documentation"
        let include = Some(vec!["doc".to_string()]);
        let filtered = filter_files_by_patterns(root, files, &include, &None);

        assert_eq!(filtered.len(), 2);
        let file_names: Vec<String> = filtered
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(file_names.contains(&"readme.txt".to_string()));
        assert!(file_names.contains(&"guide.txt".to_string()));
    }

    #[test]
    fn test_exclude_single_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 4);

        let exclude = Some(vec!["src".to_string()]);
        let filtered = filter_files_by_patterns(root, files, &None, &exclude);

        assert_eq!(filtered.len(), 3);
        let file_names: Vec<String> = filtered
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(file_names.contains(&"readme.txt".to_string()));
        assert!(file_names.contains(&"test1.rs".to_string()));
        assert!(file_names.contains(&"guide.txt".to_string()));
        assert!(!file_names.contains(&"main.rs".to_string()));
    }

    #[test]
    fn test_exclude_multiple_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 4);

        let exclude = Some(vec!["src".to_string(), "test".to_string()]);
        let filtered = filter_files_by_patterns(root, files, &None, &exclude);

        assert_eq!(filtered.len(), 2);
        let file_names: Vec<String> = filtered
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(file_names.contains(&"readme.txt".to_string()));
        assert!(file_names.contains(&"guide.txt".to_string()));
    }

    #[test]
    fn test_exclude_fuzzy_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 4);

        // "doc" should exclude both "docs" and "documentation"
        let exclude = Some(vec!["doc".to_string()]);
        let filtered = filter_files_by_patterns(root, files, &None, &exclude);

        assert_eq!(filtered.len(), 2);
        let file_names: Vec<String> = filtered
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(file_names.contains(&"main.rs".to_string()));
        assert!(file_names.contains(&"test1.rs".to_string()));
    }

    #[test]
    fn test_no_include_or_exclude() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        create_multi_dir_structure(root).unwrap();

        let files = collect_files(root, None).unwrap();
        assert_eq!(files.len(), 4);

        let filtered = filter_files_by_patterns(root, files.clone(), &None, &None);

        // With no filters, all files should be included
        assert_eq!(filtered.len(), 4);
    }
}
