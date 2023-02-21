use serde::{Serialize, Deserialize};
use apt_parser::Filter;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lib {
    library_name: String,
    sover: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct ContentsFilter {
    name: Vec<(String, String)>,
}

impl Lib {
    pub fn new<S: AsRef<str>>(
        library_name: S,
        sover: Vec<usize>,
    ) -> Self {
        Self {
            library_name: library_name.as_ref().replace('_', "-").to_lowercase(),
            sover,
        }
    }

    pub fn get_lib_name(&self) -> &str {
        &self.library_name
    }

    pub fn get_translated_lib_name(&self) -> String {
        let version_suffix = if self.sover.is_empty() {
            None
        } else {
            Some(self.sover[0])
        };
        let end_numeric = self.library_name.chars().last().unwrap().is_numeric();
        let lib_name = self.get_lib_name();

        match (end_numeric, version_suffix) {
            (true, Some(suffix)) => format!("{}-{}", lib_name, suffix),
            (false, Some(suffix)) => format!("{}{}", lib_name, suffix),
            _ => lib_name.to_string(),
        }
    }

    pub fn get_translated_dev_name(&self) -> String {
        format!("{}-dev", self.get_lib_name())
    }

    pub fn get_sover(&self) -> &[usize] {
        &self.sover
    }
}

impl ContentsFilter {
    fn new<S: AsRef<str>>(names: Vec<S>) -> Self {
        unimplemented!()
    }

    fn extract_libname<S: AsRef<str>>(name: S) -> String {
        let mut s = name.as_ref().trim();
        if s.ends_with("-dev") {
            s = &s[..s.len() - 4];
        }
        
        "".to_string()
    }
}

#[cfg(test)]
mod test {
    use super::Lib;

    #[test]
    fn lib_get_lib_name_libadwaitaqt1() {
        let lib = Lib::new("libadwaitaqt", vec![1, 4, 0]);
        assert_eq!("libadwaitaqt1", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_lib_name_libnss3() {
        let lib = Lib::new( "libnss3", vec![]);
        assert_eq!("libnss3", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_lib_name_libiso9660pp() {
        let lib = Lib::new("libiso9660++", vec![0, 0, 0]);
        assert_eq!("libiso9660++0", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_lib_name_libiso9660() {
        let lib = Lib::new("libiso9660", vec![11, 0, 0]);
        assert_eq!("libiso9660-11", lib.get_translated_lib_name());
    }

    #[test]
    fn lib_get_dev_name_libadwaitaqt1() {
        let lib = Lib::new("libadwaitaqt", vec![1, 4, 0]);
        assert_eq!("libadwaitaqt-dev", lib.get_translated_dev_name());
    }

    #[test]
    fn lib_get_dev_name_libnss3() {
        let lib = Lib::new( "libnss3", vec![]);
        assert_eq!("libnss3-dev", lib.get_translated_dev_name());
    }

    #[test]
    fn lib_get_dev_name_libiso9660pp() {
        let lib = Lib::new("libiso9660++", vec![0, 0, 0]);
        assert_eq!("libiso9660++-dev", lib.get_translated_dev_name());
    }

    #[test]
    fn lib_get_dev_name_libiso9660() {
        let lib = Lib::new("libiso9660", vec![11, 0, 0]);
        assert_eq!("libiso9660-dev", lib.get_translated_dev_name());
    }
}
