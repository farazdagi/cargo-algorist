use {
    crate::cmd::bundle::{
        Bundler,
        context::BundlerContext,
        phases::{
            self,
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
    },
    anyhow::{Context, Result},
    prettyplease::unparse,
    regex::Regex,
    std::{fs, io::Write, path::PathBuf},
    syn::{parse_file, parse_quote, visit_mut::VisitMut},
    tap::Tap,
};

/// Recursively process all crates and their modules, expanding all used modules
/// within a single output file.
#[derive(Default)]
pub struct ExpandMods {
    pub crate_name: String,
    pub path: PathBuf,
    pub import_path: String,
}

impl BunlingPhase for ExpandMods {}

impl<'a> Bundler<'a, ExpandMods> {
    pub fn expand_mods(mut self) -> Result<Bundler<'a, phases::CompleteBundling>> {
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

            let file_content = match fs::read_to_string(crate_path.join("src/lib.rs")) {
                Ok(content) => content,
                Err(_) => {
                    println!("Library file for crate {crate_name:?} not found, skipping.");
                    continue;
                }
            };
            let mut ast = parse_file(&file_content).context("failed to parse library file")?;

            FileProcessor {
                ctx: self.ctx,
                state: ExpandMods {
                    crate_name: crate_name.clone(),
                    path: crate_path
                        .join("src")
                        .canonicalize()
                        .context("failed to canonicalize src path")?,
                    import_path: crate_name.clone(),
                },
            }
            .visit_file_mut(&mut ast);

            // Wrap the items within crate into the main module name.
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
                ident: syn::Ident::new(&crate_name, proc_macro2::Span::call_site()),
                content: Some((Default::default(), items)),
                semi: None,
            });
            ast.items = vec![mod_item];

            // Write the modified AST back to the output file.
            let content = self
                .post_process_output_string(&crate_name, unparse(&ast))
                .context("failed to unparse and post-process output string")?;
            writeln!(self.ctx.out, "{}", content).context("failed to write bundled file")?;
        }

        Ok(Bundler {
            ctx: self.ctx,
            state: phases::CompleteBundling {},
        })
    }

    fn post_process_output_string(&mut self, crate_name: &str, content: String) -> Result<String> {
        // Replace `crate::` with `crate::{self.state.crate_name}::` in use paths.
        // Basically you just inject `{self.state.crate_name}::` after `crate::`.
        //
        // The reason is that we bundle crates as modules, within the binary file,
        // so we need to adjust the paths accordingly.
        let re = Regex::new(r"crate::\b").unwrap();
        let new_content = re.replace_all(&content, format!("crate::{}::", crate_name));

        Ok(new_content.into_owned())
    }
}

impl ExpandMods {
    /// Filter out file tree items that should not be included in the final
    /// output.
    fn filter_file_items(&mut self, ctx: &mut BundlerContext, items: &mut Vec<syn::Item>) {
        let mut new_items = Vec::new();

        for item in items.drain(..) {
            match &item {
                syn::Item::Mod(item) => {
                    // Only retain modules that are used in the binary.
                    // Remove test modules.
                    if is_test_module(item) || !self.is_used_in_binary(ctx, item) {
                        // Skip test modules.
                        continue;
                    }
                }
                syn::Item::Use(item) => {
                    // Transform `pub use` declarations: only retain those that are used in the
                    // binary (and thus are available in the output file).
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
                                    tranform_alias_and_fqn(alias, &self.import_path, &path);
                                if ctx.used_paths.is_pub_use_used(&alias) {
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

    fn expand_mod(&mut self, ctx: &mut BundlerContext, node: &mut syn::ItemMod) {
        // If the module has content, we don't need to do anything.
        if node.content.is_some() {
            return;
        }

        let mod_name = node.ident.to_string();
        let (base_path, code) = load_mod(&self.path, &mod_name).expect("Failed to load module");

        let mut ast = parse_file(&code).expect("Failed to parse module file");

        let crate_src_path = ctx
            .crates
            .path(&self.crate_name)
            .expect("crate path not found")
            .join("src");
        let import_path = base_path
            .display()
            .to_string()
            .replace(&ctx.root_path, "")
            .replace(
                crate_src_path
                    .to_str()
                    .expect("failed to convert crate source path"),
                &self.crate_name,
            )
            .trim_start_matches('/')
            .to_string();
        FileProcessor {
            ctx,
            state: ExpandMods {
                crate_name: self.crate_name.clone(),
                path: base_path,
                import_path,
            },
        }
        .visit_file_mut(&mut ast);

        // Populate the module content with the parsed items.
        node.content = Some((Default::default(), ast.items));
    }

    fn is_used_in_binary(&self, ctx: &BundlerContext, node: &syn::ItemMod) -> bool {
        // If base path is not empty, prefix the module name with it.
        let mod_name = if self.import_path.is_empty() {
            node.ident.to_string()
        } else {
            format!("{}/{}", self.import_path, node.ident.to_string())
        };

        ctx.used_paths.contains_path(&mod_name).tap(|&res| {
            println!(
                "- Processing module: {mod_name:?} {}",
                if res { "[used]" } else { "[ignored]" }
            );
        })
    }
}

/// Processes a single file, recursively descending into its modules.
struct FileProcessor<'a> {
    ctx: &'a mut BundlerContext,
    state: ExpandMods,
}

impl<'a> FileProcessor<'a> {}

impl<'a> VisitMut for FileProcessor<'_> {
    fn visit_file_mut(&mut self, file: &mut syn::File) {
        self.visit_attributes_mut(&mut file.attrs);

        self.state.filter_file_items(self.ctx, &mut file.items);

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

        self.state.expand_mod(self.ctx, node);

        if let Some(it) = &mut node.content {
            for it in &mut (it).1 {
                self.visit_item_mut(it);
            }
        }
    }
}
