use lazy_static::lazy_static;
use regex::Regex;
use sha2::Digest;
use futures::io::{AsyncRead, BufReader, AsyncBufReadExt};
use async_compression::futures::bufread::GzipDecoder;
use serde::{Serialize, Deserialize};

use std::collections::HashMap;
use std::marker::Unpin;

lazy_static! {
    pub static ref CONTENTS_REGEX: Regex = Regex::new(r"(?:(?:./)?usr/lib/)(?P<name>lib[a-zA-Z0-9_\-+]+).so(?:.(?P<sover>(?:[0-9]+.?)+)*)?   (?:[a-zA-Z0-9]*)/(?P<dep>[a-zA-Z0-9\-]+)").expect("Failed to parse regex");
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lib {
    package_name: String,
    library_name: String,
    package_version: Option<String>,
    sover: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ContentsParser {
    pub(crate) filter: Option<Regex>,
}

impl Lib {
    pub fn new<S: AsRef<str>>(
        package_name: S,
        library_name: S,
        sover: Option<String>,
    ) -> Self {
        Self {
            package_name: package_name.as_ref().to_string(),
            library_name: library_name.as_ref().to_string(),
            package_version: None,
            sover,
        }
    }

    pub fn get_lib_name(&self) -> String {
        self.library_name.replace('_', "-").to_lowercase()
    }

    pub fn get_translated_lib_name(&self) -> String {
        let version_suffix = match &self.sover {
            Some(sover) => {
                match sover.split_once('.') {
                    Some((suffix, _)) => Some(suffix.to_string()),
                    _ => Some(sover.clone()),
                }
            },
            _ => None,
        };
        let end_numeric = self.library_name.chars().last().unwrap().is_numeric();
        let lib_name = self.get_lib_name();

        match (end_numeric, version_suffix) {
            (true, Some(suffix)) => format!("{}-{}", lib_name, suffix),
            (false, Some(suffix)) => format!("{}{}", lib_name, suffix),
            _ => lib_name,
        }
    }

    pub fn get_translated_dev_name(&self) -> String {
        format!("{}-dev", self.get_lib_name())
    }

    pub fn set_version<S: AsRef<str>>(mut self, version: S) {
        self.package_version = Some(version.as_ref().to_string());
    }

    pub fn get_version(&self) -> Option<String> {
        match (self.sover.clone(), self.package_version.clone()) {
            (Some(sover), _) => Some(sover),
            (None, Some(version)) => Some(version),
            _ => None,
        }
    }

    pub fn get_sover(&self) -> Option<String> {
        self.sover.clone()
    }

    pub fn get_package_name(&self) -> String {
        self.package_name.clone()
    }
}

impl ContentsParser {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn new_with_regex(filter: Regex) -> Self {
        Self { filter: Some(filter) }
    }

    pub async fn parse_async<R: AsyncRead + Unpin, D: Digest>(
        &self, read: &mut R, hasher: &mut D
    ) -> Vec<Lib> {
        let mut source = BufReader::new(read);
        let mut line_buf = Vec::new();
        let mut ret = HashMap::new();
        loop {
            let read_until = source.read_until(b'\n', &mut line_buf).await;
            if read_until.is_err() || (read_until.expect("WTF") == 0) {
                break;
            }
            let line = String::from_utf8_lossy(&line_buf).to_string();
            hasher.update(&line_buf);
            line_buf.clear();
            if !line.contains("usr/lib") {
                continue;
            }
            let captures = CONTENTS_REGEX.captures(&line);
            if captures.is_none() {
                continue;
            }
            let captures = captures.unwrap();
            let name = captures.name("name");
            let dep = captures.name("dep");
            let sover = captures.name("sover").map(|s| s.as_str().to_string());
            if name.is_none() || dep.is_none() {
                continue;
            }
            let name = name.unwrap().as_str().to_string();
            let lib = Lib::new(dep.unwrap().as_str(), &name, sover.clone());
            let lib_name = lib.get_lib_name();
            if ret.contains_key(&lib_name) {
                let prev_lib: &Lib = ret.get(&lib_name).unwrap();
                let prev_sover = prev_lib.get_sover();
                match (prev_sover, sover) {
                    (Some(prev_sover_str), Some(sover_str)) => {
                        if prev_sover_str.matches('.').count() < sover_str.matches('.').count() {
                            ret.remove(&lib_name);
                        }
                    },
                    (None, Some(_)) => {
                        ret.remove(&lib_name);
                    }
                    _ => {},
                }
            }
            match &self.filter {
                None => {ret.insert(lib_name, lib);},
                Some(regex) => {
                    if regex.is_match(&lib.get_translated_lib_name()) || regex.is_match(&lib.get_translated_dev_name()) || regex.is_match(&lib_name) {
                        ret.insert(lib_name, lib);
                    }
                },
            }
        }
        ret.into_values().collect()
    }

    // TODO: Fix digest calculation
    pub async fn parse_async_gzip<R: AsyncRead + Unpin, D: Digest>(
        &self, read: &mut R, hasher: &mut D
    ) -> Vec<Lib> {
        let bufread = BufReader::new(read);
        let mut gzdecode_stream = GzipDecoder::new(bufread);
        self.parse_async(&mut gzdecode_stream, hasher).await
    }
}

#[cfg(test)]
mod test {
    use super::{Lib, ContentsParser};

    use tokio::fs::File;
    use tokio::runtime::Runtime;
    use async_compat::Compat;
    use regex::Regex;
    use sha2::{Digest, Sha256};

    use std::env;
    use std::path::PathBuf;

    fn test_parse_from_file<S: AsRef<str>>(path: S, filter: Option<Regex>, num_entries: usize, sha256: S) {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let parser = ContentsParser{ filter };
            let path = PathBuf::from(path.as_ref());
            let file = File::open(path).await.expect("Failed to open dummy Contents-amd64");
            let mut hasher = Sha256::new();
            let ret = parser.parse_async(&mut Compat::new(file), &mut hasher).await;
            // for lib in &ret {
            //     println!("{: <50} {: <50} {:?}", lib.get_translated_lib_name(), lib.get_translated_dev_name(), lib);
            // }
            assert_eq!(ret.len(), num_entries);
            assert_eq!(hex::encode(hasher.finalize()), sha256.as_ref());
        });
    }

    fn test_parse_from_gz_file<S: AsRef<str>>(path: S, filter: Option<Regex>, num_entries: usize, sha256: S) {
        let rt = Runtime::new().unwrap();
        rt.block_on(async move {
            let parser = ContentsParser{ filter };
            let path = PathBuf::from(path.as_ref());
            let file = File::open(path).await.expect("Failed to open dummy Contents-amd64.gz");
            let mut hasher = Sha256::new();
            let ret = parser.parse_async_gzip(&mut Compat::new(file), &mut hasher).await;
            // for lib in &ret {
            //     println!("{: <50} {: <50} {:?}", lib.get_translated_lib_name(), lib.get_translated_dev_name(), lib);
            // }
            assert_eq!(ret.len(), num_entries);
            assert_eq!(hex::encode(hasher.finalize()), sha256.as_ref());
        });
    }

    #[test]
    fn lib_get_lib_name_libadwaitaqt1() {
        let lib = Lib::new("adwaita-qt", "libadwaitaqt", Some("1.4.0".to_string()));
        assert_eq!("libadwaitaqt1", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_lib_name_libnss3() {
        let lib = Lib::new("nss", "libnss3", None);
        assert_eq!("libnss3", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_lib_name_libiso9660pp() {
        let lib = Lib::new("libcdio", "libiso9660++", Some("0.0.0".to_string()));
        assert_eq!("libiso9660++0", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_lib_name_libiso9660() {
        let lib = Lib::new("libcdio", "libiso9660", Some("11.0.0".to_string()));
        assert_eq!("libiso9660-11", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_dev_name_libadwaitaqt1() {
        let lib = Lib::new("adwaita-qt", "libadwaitaqt", Some("1.4.0".to_string()));
        assert_eq!("libadwaitaqt-dev", lib.get_translated_dev_name());
    }

    #[test]
    fn lib_get_dev_name_libnss3() {
        let lib = Lib::new("nss", "libnss3", None);
        assert_eq!("libnss3-dev", lib.get_translated_dev_name());
    }

    #[test]
    fn lib_get_dev_name_libiso9660pp() {
        let lib = Lib::new("libcdio", "libiso9660++", Some("0.0.0".to_string()));
        assert_eq!("libiso9660++-dev", lib.get_translated_dev_name());
    }

    #[test]
    fn lib_get_dev_name_libiso9660() {
        let lib = Lib::new("libcdio", "libiso9660", Some("11.0.0".to_string()));
        assert_eq!("libiso9660-dev", lib.get_translated_dev_name());
    }

    #[test]
    fn parse_without_filter() {
        test_parse_from_file(format!("{}/tests/Contents-amd64-dummy", env::var("CARGO_MANIFEST_DIR").unwrap()), None, 8, "bced8bf932b7a007a5481bd5572abfacfb9eb16c70e243658540959337e0f769".to_string());
    }

    #[test]
    fn parse_actual_data_without_filter() {
        test_parse_from_file(format!("{}/tests/Contents-amd64", env::var("CARGO_MANIFEST_DIR").unwrap()), None, 4171, "08a533991ca1d1c4881d9bfb14dbeeb69428f25f935079b8e0c4072f1de16423".to_string());
    }

    #[test]
    fn parse_with_filter() {
        test_parse_from_file(format!("{}/tests/Contents-amd64-dummy", env::var("CARGO_MANIFEST_DIR").unwrap()), Regex::new("libnss3").ok(), 1, "bced8bf932b7a007a5481bd5572abfacfb9eb16c70e243658540959337e0f769".to_string());
    }

    #[test]
    fn parse_gzip_actual_data_without_filter() {
        test_parse_from_gz_file(format!("{}/tests/Contents-amd64.gz", env::var("CARGO_MANIFEST_DIR").unwrap()), None, 4195, "e0859f91cd9d07871e253f5c6ed0c2b6c14955d5ad3582c82133251d5adb4ff5".to_string());
    }
}
