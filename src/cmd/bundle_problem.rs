use {
    crate::cmd::{SubCmd, TPL_DIR, copy_to},
    anyhow::{Context, Result},
    argh::FromArgs,
    prettyplease::unparse,
    regex::Regex,
    std::{
        collections::HashSet,
        fs::{self, File},
        io::{BufWriter, Write},
        path::{Path, PathBuf},
    },
    syn::{parse_file, parse_quote, visit::Visit, visit_mut::VisitMut},
    tap::Tap,
    toml::Value,
};

/// Bundle given problem into a single file.
#[derive(FromArgs)]
#[argh(subcommand, name = "bundle")]
pub struct BundleProblemSubCmd {
    #[argh(positional)]
    /// problem ID
    id: String,
}

impl SubCmd for BundleProblemSubCmd {
    fn run(&self) -> Result<()> {
        let mut ctx = BundlerContext::new(&self.id).context(format!(
            "failed to create bundler context for problem {}",
            self.id
        ))?;

        Bundler::new(&mut ctx)?
            .run()
            .context(format!("failed to bundle problem {}", self.id))?;

        Ok(())
    }
}

/// Represents a set of used modules and their paths.
///
/// Paths are calculated based on the used modules. Each segment in a used
/// module is part of some path. So, for example, if we have a used module
/// `algorist::foo::bar`, it means that we have three paths:
/// `/algorist`, `/algorist/foo`, and `/algorist/foo/bar`.
#[derive(Debug, Default, Clone)]
struct UsedMods {
    paths: HashSet<String>,
}

impl UsedMods {
    fn new() -> Self {
        Self {
            paths: HashSet::new(),
        }
    }

    fn insert(&mut self, segments: &[String]) {
        let segments = segments.to_vec();

        // Traverse the segments and create paths.
        let mut path = String::new();
        for segment in &segments {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(segment);
            self.paths.insert(path.clone());
        }
    }

    /// Check if path is contained in the set of used modules.
    fn contains(&self, other: &str) -> bool {
        self.paths.contains(other)
    }
}

trait BunlingPhase {}

mod phases {
    use super::*;

    pub struct ProcessBinaryFile {}

    pub struct CollectLibraryFiles {}

    pub struct ProcessLibraryFile {
        pub crate_name: String,
        pub path: PathBuf,
        pub import_path: String,
    }

    pub struct BundlingCompleted;

    impl BunlingPhase for ProcessBinaryFile {}
    impl BunlingPhase for CollectLibraryFiles {}
    impl BunlingPhase for ProcessLibraryFile {}
    impl BunlingPhase for BundlingCompleted {}
}

#[derive(Debug)]
struct BundlerContext {
    /// Problem ID, used to locate the source file.
    problem_id: String,

    /// List of crates available in the project.
    ///
    /// Basically, folder names in `crates` directory.
    /// Any import that is not from these crates will be ignored.
    crates: Vec<String>,

    /// Set of used modules, collected from the binary file.
    modules: UsedMods,

    /// Root path of the project, in canonical form.
    root_path: String,

    /// Source file path, in canonical form.
    src: PathBuf,

    /// Destination file path, in canonical form.
    dst: PathBuf,

    /// Output file writer.
    /// All bundled code will be written to this file.
    out: BufWriter<File>,
}

impl BundlerContext {
    fn new(problem_id: &str) -> Result<Self> {
        // Validate the problem ID.
        let src = PathBuf::from(format!("./src/bin/{}.rs", problem_id))
            .canonicalize()
            .context("source file for the problem is not found")?;

        // Create the destination directory if it doesn't exist.
        let bundled_dir = PathBuf::from("./bundled");
        fs::create_dir_all(bundled_dir.join("src/bin"))?;

        // Copy over `Cargo.toml` file to the bundled directory.
        // Replace the `{{EXTERNAL_CRATE}}` placeholder with an empty string.
        let cargo_toml = bundled_dir.join("Cargo.toml");
        copy_to(&TPL_DIR, "Cargo.toml.tpl", &cargo_toml)?;
        fs::write(
            &cargo_toml,
            fs::read_to_string(&cargo_toml)?.replace("{{EXTERNAL_CRATE}}", ""),
        )?;

        let dst = bundled_dir
            .join("src/bin")
            .join(format!("{}.rs", problem_id));
        let out = BufWriter::new(File::create(&dst).context("failed to create output file")?);

        let root_path = PathBuf::from("./")
            .canonicalize()
            .context("Failed to canonicalize root path")?;

        // Get the list of crates available in the project.
        let crates =
            crate_names(Path::new("crates")).context("failed to get library crate names")?;

        Ok(Self {
            problem_id: problem_id.to_string(),
            crates,
            modules: UsedMods::new(),
            root_path: root_path.display().to_string(),
            src,
            dst,
            out,
        })
    }
}

#[derive(Debug)]
struct Bundler<'a, P: BunlingPhase = phases::ProcessBinaryFile> {
    ctx: &'a mut BundlerContext,
    state: P,
}

impl<'a> Bundler<'a, phases::ProcessBinaryFile> {
    fn new(ctx: &'a mut BundlerContext) -> Result<Self> {
        Ok(Self {
            ctx,
            state: phases::ProcessBinaryFile {},
        })
    }

    fn run(self) -> Result<()> {
        self.process_binary_file()?
            .collect_library_files()?
            .complete_bundling()
    }

    fn process_binary_file(mut self) -> Result<Bundler<'a, phases::CollectLibraryFiles>> {
        let src = self.ctx.src.display().to_string();
        let dst = self.ctx.dst.display().to_string();
        println!("Bundling {src} -> {dst}");

        // Read the executable source file to find used modules.
        let file_content =
            fs::read_to_string(&self.ctx.src).context("failed to read source file")?;
        let mut ast = parse_file(&file_content).context("failed to parse source file")?;
        self.visit_file(&mut ast);

        // Write the source file -- unmodified -- to the output file.
        writeln!(self.ctx.out, "{}", unparse(&ast)).context("failed to write source file")?;

        Ok(Bundler {
            ctx: self.ctx,
            state: phases::CollectLibraryFiles {},
        })
    }

    /// Extracts used modules from the `use` tree and inserts them into the
    /// context.
    fn process_item_use(&mut self, tree: &syn::UseTree) -> Result<()> {
        use syn::{UseGlob, UseGroup, UseName, UsePath, UseRename, UseTree};

        fn extract_imported_paths(tree: &UseTree, prefix: Vec<String>) -> Vec<Vec<String>> {
            match tree {
                UseTree::Path(UsePath { ident, tree, .. }) => {
                    let mut new_prefix = prefix.clone();
                    new_prefix.push(ident.to_string());
                    extract_imported_paths(tree, new_prefix)
                }
                UseTree::Name(UseName { ident, .. }) | UseTree::Rename(UseRename { ident, .. }) => {
                    let mut path = prefix;
                    path.push(ident.to_string());
                    vec![path]
                }
                UseTree::Group(UseGroup { items, .. }) => items
                    .iter()
                    .flat_map(|item| extract_imported_paths(item, prefix.clone()))
                    .collect(),
                UseTree::Glob(UseGlob { .. }) => {
                    // If it's a glob import, we don't have specific paths, so we return
                    // the current prefix as a single path.
                    vec![prefix]
                }
            }
        }

        let paths = extract_imported_paths(tree, Vec::new());
        for path in paths {
            if path.is_empty() {
                // Skip empty paths
                continue;
            }

            // Skip paths that do not start with the known crate name.
            if !self.ctx.crates.contains(&path[0]) {
                continue;
            }

            self.ctx.modules.insert(&path);
        }

        Ok(())
    }
}

impl<'ast> Visit<'ast> for Bundler<'_, phases::ProcessBinaryFile> {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        // Ignore all imports except those from the available crates.
        if let syn::UseTree::Path(path) = &node.tree {
            if !self.ctx.crates.contains(&path.ident.to_string()) {
                return;
            }
        }

        self.process_item_use(&node.tree)
            .expect("Failed to process use tree");
    }
}

impl<'a> Bundler<'a, phases::CollectLibraryFiles> {
    fn collect_library_files(self) -> Result<Bundler<'a, phases::BundlingCompleted>> {
        // For all crates in `crates` directory, we need to check if they are used in
        // the binary, and if so, process their library files.
        let crate_names = self.ctx.crates.clone();
        for crate_name in crate_names {
            if !self.ctx.modules.contains(&crate_name) {
                println!("Ignoring unused crate: {crate_name}");
                continue;
            }

            println!("Processing crate: {crate_name:?}");
            Bundler {
                ctx: self.ctx,
                state: phases::ProcessLibraryFile {
                    crate_name: crate_name.clone(),
                    path: PathBuf::from(format!("crates/{crate_name}/src"))
                        .canonicalize()
                        .context("failed to canonicalize src path")?,
                    import_path: crate_name.clone(),
                },
            }
            .process_library_file(&crate_name)
            .context(format!(
                "failed to process library file for crate {crate_name}"
            ))?;
        }

        Ok(Bundler {
            ctx: self.ctx,
            state: phases::BundlingCompleted {},
        })
    }
}

impl<'a> Bundler<'a, phases::ProcessLibraryFile> {
    fn process_library_file(&mut self, crate_name: &str) -> Result<()> {
        // Read the library source file to expand all used modules.
        // Modules are expanded recursively.
        // Modules that are not used in the binary are ignored.
        let file_content = match fs::read_to_string(format!("crates/{}/src/lib.rs", crate_name)) {
            Ok(content) => content,
            Err(_) => {
                println!("Library file for crate {crate_name:?} not found, skipping.");
                return Ok(());
            }
        };
        let mut ast = parse_file(&file_content).context("failed to parse library file")?;
        self.visit_file_mut(&mut ast);

        // Wrap the items in a module with the main module name.
        let items = std::mem::take(&mut ast.items);
        let mod_item = syn::Item::Mod(syn::ItemMod {
            unsafety: None,
            attrs: vec![
                parse_quote!(#[allow(dead_code)]),
                parse_quote!(#[allow(unused_imports)]),
                parse_quote!(#[allow(unused_macros)]),
            ],
            vis: syn::Visibility::Inherited,
            mod_token: Default::default(),
            ident: syn::Ident::new(crate_name, proc_macro2::Span::call_site()),
            content: Some((Default::default(), items)),
            semi: None,
        });
        ast.items = vec![mod_item];

        // Write the modified AST back to the output file.
        let content = self
            .post_process_output_string(unparse(&ast))
            .context("failed to unparse and post-process output string")?;
        writeln!(self.ctx.out, "{}", content).context("failed to write bundled file")?;

        Ok(())
    }

    fn post_process_output_string(&mut self, content: String) -> Result<String> {
        // Replace `crate::` with `crate::{self.state.crate_name}::` in use paths.
        // Basically you just inject `{self.state.crate_name}::` after `crate::`.
        //
        // The reason is that we bundle crates as modules, within the binary file,
        // so we need to adjust the paths accordingly.
        let re = Regex::new(r"crate::\b").unwrap();
        let new_content = re.replace_all(&content, format!("crate::{}::", self.state.crate_name));

        Ok(new_content.into_owned())
    }

    fn is_used_in_binary(&self, node: &syn::ItemMod) -> bool {
        // If base path is not empty, prefix the module name with it.
        let mod_name = if self.state.import_path.is_empty() {
            node.ident.to_string()
        } else {
            format!("{}/{}", self.state.import_path, node.ident.to_string())
        };

        self.ctx.modules.contains(&mod_name).tap(|&res| {
            println!(
                "- Processing module: {mod_name:?} {}",
                if res { "[used]" } else { "[ignored]" }
            );
        })
    }

    fn process_item_mod_mut(&mut self, node: &mut syn::ItemMod) {
        // If the module has content, we don't need to do anything.
        if node.content.is_some() {
            return;
        }

        let mod_name = node.ident.to_string();

        // Load the module file from the source directory.
        // Module may be EITHER in the form of `src/foo.rs` or `src/foo/mod.rs`.
        // Try both, and since only one works, we can use `find` to get the first one.
        let (base_path, code): (_, String) = vec![
            format!("{}/{}.rs", self.state.path.display(), mod_name),
            format!("{}/{}/mod.rs", self.state.path.display(), mod_name),
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .map(|p| {
            let base_path = p
                .clone()
                .parent()
                .expect("Failed to get parent directory")
                .to_path_buf();
            (base_path, p)
        })
        .and_then(|(base_path, mod_path)| {
            fs::read_to_string(mod_path)
                .context("failed to read source file")
                .ok()
                .and_then(|code| Some((base_path, code)))
        })
        .expect("Module file not found");

        let mut ast = parse_file(&code)
            .context("failed to parse source file")
            .expect("Failed to parse module file");

        let import_path = base_path
            .display()
            .to_string()
            .replace(&self.ctx.root_path, "")
            .replace(
                &format!("/crates/{}/src", self.state.crate_name),
                &self.state.crate_name,
            );
        Bundler {
            ctx: self.ctx,
            state: phases::ProcessLibraryFile {
                crate_name: self.state.crate_name.clone(),
                path: base_path,
                import_path,
            },
        }
        .visit_file_mut(&mut ast);

        // Populate the module content with the parsed items.
        node.content = Some((Default::default(), ast.items));
    }
}

impl<'a> VisitMut for Bundler<'a, phases::ProcessLibraryFile> {
    fn visit_attributes_mut(&mut self, attrs: &mut Vec<syn::Attribute>) {
        // Drop all attributes that are not relevant for bundling.
        *attrs = attrs
            .drain(..)
            .filter(|attr| {
                !attr.path().is_ident("doc")
                    && !attr.path().is_ident("allow")
                    && !attr.path().is_ident("cfg")
                    && !attr.path().is_ident("warn")
            })
            .collect();
    }

    fn visit_file_mut(&mut self, file: &mut syn::File) {
        self.visit_attributes_mut(&mut file.attrs);
        for it in &mut file.items {
            self.visit_item_mut(it);
        }
    }

    fn visit_item_mut(&mut self, node: &mut syn::Item) {
        match node {
            syn::Item::Mod(item) => {
                fn is_test_module(item_mod: &syn::ItemMod) -> bool {
                    // locate `#[cfg(test)]` attribute
                    item_mod.attrs.iter().any(|attr| {
                        if attr.path().is_ident("cfg") {
                            let cfg_args: syn::Expr = attr.parse_args().unwrap();
                            if let syn::Expr::Path(syn::ExprPath { path, .. }) = cfg_args {
                                return path.is_ident("test");
                            }
                        }
                        false
                    })
                }

                // Skip test modules.
                if is_test_module(item) {
                    *node = syn::Item::Verbatim(quote::quote! {
                        /* removed by bundle_problem */
                    });
                    return;
                }

                // Skip modules that are not used in the binary.
                if !self.is_used_in_binary(item) {
                    // Remove only if module's content hasn't be populated already.
                    if item.content.is_none() {
                        *node = syn::Item::Verbatim(quote::quote! {});
                    }
                    return;
                }

                self.visit_item_mod_mut(item);
            }

            _ => {
                syn::visit_mut::visit_item_mut(self, node);
            }
        }
    }

    fn visit_item_mod_mut(&mut self, node: &mut syn::ItemMod) {
        self.visit_attributes_mut(&mut node.attrs);
        self.visit_visibility_mut(&mut node.vis);
        self.visit_ident_mut(&mut node.ident);

        self.process_item_mod_mut(node);

        if let Some(it) = &mut node.content {
            for it in &mut (it).1 {
                self.visit_item_mut(it);
            }
        }
    }
}

impl<'a> Bundler<'a, phases::BundlingCompleted> {
    fn complete_bundling(self) -> Result<()> {
        println!(
            "Problem {:?} bundled successfully into {:?}",
            self.ctx.problem_id, self.ctx.dst
        );

        Ok(())
    }
}

fn crate_names(crates_dir: &Path) -> std::io::Result<Vec<String>> {
    let mut crate_names = Vec::new();
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
                        crate_names.push(name.to_string());
                    }
                }
            }
        }
    }

    Ok(crate_names)
}
