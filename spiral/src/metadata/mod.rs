use serde::{Serialize, Deserialize};

use std::ops::Deref;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardcodeTable {
    entries: HashMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LookupTable {
    entries: HashMap<String, String>,
}

impl Deref for HardcodeTable {
    type Target = HashMap<String, Vec<String>>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl HardcodeTable {
    fn unwrap(self) -> HashMap<String, Vec<String>> {
        self.entries
    }
}

impl From<HardcodeTable> for LookupTable {
    fn from(h: HardcodeTable) -> Self {
        let mut entries = HashMap::new();
        for (key, value) in h.unwrap().into_iter() {
            for name in value {
                entries.insert(name, key.clone());
            }
        }
        Self {
            entries,
        }
    }
}

// impl From<Vec<Lib>> for LookupTable {
//     fn from(l: Vec<Lib>) -> Self {
//         let mut entries = HashMap::new();
//         for lib in l {
//             for translated_name in vec![lib.get_translated_lib_name(), lib.get_translated_dev_name()] {
//                 entries.insert(translated_name, lib.get_lib_name());
//             }
//         }
//         Self {
//             entries,
//         }
//     }
// }

impl Deref for LookupTable {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl LookupTable {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn unwrap(self) -> HashMap<String, String> {
        self.entries
    }

    pub fn merge(&mut self, other: Self) {
        self.entries.extend(other.unwrap())
    }

    pub fn append_hardcode_table(&mut self, other: HardcodeTable) {
        self.merge(Self::from(other))
    }
}
