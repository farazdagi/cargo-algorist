mod context;
mod parsed_data;
mod phases;

use {
    crate::cmd::{SubCmd, bundle::context::BundlerContext},
    anyhow::{Context, Result},
    argh::FromArgs,
    phases::{
        BunlingPhase,
        utils::{
            extract_imported_paths,
            flatten_imported_paths,
            is_pub_use,
            is_test_module,
            load_mod,
            tranform_alias_and_fqn,
        },
    },
    prettyplease::unparse,
    regex::Regex,
    std::{fs, io::Write},
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

        Bundler::new(&mut ctx)?
            .traverse_crates()?
            .process_binary_file()?
            .collect_library_files()?
            .complete_bundling()
    }
}

#[derive(Debug)]
struct Bundler<'a, P: BunlingPhase = phases::TraverseCrates> {
    ctx: &'a mut BundlerContext,
    state: P,
}

impl<'a> Bundler<'a, phases::TraverseCrates> {
    fn new(ctx: &'a mut BundlerContext) -> Result<Self> {
        Ok(Self {
            ctx,
            state: phases::TraverseCrates::default(),
        })
    }
}

impl<'a> Bundler<'a, phases::ProcessBinaryFile> {
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

            self.ctx.used_paths.insert_path(&path.join("/"));
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
    fn collect_library_files(self) -> Result<Bundler<'a, phases::CompleteBundling>> {
        // For all crates in `crates` directory, we need to check if they are used in
        // the binary, and if so, process their library files.
        let crates = self.ctx.crates.clone();
        for (crate_name, crate_path) in crates.into_iter() {
            if !self.ctx.used_paths.contains_path(&crate_name) {
                println!("Ignoring unused crate: {crate_name}");
                continue;
            }

            println!(
                "Processing crate: {crate_name:?} ({})",
                crate_path.display()
            );
            Bundler {
                ctx: self.ctx,
                state: phases::ProcessLibraryFile {
                    crate_name: crate_name.clone(),
                    path: crate_path
                        .join("src")
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
            state: phases::CompleteBundling {},
        })
    }
}

impl<'a> Bundler<'a, phases::ProcessLibraryFile> {
    fn process_library_file(&mut self, crate_name: &str) -> Result<()> {
        // Read the library source file to expand all used modules.
        // Modules are expanded recursively.
        // Modules that are not used in the binary are ignored.
        let crate_path = self
            .ctx
            .crates
            .path(crate_name)
            .context(format!("crate {crate_name} not found"))?;
        let file_content = match fs::read_to_string(crate_path.join("src/lib.rs")) {
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

        self.ctx.used_paths.contains_path(&mod_name).tap(|&res| {
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
        let (base_path, code) =
            load_mod(&self.state.path, &mod_name).expect("Failed to load module");

        let mut ast = parse_file(&code).expect("Failed to parse module file");

        let crate_src_path = self
            .ctx
            .crates
            .path(&self.state.crate_name)
            .expect("crate path not found")
            .join("src");
        let import_path = base_path
            .display()
            .to_string()
            .replace(&self.ctx.root_path, "")
            .replace(
                crate_src_path
                    .to_str()
                    .expect("failed to convert crate source path"),
                &self.state.crate_name,
            )
            .trim_start_matches('/')
            .to_string();
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

    fn filter_file_items(&mut self, items: &mut Vec<syn::Item>) {
        let mut new_items = Vec::new();

        for item in items.drain(..) {
            match &item {
                syn::Item::Mod(item) => {
                    // Only retain modules that are used in the binary.
                    // Remove test modules.
                    if is_test_module(item) || !self.is_used_in_binary(item) {
                        // Skip test modules.
                        continue;
                    }
                }
                syn::Item::Use(item) => {
                    // Ignore non-public imports. We only care about `pub use` declarations.
                    if is_pub_use(item) {
                        // Expand group into individual uses
                        let use_items = flatten_imported_paths(&item.tree, vec![]);

                        // Filter out unused `pub use` declarations.
                        for use_item in use_items {
                            if let Some(path) =
                                extract_imported_paths(&use_item.tree, Vec::new()).first()
                            {
                                let alias =
                                    path.last().expect("Path must have at least one segment");
                                let (alias, _fully_qualified) =
                                    tranform_alias_and_fqn(alias, &self.state.import_path, &path);
                                if self.ctx.used_paths.is_pub_use_used(&alias) {
                                    new_items.push(syn::Item::Use(use_item));
                                }
                            }
                        }
                        continue;
                    }
                }
                _ => {}
            }
            new_items.push(item);
        }
        *items = new_items;
    }
}

impl<'a> VisitMut for Bundler<'a, phases::ProcessLibraryFile> {
    fn visit_file_mut(&mut self, file: &mut syn::File) {
        self.visit_attributes_mut(&mut file.attrs);

        self.filter_file_items(&mut file.items);
        for it in &mut file.items {
            self.visit_item_mut(it);
        }
    }

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
