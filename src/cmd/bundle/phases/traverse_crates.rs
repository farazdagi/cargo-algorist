use {
    crate::cmd::bundle::{
        Bundler,
        context::BundlerContext,
        phases::{
            self,
            BunlingPhase,
            utils::{
                extract_imported_paths,
                is_pub_use,
                is_test_module,
                load_mod,
                tranform_alias_and_fqn,
            },
        },
    },
    anyhow::{Context, Result},
    std::{fs, path::PathBuf},
    syn::{parse_file, visit::Visit},
};

/// Traverses all the crates in the project, recursively processing all
/// their files.
#[derive(Default)]
pub struct TraverseCrates {
    crate_name: String,
    path: PathBuf,
    import_path: String,
}

impl BunlingPhase for TraverseCrates {}

impl<'a> Bundler<'a, TraverseCrates> {
    pub fn traverse_crates(self) -> Result<Bundler<'a, phases::ParseBinary>> {
        // For all crates in `crates` directory, start traversal of their files.
        let crates = self.ctx.crates.clone();
        for (crate_name, crate_path) in crates.into_iter() {
            let file_content = fs::read_to_string(crate_path.join("src/lib.rs")).context(
                format!("failed to read library file for crate {crate_name}"),
            )?;
            let ast = parse_file(&file_content).context(format!(
                "failed to parse library file for crate {crate_name}"
            ))?;

            FileProcessor {
                ctx: self.ctx,
                state: TraverseCrates {
                    crate_name: crate_name.clone(),
                    path: crate_path
                        .join("src")
                        .canonicalize()
                        .context("failed to canonicalize src path")?,
                    import_path: crate_name.clone(),
                },
            }
            .visit_file(&ast);
        }

        Ok(Bundler {
            ctx: self.ctx,
            state: phases::ParseBinary {},
        })
    }
}

impl TraverseCrates {
    /// Build an index of names exposed with `pub use` statements.
    ///
    /// Fully qualified names are stored along with the aliases. This allows,
    /// during the next phase, to expand all the used modules, so that they use
    /// fully qualified names.
    fn extract_pub_use_decls(&mut self, ctx: &mut BundlerContext, node: &syn::ItemUse) {
        // Ignore non-public imports. We only care about `pub use` declarations.
        if !is_pub_use(node) {
            return;
        }

        let paths = extract_imported_paths(&node.tree, Vec::new());
        for path in paths {
            if let Some(alias) = path.last() {
                let (alias, fully_qualified) =
                    tranform_alias_and_fqn(alias, &self.import_path, &path);
                ctx.used_paths.insert_pub_use_decl(&alias, &fully_qualified);
            }
        }
    }

    fn traverse_mod(&mut self, ctx: &mut BundlerContext, node: &syn::ItemMod) {
        if node.content.is_some() {
            return;
        }

        if is_test_module(node) {
            return;
        }

        let mod_name = node.ident.to_string();
        let (base_path, code) = load_mod(&self.path, &mod_name).expect("Failed to load module");

        let ast = parse_file(&code).expect("Failed to parse module file");

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
            state: TraverseCrates {
                crate_name: self.crate_name.clone(),
                path: base_path,
                import_path,
            },
        }
        .visit_file(&ast);
    }
}

/// Processes a single file, recursively descending into its modules.
struct FileProcessor<'a> {
    ctx: &'a mut BundlerContext,
    state: TraverseCrates,
}

impl<'a> FileProcessor<'a> {}

impl<'ast> Visit<'ast> for FileProcessor<'_> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.state.traverse_mod(self.ctx, node);

        syn::visit::visit_item_mod(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        self.state.extract_pub_use_decls(self.ctx, node);

        syn::visit::visit_item_use(self, node);
    }
}
