//! Parser for Contents-{arch} file inside an APT repository

use nom::bytes::complete::{tag, take_while, take_while1};
use nom::character::{is_space, is_alphanumeric, is_digit};
use nom::sequence::{preceded, terminated, tuple};
use nom::branch::alt;
use nom::multi::{many0, many_m_n, separated_list0, separated_list1};
use nom::IResult;

use std::fmt;
use std::io::{Read, BufRead, BufReader};
use std::path::PathBuf;
use std::iter::Iterator;

use crate::Filter;

const PATH_SEPARATOR: &str = "/";
const SOVER_SEPARATOR: &str = ".";
const SONAME_SEPARATOR: &str = ".so";
const SECTION_SEPARATOR: &str = "/";
const LIST_SEPARATOR: &str = ",";
const NEWLINE: &str = "\n";

macro_rules! generate_iterator {
    ($name:ident, $func:ident) => {
        #[derive(Debug)]
        pub struct $name<R, F> {
            reader: BufReader<R>,
            filter: F,
        }

        impl<R: Read, F: Filter> Iterator for $name<R, F> {
            type Item = ContentsEntry;
        
            fn next(&mut self) -> Option<Self::Item> {
                let mut buf = Vec::new();
                loop {
                    if self.reader.read_until(b'\n', &mut buf).is_err() {
                        return None;
                    }
                    if buf.is_empty() {
                        return None;
                    }
                    if ! self.filter.filter_bytes(&buf) {
                        buf.clear();
                        continue;
                    }
                    if let Ok((_, Some(entry))) = $func(&buf) {
                        return Some(entry);
                    }
                    // print!("Failed to parse: {}", String::from_utf8_lossy(&buf).to_string());
                    buf.clear();
                }
            }
        }

        impl<R: Read, F: Filter> $name<R, F> {
            pub fn new(read: R, filter: F) -> Self {
                Self {
                    reader: BufReader::new(read),
                    filter,
                }
            }
        }
    };
}

/// Shared Library
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedLibrary {
    name: String,
    sover: Vec<usize>,
}

/// File
/// 
/// A file path could either be a shared library or a normal file
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum File {
    SharedLibrary(SharedLibrary),
    Normal(String),
}

/// Path inside a Contents file
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentsPath {
    parent: PathBuf,
    file: File,
}

/// Name of a package
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageName {
    area: Option<String>,
    section: Option<String>,
    name: String,
}

/// Entry inside a Contents file
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentsEntry {
    path: ContentsPath,
    packages: Vec<PackageName>,
}

generate_iterator!(ContentsIterator, take_line);
generate_iterator!(ContentsSharedLibraryIterator, take_line_so);

#[inline]
fn separator(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(is_space)(input)
}

#[inline]
fn is_section_name(chr: u8) -> bool {
    is_alphanumeric(chr) || [b'-'].contains(&chr)
}

#[inline]
fn is_package_name(chr: u8) -> bool {
    (b'a'..=b'z').contains(&chr) || (b'0'..=b'9').contains(&chr) || [b'+', b'-', b'_', b'.'].contains(&chr)
}

#[inline]
fn is_soname(chr: u8) -> bool {
    is_alphanumeric(chr) || [b'+', b'-', b'_'].contains(&chr)
}

// TODO: Accept filenames with spaces
#[inline]
fn is_file_name(chr: u8) -> bool {
    //is_alphanumeric(chr) || [b'+', b'-', b':', b'.', b'_', b'!', b'$', b'(', b')', b'@', b'~', b'{', b'}', b'#', b',', b'\'', b'%'].contains(&chr)
    ![b'\t', b'/'].contains(&chr)
}

#[inline]
fn take_path_segment(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(take_while(is_file_name), tag(PATH_SEPARATOR))(input)
}

#[inline]
fn many0_path_segments(input: &[u8]) -> IResult<&[u8], PathBuf> {
    let (i, segments) = many0(take_path_segment)(input)?;
    let path = String::from_utf8_lossy(&segments.join(&b'/')).to_string();
    Ok((i, PathBuf::from(path)))
}

#[inline]
fn sover_segment(input: &[u8]) -> IResult<&[u8], usize> {
    let (i, sover) = preceded(tag(SOVER_SEPARATOR), take_while1(is_digit))(input)?;
    Ok((i, sover.iter().fold(0, |acc, digit| {
        acc * 10 + (digit - b'0') as usize
    })))
}

#[inline]
fn many0_sover_segment(input: &[u8]) -> IResult<&[u8], Vec<usize>> {
    many0(sover_segment)(input)
}

#[inline]
fn take_file_so(input: &[u8]) -> IResult<&[u8], File> {
    let (i, (soname, sover, _)) = tuple((terminated(take_while1(is_soname), tag(SONAME_SEPARATOR)), many0_sover_segment, take_while1(is_space)))(input)?;
    Ok((i, File::so(soname, sover)))
}

#[inline]
fn take_file_else(input: &[u8]) -> IResult<&[u8], File> {
    let (i, (name, _)) = tuple((take_while(is_file_name), separator))(input)?;
    Ok((i, File::normal(name)))
}

#[inline]
fn take_file(input: &[u8]) -> IResult<&[u8], File> {
    alt((take_file_so, take_file_else))(input)
}

#[inline]
fn take_path(input: &[u8]) -> IResult<&[u8], ContentsPath> {
    let (i, (path, file)) = tuple((many0_path_segments, take_file))(input)?;
    Ok((i, ContentsPath::new(path, file)))
}

#[inline]
fn take_path_so(input: &[u8]) -> IResult<&[u8], ContentsPath> {
    let (i, (path, file)) = tuple((many0_path_segments, take_file_so))(input)?;
    Ok((i, ContentsPath::new(path, file)))
}

#[inline]
fn take_package_name(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(is_package_name)(input)
}

#[inline]
fn take_section(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(take_while1(is_section_name), tag(SECTION_SEPARATOR))(input)
}

#[inline]
fn take_sections(input: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
    many_m_n(0, 2, take_section)(input)
}

#[inline]
fn take_package(input: &[u8]) -> IResult<&[u8], PackageName> {
    let (i, (sections, name)) = tuple((take_sections, take_package_name))(input)?;
    let package = match sections.len() {
        0 => PackageName::from_bytes(None, None, name),
        1 => PackageName::from_bytes(None, Some(sections[0]), name),
        2 => PackageName::from_bytes(Some(sections[0]), Some(sections[1]), name),
        _ => unreachable!(),
    };
    Ok((i, package))
}

#[inline]
fn take_packages(input: &[u8]) -> IResult<&[u8], Vec<PackageName>> {
    preceded(separator, separated_list1(tag(LIST_SEPARATOR), take_package))(input)
}

#[inline]
pub fn take_line(input: &[u8]) -> IResult<&[u8], Option<ContentsEntry>> {
    let mut separate = input.len();
    for i in (0..input.len()).rev() {
        if is_space(input[i]) {
            separate = i;
            break;
        }
    }
    let (_, path) = take_path(&input[..=separate])?;
    let (i, packages) = take_packages(&input[separate..])?;
    Ok((i, Some(ContentsEntry::new(path, packages))))
}

#[inline]
pub fn take_line_so(input: &[u8]) -> IResult<&[u8], Option<ContentsEntry>> {
    let mut separate = input.len();
    for i in (0..input.len()).rev() {
        if is_space(input[i]) {
            separate = i;
            break;
        }
    }
    let (_, path) = take_path_so(&input[..=separate])?;
    let (i, packages) = take_packages(&input[separate..])?;
    Ok((i, Some(ContentsEntry::new(path, packages))))
}

#[inline]
pub fn parse_multiple_line(input: &[u8]) -> IResult<&[u8], Vec<Option<ContentsEntry>>> {
    separated_list0(tag(NEWLINE), take_line)(input)
}

impl fmt::Display for SharedLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}{}", self.name, SONAME_SEPARATOR)?;
        for segment in &self.sover {
            write!(f, ".{}", segment)?;
        }
        Ok(())
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            File::SharedLibrary(so) => write!(f, "{}", so),
            File::Normal(name) => write!(f, "{}", name),
        }
    }
}

impl fmt::Display for ContentsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let path = self.parent.join(self.file.to_string());
        write!(f, "{}", path.to_string_lossy())
    }
}

impl SharedLibrary {
    pub fn from_bytes(soname: &[u8], sover: Vec<usize>) -> Self {
        Self {
            name: String::from_utf8_lossy(soname).trim_end().to_string(),
            sover,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_sover(&self) -> &[usize] {
        &self.sover
    }
}

impl File {
    pub fn so(soname: &[u8], sover: Vec<usize>) -> Self {
        Self::SharedLibrary(SharedLibrary::from_bytes(soname, sover))
    }

    pub fn normal(name: &[u8]) -> Self {
        Self::Normal(String::from_utf8_lossy(name).trim_end().to_string())
    }
}

impl ContentsPath {
    pub fn new(parent: PathBuf, file: File) -> Self {
        Self {
            parent,
            file,
        }
    }
}

impl PackageName {
    pub fn from_bytes(area: Option<&[u8]>, section: Option<&[u8]>, name: &[u8]) -> Self {
        Self {
            area: area.map(|a| String::from_utf8_lossy(a).to_string()),
            section: section.map(|s| String::from_utf8_lossy(s).to_string()),
            name: String::from_utf8_lossy(name).to_string(),
        }
    }
}

impl ContentsEntry {
    pub fn new(path: ContentsPath, packages: Vec<PackageName>) -> Self {
        Self {
            path,
            packages,
        }
    }

    pub fn get_path(&self) -> &ContentsPath {
        &self.path
    }

    pub fn get_packages(&self) -> &[PackageName] {
        &self.packages
    }
}

#[cfg(test)]
mod test {
    use super::{File, ContentsEntry, SharedLibrary, ContentsPath, PackageName, ContentsIterator, ContentsSharedLibraryIterator, many0_path_segments, many0_sover_segment, take_file_so, take_file, take_path, take_line, take_package, take_packages};
    use crate::AcceptAllFilter;

    #[cfg(not(debug_assertions))]
    use flate2::read::GzDecoder;

    use std::fs;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn test_many0_path_segments() {
        assert_eq!(many0_path_segments(b"/usr/bin/bash "), Ok((&b"bash "[..], PathBuf::from("/usr/bin"))));
        assert_eq!(many0_path_segments(b"/usr/bin "), Ok((&b"bin "[..], PathBuf::from("/usr"))));
        assert_eq!(many0_path_segments(b"./usr/bin "), Ok((&b"bin "[..], PathBuf::from("./usr"))));
    }

    #[test]
    fn test_many0_sover_segment() {
        assert_eq!(many0_sover_segment(b".1.1.4 "), Ok((&b" "[..], vec![1, 1, 4])));
        assert_eq!(many0_sover_segment(b" "), Ok((&b" "[..], vec![])));
    }

    #[test]
    fn test_take_file_so() {
        assert_eq!(take_file_so(b"libnuma.so.1.1.4 "), Ok((&b""[..], File::SharedLibrary(SharedLibrary{
            name: "libnuma".to_string(),
            sover: vec![1, 1, 4],
        }))));
        assert_eq!(take_file_so(b"libnuma.so.1.1.4.5.1.4 "), Ok((&b""[..], File::SharedLibrary(SharedLibrary{
            name: "libnuma".to_string(),
            sover: vec![1, 1, 4, 5, 1, 4],
        }))));
        assert_eq!(take_file_so(b"libnuma.so "), Ok((&b""[..], File::SharedLibrary(SharedLibrary{
            name: "libnuma".to_string(),
            sover: vec![],
        }))));
        assert!(take_file_so(b"bash ").is_err());
    }

    #[test]
    fn test_take_file() {
        assert_eq!(take_file(b"libnuma.so.1.1.4 "), Ok((&b""[..], File::SharedLibrary(SharedLibrary{
            name: "libnuma".to_string(),
            sover: vec![1, 1, 4],
        }))));
        assert_eq!(take_file(b"libnuma.so.1.1.4.5.1.4 "), Ok((&b""[..], File::SharedLibrary(SharedLibrary{
            name: "libnuma".to_string(),
            sover: vec![1, 1, 4, 5, 1, 4],
        }))));
        assert_eq!(take_file(b"libnuma.so "), Ok((&b""[..], File::SharedLibrary(SharedLibrary{
            name: "libnuma".to_string(),
            sover: vec![],
        }))));
        assert_eq!(take_file(b"bash "), Ok((&b""[..], File::Normal("bash".to_string()))));
    }

    #[test]
    fn test_take_path() {
        assert_eq!(take_path(b"./usr/lib/libnuma.so.1.1.4 "), Ok((&b""[..], ContentsPath {
            parent: PathBuf::from("./usr/lib/"),
            file: File::SharedLibrary(SharedLibrary {
                name: "libnuma".to_string(),
                sover: vec![1, 1, 4],
            })
        })));
        assert_eq!(take_path(b"./usr/lib/libnuma.so "), Ok((&b""[..], ContentsPath {
            parent: PathBuf::from("./usr/lib/"),
            file: File::SharedLibrary(SharedLibrary {
                name: "libnuma".to_string(),
                sover: vec![],
            })
        })));
        assert_eq!(take_path(b"./usr/lib/libnuma.so.sign "), Ok((&b""[..], ContentsPath {
            parent: PathBuf::from("./usr/lib/"),
            file: File::normal(b"libnuma.so.sign"),
        })));
        assert_eq!(take_path(b"./usr/bin/bash "), Ok((&b""[..], ContentsPath {
            parent: PathBuf::from("./usr/bin/"),
            file: File::Normal("bash".to_string()),
        })));
    }

    #[test]
    fn test_take_package() {
        assert_eq!(take_package(b"zsh\n"), Ok((&b"\n"[..], PackageName {
            area: None,
            section: None,
            name: "zsh".to_string(),
        })));
        assert_eq!(take_package(b"shells/zsh\n"), Ok((&b"\n"[..], PackageName {
            area: None,
            section: Some("shells".to_string()),
            name: "zsh".to_string(),
        })));
        assert_eq!(take_package(b"non-free/devel/cuda\n"), Ok((&b"\n"[..], PackageName {
            area: Some("non-free".to_string()),
            section: Some("devel".to_string()),
            name: "cuda".to_string(),
        })));
    }

    #[test]
    fn test_take_packages() {
        assert_eq!(take_packages(b"shells/bash,shells/zsh\n"), Ok((&b"\n"[..], vec![
            PackageName {
                area: None,
                section: Some("shells".to_string()),
                name: "bash".to_string(),
            },
            PackageName {
                area: None,
                section: Some("shells".to_string()),
                name: "zsh".to_string(),
            }
        ]
        )));
    }

    #[test]
    fn test_take_line_normal() {
        let input = b"./usr/bin/bash   shells/bash\n";
        assert_eq!(take_line(input), Ok((&b"\n"[..], Some(ContentsEntry {
            path: ContentsPath {
                parent: PathBuf::from("./usr/bin"),
                file: File::Normal("bash".to_string()),
            },
            packages: vec![
                PackageName {
                    area: None,
                    section: Some("shells".to_string()),
                    name: "bash".to_string(),
                }
            ],
        }))));
    }

    #[test]
    fn test_take_line_so() {
        let input = b"./usr/lib/libnuma.so.1.1.4   admin/numactl\n";
        assert_eq!(take_line(input), Ok((&b"\n"[..], Some(ContentsEntry {
            path: ContentsPath {
                parent: PathBuf::from("./usr/lib"),
                file: File::SharedLibrary(SharedLibrary {
                    name: "libnuma".to_string(),
                    sover: vec![1, 1, 4],
                }),
            },
            packages: vec![
                PackageName {
                    area: None,
                    section: Some("admin".to_string()),
                    name: "numactl".to_string(),
                }
            ],
        }))));
    }

    #[test]
    fn test_sharedlibrary_to_string() {
        assert_eq!(SharedLibrary {
            name: "libnuma".into(),
            sover: vec![1, 1, 4, 5, 1, 4],
        }.to_string(), "libnuma.so.1.1.4.5.1.4");
        assert_eq!(SharedLibrary {
            name: "libnuma".into(),
            sover: vec![],
        }.to_string(), "libnuma.so");
    }

    #[test]
    fn test_file_to_string() {
        assert_eq!(File::normal(b"bash").to_string(), "bash");
        assert_eq!(File::SharedLibrary(SharedLibrary {
            name: "libnuma".into(),
            sover: vec![1, 1, 4, 5, 1, 4],
        }).to_string(), "libnuma.so.1.1.4.5.1.4");
    }

    #[test]
    fn test_content_path_to_string() {
        assert_eq!(ContentsPath::new(PathBuf::from("/usr/bin"), File::normal(b"bash")).to_string(), "/usr/bin/bash");
        assert_eq!(ContentsPath::new(PathBuf::from("/usr/lib"), File::SharedLibrary(SharedLibrary {
            name: "libnuma".into(),
            sover: vec![1, 1, 4, 5, 1, 4],
        })).to_string(), "/usr/lib/libnuma.so.1.1.4.5.1.4");
    }

    #[test]
    fn test_parser_dummy() {
        let file = fs::File::open(format!("{}/tests/Contents-amd64-dummy", env::var("CARGO_MANIFEST_DIR").unwrap())).unwrap();
        let parser = ContentsIterator::new(file, AcceptAllFilter::new());
        let result: Vec<ContentsEntry> = parser.collect();
        assert_eq!(result.len(), 19);
    }

    #[test]
    fn test_parser_dummy_so() {
        let file = fs::File::open(format!("{}/tests/Contents-amd64-dummy", env::var("CARGO_MANIFEST_DIR").unwrap())).unwrap();
        let parser = ContentsSharedLibraryIterator::new(file, AcceptAllFilter::new());
        let result: Vec<ContentsEntry> = parser.collect();
        assert_eq!(result.len(), 18);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn test_parser() {
        let fs = fs::File::open(format!("{}/tests/Contents-amd64.gz", env::var("CARGO_MANIFEST_DIR").unwrap())).unwrap();
        let parser = ContentsIterator::new(GzDecoder::new(fs), AcceptAllFilter::new());
        let result: Vec<ContentsEntry> = parser.collect();
        assert_eq!(result.len(), 4411104); // 4411104 lines total
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn test_parser_so() {
        let fs = fs::File::open(format!("{}/tests/Contents-amd64.gz", env::var("CARGO_MANIFEST_DIR").unwrap())).unwrap();
        let parser = ContentsSharedLibraryIterator::new(GzDecoder::new(fs), AcceptAllFilter::new());
        let result: Vec<ContentsEntry> = parser.collect();
        println!("{}", result.iter().map(|entry| entry.get_path().to_string()).collect::<Vec<String>>().join("\n"));
        assert_eq!(result.len(), 33174); // 4411104 lines total
    }
}
