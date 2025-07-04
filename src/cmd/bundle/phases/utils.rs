use {
    anyhow::{Context, Result},
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

pub fn is_test_module(item_mod: &syn::ItemMod) -> bool {
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

pub fn is_pub_use(item: &syn::ItemUse) -> bool {
    matches!(item.vis, syn::Visibility::Public(_))
}

/// Load a module file from the source directory.
///
/// Return a tuple containing the base path of the module and its source code.
pub fn load_mod(base_path: &Path, mod_name: &str) -> Result<(PathBuf, String)> {
    // Load the module file from the source directory.
    // Module may be EITHER in the form of `src/foo.rs` or `src/foo/mod.rs`.
    // Try both, and since only one works, we can use `find` to get the first one.
    vec![
        format!("{}/{}.rs", base_path.display(), mod_name),
        format!("{}/{}/mod.rs", base_path.display(), mod_name),
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
    .context("Module file not found")
}

pub fn tranform_alias_and_fqn(
    alias: &str,
    import_path: &str,
    segments: &[String],
) -> (String, String) {
    if segments.is_empty() {
        return (alias.to_string(), import_path.to_string());
    }

    let alias = format!("{}/{}", import_path, alias);
    let fully_qualified = if segments[0] != "std" {
        format!("{}/{}", import_path, segments.join("/"))
    } else {
        segments.join("/")
    };
    (alias, fully_qualified)
}

pub fn extract_imported_paths(tree: &syn::UseTree, prefix: Vec<String>) -> Vec<Vec<String>> {
    use syn::{UseGlob, UseGroup, UseName, UsePath, UseRename, UseTree};
    match tree {
        UseTree::Path(UsePath { ident, tree, .. }) => {
            let mut new_prefix = prefix.clone();
            new_prefix.push(ident.to_string());
            extract_imported_paths(tree, new_prefix)
        }
        UseTree::Name(UseName { ident, .. }) | UseTree::Rename(UseRename { rename: ident, .. }) => {
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

pub fn flatten_imported_paths(tree: &syn::UseTree, prefix: Vec<syn::UseTree>) -> Vec<syn::ItemUse> {
    use syn::{UseGroup, UseTree};

    fn wrap(segments: Vec<syn::UseTree>, last: syn::UseTree) -> syn::ItemUse {
        let tree = segments
            .into_iter()
            .rev()
            .fold(last, |tree, segment| match segment {
                UseTree::Path(path) => {
                    let new_path = syn::UsePath {
                        ident: path.ident,
                        colon2_token: path.colon2_token,
                        tree: Box::new(tree),
                    };
                    UseTree::Path(new_path)
                }
                UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => segment,
                UseTree::Group(group) => {
                    panic!("Unexpected group in flatten_imported_paths: {:?}", group)
                }
            });
        syn::ItemUse {
            attrs: Vec::new(),
            vis: syn::Visibility::Public(syn::parse_quote!(pub)),
            use_token: syn::token::Use {
                span: proc_macro2::Span::call_site(),
            },
            leading_colon: None,
            tree,
            semi_token: syn::token::Semi {
                spans: [proc_macro2::Span::call_site()],
            },
        }
    }

    match tree {
        UseTree::Path(path) => {
            let mut new_prefix = prefix.clone();
            new_prefix.push(tree.clone());
            flatten_imported_paths(&path.tree, new_prefix)
        }
        UseTree::Name(_) | UseTree::Rename(_) | UseTree::Glob(_) => {
            vec![wrap(prefix, tree.clone())]
        }
        UseTree::Group(UseGroup { items, .. }) => items
            .iter()
            .flat_map(|item| flatten_imported_paths(item, prefix.clone()))
            .collect(),
    }
}
