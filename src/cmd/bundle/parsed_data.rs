use {
    std::{
        collections::{HashMap, HashSet},
        fs,
        path::{Path, PathBuf},
    },
    toml::Value,
};

/// Parsed path data extracted during the first phase, where both binary file
/// and libraries are recursively processed, and all used paths, aliases and
/// other information, required for next phase, is collected.
#[derive(Debug, Default, Clone)]
pub struct ParsedPaths {
    /// Set of paths that are used in the binary file.
    ///
    /// Paths are calculated based on the used modules. Each segment in a used
    /// module is part of some path. So, for example, if we have a used module
    /// `algorist::foo::bar`, it means that we register three paths:
    /// `/algorist`, `/algorist/foo`, and `/algorist/foo/bar`.
    paths: HashSet<String>,

    /// This is a map of fully qualified names for `pub use` declarations.
    /// The key is alias name, and the value is the fully qualified name.
    pub_use_decls: HashMap<String, String>,

    /// If an aliased name is used in the binary file, it will be marked as
    /// `true`, and its corresponding `pub use` declaration, will remain in the
    /// final output. Otherwise, it will be removed (since its module is also
    /// omitted).
    pub_use_used: HashSet<String>,
}

impl ParsedPaths {
    pub fn new() -> Self {
        Self {
            paths: HashSet::new(),
            pub_use_decls: HashMap::new(),
            pub_use_used: HashSet::new(),
        }
    }

    pub fn insert_path(&mut self, path: &str) {
        println!("Registering path: {}", path);
        let segments = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect::<Vec<_>>();

        // Full path is added immediately, so that we can check if the path is already
        // inserted.
        self.paths.insert(segments.join("/"));

        // Traverse the segments and create paths.
        let mut path = String::new();
        for segment in &segments {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(segment);

            let cur_path = path.clone();
            self.paths.insert(cur_path.clone());

            // See if the current path is an alias created with `pub use`
            if let Some(fully_qualified) = self.pub_use_decls.get(&cur_path) {
                // If it is, we need to insert the fully qualified name as well, if it is not
                // already inserted.
                if !self.paths.contains(fully_qualified) {
                    self.insert_path(&fully_qualified.clone());
                }
                // Mark item as used, so that its `pub use` declaration and the corresponding
                // module will be included in the final output.
                self.pub_use_used.insert(cur_path);
            }
        }
    }

    /// Check if path is contained in the set of used modules.
    pub fn contains_path(&self, other: &str) -> bool {
        self.paths.contains(other)
    }

    /// Insert a `pub use` declaration into the set of used modules.
    pub fn insert_pub_use_decl(&mut self, alias: &str, fully_qualified: &str) {
        self.pub_use_decls
            .insert(alias.to_string(), fully_qualified.to_string());
    }

    /// Whether the `pub use` declaration used in the binary file.
    pub fn is_pub_use_used(&self, alias: &str) -> bool {
        self.pub_use_used.contains(alias)
    }
}

/// Set of crates available in the project.
#[derive(Debug, Clone)]
pub struct Crates(HashMap<String, PathBuf>);

impl Crates {
    /// Create a new `Crates` instance by scanning the specified directory for
    /// `Cargo.toml` files, extracting crate names, and storing their paths.
    ///
    /// Normally, this directory is `crates` in the project root.
    pub fn new(crates_dir: &Path) -> std::io::Result<Crates> {
        let mut crates = Self(HashMap::new());
        for entry in fs::read_dir(crates_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let cargo_toml = path.join("Cargo.toml");
                if cargo_toml.exists() {
                    let content = fs::read_to_string(cargo_toml)?;
                    if let Ok(value) = content.parse::<Value>() {
                        if let Some(name) = value
                            .get("package")
                            .and_then(|pkg| pkg.get("name"))
                            .and_then(|n| n.as_str())
                        {
                            crates.push(name, path);
                        }
                    }
                }
            }
        }
        Ok(crates)
    }

    pub fn push(&mut self, name: &str, path: PathBuf) {
        self.0.insert(name.replace("-", "_"), path);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    pub fn path(&self, name: &str) -> Option<&PathBuf> {
        self.0.get(name)
    }

    pub fn into_iter(self) -> impl Iterator<Item = (String, PathBuf)> {
        self.0.into_iter()
    }
}
