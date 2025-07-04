use {
    crate::cmd::bundle::{
        Bundler,
        context::BundlerContext,
        phases::{self, BunlingPhase, utils::extract_imported_paths},
    },
    anyhow::{Context, Result},
    prettyplease::unparse,
    std::{fs, io::Write},
    syn::{parse_file, visit::Visit},
};

/// Extract all used modules used in problem's binary file.
pub struct ParseBinary {}

impl BunlingPhase for ParseBinary {}

impl<'a> Bundler<'a, ParseBinary> {
    pub fn parse_binary(mut self) -> Result<Bundler<'a, phases::CollectLibraryFiles>> {
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
}

impl ParseBinary {
    /// Extracts used modules from the `use` tree and saves them for later
    /// stages.
    fn extract_used_mods(&mut self, ctx: &mut BundlerContext, node: &syn::ItemUse) {
        let paths = extract_imported_paths(&node.tree, Vec::new());
        for path in paths {
            if path.is_empty() {
                // Skip empty paths
                continue;
            }

            // Skip paths that do not start with the known crate name.
            if !ctx.crates.contains(&path[0]) {
                continue;
            }

            ctx.used_paths.insert_path(&path.join("/"));
        }
    }
}

impl<'ast> Visit<'ast> for Bundler<'_, phases::ParseBinary> {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        // Ignore all imports except those from the available crates.
        if let syn::UseTree::Path(path) = &node.tree {
            if !self.ctx.crates.contains(&path.ident.to_string()) {
                return;
            }
        }

        self.state.extract_used_mods(self.ctx, node);

        syn::visit::visit_item_use(self, node);
    }
}
