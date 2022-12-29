use anyhow::Error;
use ar::{Builder as ArBuilder, Header as ArHeader};
use flate2::write::GzEncoder;
use flate2::Compression;
use lazy_static::lazy_static;
use sailfish::TemplateOnce;
use tar::{Builder as TarBuilder, EntryType, Header as TarHeader};
use strum::{Display, EnumString};
use sailfish::runtime::{Render, RenderError, Buffer};

use std::io::{empty, Cursor, Write};
use std::string::ToString;

#[cfg(feature = "std-systemtime")]
use std::time::{SystemTime, UNIX_EPOCH};

const DOC_DIR: &str = "usr/share/doc";

#[cfg(feature = "std-systemtime")]
lazy_static! {
    static ref TIMESTAMP: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}

#[cfg(not(feature = "std-systemtime"))]
lazy_static! {
    static ref TIMESTAMP: u64 = 0;
}

lazy_static! {
    static ref DEBIAN_BINARY: Cursor<Vec<u8>> =
        Cursor::new("2.0\n".to_string().as_bytes().to_vec());
    static ref TAR_FILE_HEADER: TarHeader = {
        let mut ret = TarHeader::new_gnu();
        ret.set_mode(0o644);
        ret.set_uid(0);
        ret.set_gid(0);
        ret.set_size(0);
        ret.set_username("root").expect("Failed to set username");
        ret.set_groupname("root").expect("Failed to set groupname");
        ret.set_mtime(*TIMESTAMP);
        ret
    };
    static ref TAR_DIR_HEADER: TarHeader = {
        let mut ret = TarHeader::new_gnu();
        ret.set_mode(0o755);
        ret.set_uid(0);
        ret.set_gid(0);
        ret.set_size(0);
        ret.set_username("root").expect("Failed to set username");
        ret.set_groupname("root").expect("Failed to set groupname");
        ret.set_mtime(*TIMESTAMP);
        ret.set_entry_type(EntryType::Directory);
        ret
    };
}

fn create_tar_file_header<S: AsRef<str>>(path: S, size: usize) -> TarHeader {
    let mut ret = TAR_FILE_HEADER.clone();
    ret.set_path(String::from(path.as_ref()))
        .expect("Failed to set tar header path");
    ret.set_size(size as u64);
    ret.set_cksum();
    ret
}

fn create_tar_path<S: AsRef<str>, W: Write>(path: S, builder: &mut TarBuilder<W>) {
    let path_segments: Vec<String> = String::from(path.as_ref())
        .split('/')
        .map(|segment| segment.to_string())
        .collect();
    for i in 0..=path_segments.len() {
        let path = "./".to_string() + &path_segments[0..i].join("/") + "/";
        let mut path_header = TAR_DIR_HEADER.clone();
        path_header
            .set_path(path)
            .expect("Failed to set tar header path");
        path_header.set_cksum();
        builder
            .append(&path_header, empty())
            .expect("Failed to append header to tar");
    }
}

fn create_ar_file_header(path: Vec<u8>, size: usize) -> ArHeader {
    let mut ret = ArHeader::new(path, size as u64);
    ret.set_mode(0o100644);
    ret
}

#[derive(Copy, Clone, Debug, Display, PartialEq, Eq, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum Architecture {
    #[strum(to_string = "amd64", serialize = "x86_64")]
    AMD64,
    #[strum(to_string = "arm64", serialize = "AArch64")]
    ARM64,
    #[strum(to_string = "loongson3")]
    LOONGSON3,
    #[strum(to_string = "ppc64el", serialize = "ppc64le")]
    PPC64EL,
    #[strum(to_string = "riscv64")]
    RISCV64,
    #[strum(to_string = "armv4")]
    ARMV4,
    #[strum(to_string = "armv6hf")]
    ARMV6HF,
    #[strum(to_string = "armv7hf")]
    ARMV7HF,
    #[strum(to_string = "i486")]
    I486,
    #[strum(to_string = "loongson2f")]
    LOONGSON2F,
    #[strum(to_string = "m68k")]
    M68K,
    #[strum(to_string = "powerpc")]
    POWERPC,
    #[strum(to_string = "ppc64")]
    PPC64,
    #[strum(to_string = "all", serialize = "noarch")]
    ALL,
}

#[derive(Debug, TemplateOnce)]
#[template(path = "control.stpl")]
struct Control {
    package: String,
    version: String,
    architecture: Architecture,
    maintainer: String,
    description: String,
    depends: Vec<String>,
}

#[derive(Debug)]
pub struct EmptyPackage(Control);

impl Render for Architecture {
    #[inline]
    fn render(&self, b: &mut Buffer) -> Result<(), RenderError> {
        self.to_string().render(b)
    }
}

impl Control {
    fn new<S: AsRef<str>>(
        package: S,
        version: S,
        architecture: Architecture,
        maintainer: S,
        description: S,
        depends: Vec<String>,
    ) -> Self {
        Self {
            package: String::from(package.as_ref()),
            version: String::from(version.as_ref()),
            architecture,
            maintainer: String::from(maintainer.as_ref()),
            description: String::from(description.as_ref()),
            depends,
        }
    }

    fn get_name(&self) -> &str {
        &self.package
    }

    fn into_string(self) -> String {
        self.render_once().expect("Failed to convert to string")
    }
}

impl EmptyPackage {
    pub fn new<S: AsRef<str>>(
        package: S,
        version: S,
        architecture: Architecture,
        maintainer: S,
        description: S,
        depends: Vec<String>,
    ) -> Self {
        Self(Control::new(
            package,
            version,
            architecture,
            maintainer,
            description,
            depends,
        ))
    }

    pub fn build(self) -> Result<Vec<u8>, Error> {
        let package_name = String::from(self.0.get_name());
        let control_data = self.0.into_string().into_bytes();

        // control.tar.gz
        let mut control_archive_builder = TarBuilder::new(GzEncoder::new(
            Cursor::new(Vec::new()),
            Compression::default(),
        ));
        let control_header = create_tar_file_header("control", control_data.len());
        control_archive_builder.append(&control_header, &*control_data)?;
        let control_archive = control_archive_builder.into_inner()?.finish()?.into_inner();
        let control_archive_size = control_archive.len();

        // data.tar.gz
        let mut data_archive_builder = TarBuilder::new(GzEncoder::new(
            Cursor::new(Vec::new()),
            Compression::default(),
        ));
        create_tar_path(
            format!("{}/{}", DOC_DIR, package_name),
            &mut data_archive_builder,
        );
        let data_archive = data_archive_builder.into_inner()?.finish()?.into_inner();
        let data_archive_size = data_archive.len();

        // Final package package
        let mut ret = ArBuilder::new(Cursor::new(Vec::new())); //, AR_IDENTIFIERS.clone());
        ret.append(
            &create_ar_file_header(b"debian-binary".to_vec(), DEBIAN_BINARY.get_ref().len()),
            DEBIAN_BINARY.clone(),
        )?;
        ret.append(
            &create_ar_file_header(b"control.tar.gz".to_vec(), control_archive_size),
            &*control_archive,
        )?;
        ret.append(
            &create_ar_file_header(b"data.tar.gz".to_vec(), data_archive_size),
            &*data_archive,
        )?;
        Ok(ret.into_inner()?.into_inner())
    }
}

#[cfg(test)]
mod deb_test {
    use super::{Control, EmptyPackage, Architecture};

    use anyhow::Error;

    use std::fs::OpenOptions;
    use std::io::{BufWriter, Write};

    #[test]
    fn parse_architecture() -> Result<(), Error> {
        let test_map = vec![
            (vec!["amd64", "AMD64", "x86_64"], Architecture::AMD64),
            (vec!["arm64", "AArch64"], Architecture::ARM64),
        ];
        for (key, value) in test_map.into_iter() {
            for name in key {
                let arch: Architecture = name.parse()?;
                assert_eq!(arch, value);
            }
        }
        Ok(())
    }

    #[test]
    fn serialize_architecture() -> Result<(), Error> {
        let test_map = vec![
            (vec!["amd64", "AMD64", "x86_64"], Architecture::AMD64),
            (vec!["arm64", "AArch64"], Architecture::ARM64),
        ];
        for (key, value) in test_map.into_iter() {
            assert_eq!(value.to_string(), key[0]);
        }
        Ok(())
    }

    #[test]
    fn create_control_no_dependencies() {
        let control = Control::new(
            "test",
            "0.0.1-0",
            Architecture::ALL,
            "Spiral Admin <admin@spiral.v2bv.net>",
            "Test control file",
            vec![],
        );
        assert_eq!(
            control.into_string(),
            r#"Package: test
Version: 0.0.1-0
Architecture: all
Maintainer: Spiral Admin <admin@spiral.v2bv.net>
Description: Test control file
"#
        )
    }

    #[test]
    fn create_control_with_dependencies() {
        let control = Control::new(
            "test",
            "0.0.1-0",
            Architecture::ALL,
            "Spiral Admin <admin@spiral.v2bv.net>",
            "Test control file",
            vec!["test1".to_string(), "test2".to_string()],
        );
        assert_eq!(
            control.into_string(),
            r#"Package: test
Version: 0.0.1-0
Architecture: all
Maintainer: Spiral Admin <admin@spiral.v2bv.net>
Description: Test control file
Depends: test1, test2
"#
        )
    }

    #[test]
    fn create_archive() {
        let package = EmptyPackage::new(
            "test",
            "0.0.1-0",
            Architecture::ALL,
            "Spiral Admin <admin@spiral.v2bv.net>",
            "Test control file",
            vec!["test1".to_string(), "test2".to_string()],
        );
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("/tmp/test.deb")
            .unwrap();
        let mut f = BufWriter::new(f);
        f.write_all(&package.build().unwrap()).unwrap();
    }
}
