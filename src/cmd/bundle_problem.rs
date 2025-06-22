use {
    crate::cmd::SubCmd,
    anyhow::{Context, Result},
    argh::FromArgs,
    prettyplease::unparse,
    std::{
        collections::HashSet,
        fs::{self, File},
        io::{BufWriter, Write},
        path::PathBuf,
    },
    syn::{parse_file, parse_quote, visit::Visit, visit_mut::VisitMut},
    tap::Tap,
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

        Bundler1::new(&mut ctx)?
            .run()
            .context(format!("failed to bundle problem {}", self.id))?;

        Ok(())
    }
}

const MAIN_MOD: &str = "algorist";

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

    pub struct ProcessBinaryFile {
        pub used_mods: UsedMods,
    }

    pub struct ProcessLibraryFile {
        pub used_mods: UsedMods,
        pub base_path: PathBuf,
        pub base_path_relative: String,
    }

    pub struct BundlingCompleted;

    impl BunlingPhase for ProcessBinaryFile {}
    impl BunlingPhase for ProcessLibraryFile {}
    impl BunlingPhase for BundlingCompleted {}
}

#[derive(Debug)]
struct BundlerContext {
    main_mod: String,
    problem_id: String,
    root_path: String,
    src: PathBuf,
    dst: PathBuf,
    out: BufWriter<File>,
}

impl BundlerContext {
    fn new(problem_id: &str) -> Result<Self> {
        // Validate the problem ID.
        let src = PathBuf::from(format!("./src/bin/{}.rs", problem_id))
            .canonicalize()
            .context("source file for the problem is not found")?;

        // Create the destination directory if it doesn't exist.
        fs::create_dir_all(PathBuf::from("bundled"))?;
        let dst = PathBuf::from(format!("./bundled/{}.rs", problem_id));
        let out = BufWriter::new(File::create(&dst).context("failed to create output file")?);

        let root_path = PathBuf::from("src")
            .canonicalize()
            .context("Failed to canonicalize src path")?;

        Ok(Self {
            main_mod: MAIN_MOD.to_string(),
            problem_id: problem_id.to_string(),
            root_path: root_path.display().to_string(),
            src,
            dst,
            out,
        })
    }
}

#[derive(Debug)]
struct Bundler1<'a, P: BunlingPhase = phases::ProcessBinaryFile> {
    ctx: &'a mut BundlerContext,
    state: P,
}

impl<'a> Bundler1<'a, phases::ProcessBinaryFile> {
    fn new(ctx: &'a mut BundlerContext) -> Result<Self> {
        Ok(Self {
            ctx,
            state: phases::ProcessBinaryFile {
                used_mods: UsedMods::new(),
            },
        })
    }

    fn run(self) -> Result<()> {
        self.process_binary_file()?
            .process_library_file()?
            .complete_bundling()
    }

    fn process_binary_file(mut self) -> Result<Bundler1<'a, phases::ProcessLibraryFile>> {
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

        let main_mod = self.ctx.main_mod.clone();
        Ok(Bundler1 {
            ctx: self.ctx,
            state: phases::ProcessLibraryFile {
                used_mods: self.state.used_mods,
                base_path: PathBuf::from("src/algorist")
                    .canonicalize()
                    .context("failed to canonicalize src path")?,
                base_path_relative: main_mod,
            },
        })
    }

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

            // Skip paths that do not start with the main module.
            // Bundler is only interested in imports from the main module.
            if path[0] != MAIN_MOD || path.len() < 2 {
                continue;
            }

            self.state.used_mods.insert(&path[1..]);
        }

        Ok(())
    }
}

impl<'ast> Visit<'ast> for Bundler1<'_, phases::ProcessBinaryFile> {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        // Ignore all imports except those from the current crate.
        if let syn::UseTree::Path(path) = &node.tree {
            if path.ident != self.ctx.main_mod {
                return;
            }
        }

        self.process_item_use(&node.tree)
            .expect("Failed to process use tree");
    }
}

impl<'a> Bundler1<'a, phases::ProcessLibraryFile> {
    fn process_library_file(mut self) -> Result<Bundler1<'a, phases::BundlingCompleted>> {
        // Read the library source file (located in `algorist/mod.rs`) to expand all
        // used modules. Modules are expanded recursively.
        // Modules that are not used in the binary are ignored.
        let file_content = fs::read_to_string(format!("src/{}/mod.rs", self.ctx.main_mod))
            .context("failed to read library file")?;
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
            ident: syn::Ident::new(&self.ctx.main_mod, proc_macro2::Span::call_site()),
            content: Some((Default::default(), items)),
            semi: None,
        });
        ast.items = vec![mod_item];

        // Write the modified AST back to the output file.
        writeln!(self.ctx.out, "{}", unparse(&ast)).context("failed to write bundled file")?;

        Ok(Bundler1 {
            ctx: self.ctx,
            state: phases::BundlingCompleted,
        })
    }

    fn is_used_in_binary(&self, node: &syn::ItemMod) -> bool {
        // If base path is not empty, prefix the module name with it.
        let mod_name = node.ident.to_string();
        let mod_name = if self.state.base_path_relative.is_empty() {
            mod_name
        } else {
            format!("{}/{}", self.state.base_path_relative, mod_name)
                .strip_prefix('/')
                .unwrap_or(&mod_name)
                .strip_prefix("algorist/")
                .unwrap_or(&mod_name)
                .to_string()
        };

        self.state.used_mods.contains(&mod_name).tap(|&res| {
            println!(
                "- Processing module: {mod_name:?} {}",
                if res { " [used]" } else { " [ignored]" }
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
            format!("{}/{}.rs", self.state.base_path.display(), mod_name),
            format!("{}/{}/mod.rs", self.state.base_path.display(), mod_name),
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

        let base_path_relative = base_path
            .display()
            .to_string()
            .replace(&self.ctx.root_path, "");
        Bundler1 {
            ctx: self.ctx,
            state: phases::ProcessLibraryFile {
                used_mods: self.state.used_mods.clone(),
                base_path,
                base_path_relative,
            },
        }
        .visit_file_mut(&mut ast);

        // Populate the module content with the parsed items.
        node.content = Some((Default::default(), ast.items));
    }
}

impl<'a> VisitMut for Bundler1<'a, phases::ProcessLibraryFile> {
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

    fn visit_use_tree_mut(&mut self, node: &mut syn::UseTree) {
        fn fix_crate_use_path(node: &mut syn::UseTree) {
            // Replace `crate::` with `crate::algorist::` in use paths.
            // Basically you just inject `algorist::` after `crate::`.
            if let syn::UseTree::Path(path) = node {
                if path.ident == "crate" {
                    if let syn::UseTree::Path(inner_path) = &*path.tree {
                        if inner_path.ident == MAIN_MOD {
                            // Already rewritten, do nothing
                            return;
                        }
                    }

                    let new_path = syn::UseTree::Path(syn::UsePath {
                        ident: syn::Ident::new(MAIN_MOD, path.ident.span()),
                        colon2_token: Default::default(),
                        tree: Box::new(*path.tree.clone()),
                    });
                    path.tree = Box::new(new_path);
                }
            }
        }

        fix_crate_use_path(node);

        syn::visit_mut::visit_use_tree_mut(self, node);
    }
}

impl<'a> Bundler1<'a, phases::BundlingCompleted> {
    fn complete_bundling(self) -> Result<()> {
        println!(
            "Problem {:?} bundled successfully into {:?}",
            self.ctx.problem_id, self.ctx.dst
        );

        Ok(())
    }
}
