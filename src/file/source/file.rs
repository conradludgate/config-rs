use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Read};
use std::iter::Iterator;
use std::path::{Path, PathBuf};

use crate::file::{
    format::ALL_EXTENSIONS, source::FileSourceResult, FileSource, FileStoredFormat, Format,
};

/// Describes a file sourced from a file
#[derive(Clone, Debug)]
pub struct FileSourceFile {
    /// Path of configuration file
    name: PathBuf,
}

impl FileSourceFile {
    pub fn new(name: PathBuf) -> FileSourceFile {
        FileSourceFile { name }
    }

    fn find_file<F>(
        &self,
        format_hint: Option<F>,
    ) -> Result<(PathBuf, Box<dyn Format>), Box<dyn Error + Send + Sync>>
    where
        F: FileStoredFormat + Format + 'static,
    {
        // First check for an _exact_ match
        let mut filename = env::current_dir()?.as_path().join(self.name.clone());
        if filename.is_file() {
            return match format_hint {
                Some(format) => Ok((filename, Box::new(format))),
                None => {
                    for (format, extensions) in ALL_EXTENSIONS.iter() {
                        if extensions.contains(
                            &filename
                                .extension()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .as_ref(),
                        ) {
                            return Ok((filename, Box::new(*format)));
                        }
                    }

                    Err(Box::new(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "configuration file \"{}\" is not of a registered file format",
                            filename.to_string_lossy()
                        ),
                    )))
                }
            };
        }

        match format_hint {
            Some(format) => {
                for ext in format.file_extensions() {
                    filename.set_extension(ext);

                    if filename.is_file() {
                        return Ok((filename, Box::new(format)));
                    }
                }
            }

            None => {
                for (format, extensions) in ALL_EXTENSIONS.iter() {
                    for ext in format.extensions() {
                        filename.set_extension(ext);

                        if filename.is_file() {
                            return Ok((filename, Box::new(*format)));
                        }
                    }
                }
            }
        }

        Err(Box::new(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "configuration file \"{}\" not found",
                self.name.to_string_lossy()
            ),
        )))
    }
}

impl<F> FileSource<F> for FileSourceFile
where
    F: Format + FileStoredFormat + 'static,
{
    fn resolve(
        &self,
        format_hint: Option<F>,
    ) -> Result<FileSourceResult, Box<dyn Error + Send + Sync>> {
        // Find file
        let (filename, format) = self.find_file(format_hint)?;

        // Attempt to use a relative path for the URI
        let base = env::current_dir()?;
        let uri = match path_relative_from(&filename, &base) {
            Some(value) => value,
            None => filename.clone(),
        };

        // Read contents from file
        let mut file = fs::File::open(filename)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;

        Ok(FileSourceResult {
            uri: Some(uri.to_string_lossy().into_owned()),
            content: text,
            format,
        })
    }
}

// TODO: This should probably be a crate
// https://github.com/rust-lang/rust/blob/master/src/librustc_trans/back/rpath.rs#L128
fn path_relative_from(path: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(PathBuf::from(path))
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }
        Some(comps.iter().map(|c| c.as_os_str()).collect())
    }
}
