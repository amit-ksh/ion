//! Check the actual launch directory on the worker, before any CLI is started.
use std::{io, path::Path};

pub fn validate_working_directory(path: &Path) -> io::Result<()> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!(
                "Working folder '{}' is a file. Choose an existing project folder from New Task and try again.",
                path.display()
            ),
        )),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!(
                "Working folder '{}' is unavailable: {error}. Choose an existing project folder from New Task, or restore the folder, and try again.",
                path.display()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_directory_is_accepted() {
        validate_working_directory(&std::env::current_dir().unwrap()).unwrap();
    }
    #[test]
    fn missing_folder_names_the_path_and_recovery() {
        let path = std::env::temp_dir()
            .join(format!("ion-missing-workspace-{}", std::process::id()))
            .join("not-created");
        let error = validate_working_directory(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains(&path.display().to_string()));
        assert!(error.to_string().contains("New Task"));
        assert!(
            !path.exists(),
            "a failed launch must not create an empty replacement project"
        );
    }
    #[test]
    fn regular_file_is_not_a_workspace() {
        let error = validate_working_directory(&std::env::current_exe().unwrap()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotADirectory);
    }
}
