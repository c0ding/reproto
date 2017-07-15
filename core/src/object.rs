use errors::*;
use std::fmt;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

pub trait Object: Send + fmt::Display + fmt::Debug {
    /// Get a path to the object, if one exists.
    fn path(&self) -> Option<&Path>;

    /// Open a reader to the object.
    fn read<'a>(&'a self) -> Result<Box<Read + 'a>>;
}

#[derive(Debug)]
pub struct BytesObject {
    bytes: Vec<u8>,
}

impl BytesObject {
    pub fn new(bytes: Vec<u8>) -> BytesObject {
        BytesObject { bytes: bytes }
    }
}

impl Object for BytesObject {
    fn path(&self) -> Option<&Path> {
        None
    }

    fn read<'a>(&'a self) -> Result<Box<Read + 'a>> {
        Ok(Box::new(Cursor::new(&self.bytes)))
    }
}

impl fmt::Display for BytesObject {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "<bytes>")
    }
}

#[derive(Debug)]
pub struct PathObject {
    path: PathBuf,
}

impl PathObject {
    pub fn new<P: AsRef<Path>>(path: P) -> PathObject {
        PathObject { path: path.as_ref().to_owned() }
    }
}

impl Object for PathObject {
    fn path(&self) -> Option<&Path> {
        Some(self.path.as_ref())
    }

    fn read(&self) -> Result<Box<Read>> {
        Ok(Box::new(File::open(&self.path)?))
    }
}

impl fmt::Display for PathObject {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.path.display())
    }
}