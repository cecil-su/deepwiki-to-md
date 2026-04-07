use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// Represents the output to be written.
pub enum Output {
    /// Write to stdout
    Stdout(String),
    /// Write to a single file
    SingleFile { path: PathBuf, content: String },
    /// Write multiple files to a directory
    Directory {
        base_dir: PathBuf,
        files: Vec<(PathBuf, String)>,
    },
}

/// Write output to the appropriate destination.
pub fn write_output(output: Output) -> Result<WriteResult, io::Error> {
    match output {
        Output::Stdout(content) => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(content.as_bytes())?;
            handle.write_all(b"\n")?;
            Ok(WriteResult {
                files_written: 0,
                mode: "stdout".to_string(),
            })
        }
        Output::SingleFile { path, content } => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&path, content)?;
            Ok(WriteResult {
                files_written: 1,
                mode: format!("single file: {}", path.display()),
            })
        }
        Output::Directory { base_dir, files } => {
            fs::create_dir_all(&base_dir)?;
            let mut count = 0;
            for (relative_path, content) in &files {
                let full_path = base_dir.join(relative_path);
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&full_path, content)?;
                count += 1;
            }
            Ok(WriteResult {
                files_written: count,
                mode: format!("directory: {}", base_dir.display()),
            })
        }
    }
}

/// Result of a write operation.
pub struct WriteResult {
    pub files_written: usize,
    pub mode: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.md");

        let result = write_output(Output::SingleFile {
            path: path.clone(),
            content: "hello world".to_string(),
        })
        .unwrap();

        assert_eq!(result.files_written, 1);
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
    }

    #[test]
    fn test_write_single_file_creates_parent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/dir/output.md");

        write_output(Output::SingleFile {
            path: path.clone(),
            content: "nested".to_string(),
        })
        .unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "nested");
    }

    #[test]
    fn test_write_directory() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("wiki");

        let files = vec![
            (PathBuf::from("1-overview.md"), "overview".to_string()),
            (PathBuf::from("2-guide.md"), "guide".to_string()),
        ];

        let result = write_output(Output::Directory {
            base_dir: base.clone(),
            files,
        })
        .unwrap();

        assert_eq!(result.files_written, 2);
        assert_eq!(
            fs::read_to_string(base.join("1-overview.md")).unwrap(),
            "overview"
        );
        assert_eq!(
            fs::read_to_string(base.join("2-guide.md")).unwrap(),
            "guide"
        );
    }

    #[test]
    fn test_write_directory_with_subdirs() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("wiki");

        let files = vec![(
            PathBuf::from("assets/mermaid/diagram.svg"),
            "<svg>...</svg>".to_string(),
        )];

        write_output(Output::Directory {
            base_dir: base.clone(),
            files,
        })
        .unwrap();

        assert!(base.join("assets/mermaid/diagram.svg").exists());
    }
}
