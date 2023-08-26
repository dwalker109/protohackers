use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum INode {
    File(OsString, Vec<(usize, Uuid)>),
    Dir(OsString),
}

impl INode {
    pub fn new_file(path: &Path) -> Self {
        let node_name = path.file_name().unwrap().to_os_string();
        let filename = Uuid::new_v4();
        fs::write(format!("{REPO_BASE}/{}", filename.as_simple()), b"").ok();

        Self::File(node_name, vec![(0, filename)])
    }

    pub fn new_dir(path: &Path) -> Self {
        Self::Dir(
            path.file_name()
                .unwrap_or_else(|| OsStr::new("/"))
                .to_os_string(),
        )
    }

    pub fn update(&mut self, data: &[u8]) -> usize {
        match self {
            INode::File(_, history) => {
                let (prev_rev, prev_filename) = history.last().expect("always initialised");

                let prev_data =
                    fs::read(format!("{REPO_BASE}/{}", prev_filename.as_simple())).unwrap();
                if data == prev_data {
                    return *prev_rev;
                }

                let curr_rev = prev_rev + 1;
                let curr_filename = Uuid::new_v4();

                fs::write(format!("{REPO_BASE}/{}", curr_filename.as_simple()), data).ok();
                history.push((prev_rev + 1, curr_filename));

                curr_rev
            }
            INode::Dir(_) => panic!("cannot update a dir node"),
        }
    }
}

impl Display for INode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = match self {
            INode::File(name, history) => {
                format!(
                    "{} r{}\n",
                    name.to_string_lossy(),
                    history.last().unwrap().0
                )
            }
            INode::Dir(name) => format!("{}/ DIR\n", name.to_string_lossy()),
        };

        write!(f, "{output}")
    }
}

impl PartialOrd for INode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let l = match self {
            INode::File(n, _) => n,
            INode::Dir(n) => n,
        };

        let r = match other {
            INode::File(n, _) => n,
            INode::Dir(n) => n,
        };

        Some(l.cmp(r))
    }
}

impl Ord for INode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

static REPO_BASE: &str = "./files";

#[derive(Clone, Debug)]
pub struct Repo(Arc<RwLock<HashMap<PathBuf, INode>>>);

impl Repo {
    pub fn len(&self) -> usize {
        self.0.read().unwrap().len()
    }

    pub fn new() -> Self {
        fs::remove_dir_all(REPO_BASE).ok();
        fs::create_dir_all(REPO_BASE).ok();

        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn put(&mut self, path: &Path, data: &[u8]) -> Result<usize, crate::error::Error> {
        let chk_data = std::str::from_utf8(data);

        if chk_data.is_err()
            || chk_data
                .unwrap()
                .contains(|c: char| ![9, 10].contains(&(c as u8)) && c.is_control())
        {
            return Err(crate::error::Error::Put);
        }

        let mut parts = path.iter().fold(
            Vec::with_capacity(path.components().count()),
            |mut acc, el| {
                let pre = acc.last().cloned().unwrap_or(PathBuf::new());

                acc.push(
                    PathBuf::from(OsString::from(format!(
                        "{}/{}",
                        pre.to_string_lossy(),
                        el.to_string_lossy()
                    )))
                    .components()
                    .collect(),
                );

                acc
            },
        );

        let mut lock = self.0.write().unwrap();

        let tail = parts.pop().unwrap();

        for part in &parts {
            lock.entry(PathBuf::from(part))
                .or_insert_with_key(|k| INode::new_dir(k));
        }

        let rev = lock
            .entry(PathBuf::from(&tail))
            .or_insert_with_key(|k| INode::new_file(k))
            .update(data);

        Ok(rev)
    }

    pub fn get(&self, path: &Path, rev: usize) -> Result<(usize, String), crate::error::Error> {
        let lock = self.0.read().unwrap();
        let inode = lock.get(path).ok_or(crate::error::Error::Get)?;

        match inode {
            INode::File(_, history) => {
                let (rev, filename) = match rev {
                    usize::MAX => history.last(),
                    0 => None,
                    _ => history.iter().find(|(rev_r, _)| rev == *rev_r),
                }
                .ok_or(crate::error::Error::Get)?;

                let data = fs::read(format!("{REPO_BASE}/{}", filename.as_simple()))
                    .map_err(|_| crate::error::Error::Get)?;

                Ok((
                    *rev,
                    String::from_utf8(data).map_err(|_| crate::error::Error::Get)?,
                ))
            }
            INode::Dir(_) => unimplemented!(),
        }
    }

    pub fn list(&self, path: &Path) -> Vec<INode> {
        let expected_len = path.iter().count() + 1;

        let mut entries = self
            .0
            .read()
            .unwrap()
            .iter()
            .filter_map(|(p, i)| {
                (p.iter().count() == expected_len && p.starts_with(path)).then_some(i)
            })
            .cloned()
            .collect::<Vec<_>>();

        entries.sort();

        entries
    }
}
