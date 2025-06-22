use {
    crate::cmd::{GITIGNORE, RUSTFMT_TOML, SRC_DIR, SubCmd, TPL_DIR, copy, copy_to},
    anyhow::{Context, Result, anyhow},
    argh::FromArgs,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

/// Create a new contest project.
#[derive(FromArgs)]
#[argh(subcommand, name = "create")]
pub struct CreateContestSubCmd {
    #[argh(positional)]
    /// contest ID
    id: String,

    #[argh(option)]
    /// path to `Cargo.toml` file (contains base algorithms and data structures
    /// project)
    manifest_path: Option<String>,

    #[argh(switch)]
    /// no problems will be added to the contest, use `add` command to add
    /// problems later
    empty: bool,
}

impl SubCmd for CreateContestSubCmd {
    fn run(&self) -> Result<()> {
        let target_dir = PathBuf::from("./")
            .canonicalize()
            .context("failed to canonicalize root directory path")?
            .join(&self.id);

        // Ensure that the root directory does not already exist.
        // Create "src" directory for the contest (if it doesn't exist).
        let src_dir = target_dir.join("src");
        if target_dir.exists() || src_dir.exists() {
            return Err(anyhow!("Directory already exists: {:?}", target_dir));
        }
        fs::create_dir_all(src_dir)?;

        // Copy template files into the contest directory.
        self.create_project(&target_dir)
            .context("failed to copy template files")?;

        self.cargo_vendor(&target_dir)
            .context("failed to run cargo vendor")?;

        println!("New contest created at {target_dir:?}");
        Ok(())
    }
}

impl CreateContestSubCmd {
    fn create_project(&self, target: &Path) -> std::io::Result<()> {
        if let Some(manifest_path) = &self.manifest_path {
            // Ensure that the manifest path exists.
            let manifest_path = PathBuf::from(manifest_path);
            if !manifest_path.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Manifest file not found: {:?}", manifest_path),
                ));
            }
            unimplemented!("Using a custom manifest path is not yet implemented.");
            return Ok(());
        }

        // Copy the necessary library files for contest project.
        println!("Copying template files to the contest directory...");
        copy(&TPL_DIR, ".cargo/**/*", &target.join(""))?;
        copy_to(&TPL_DIR, "Cargo.toml.tpl", &target.join("Cargo.toml"))?;

        // Copy files from root directory.
        fs::write(target.join(".gitignore"), GITIGNORE)?;
        fs::write(target.join("rustfmt.toml"), RUSTFMT_TOML)?;

        // Create files for problems a-h.
        if !self.empty {
            println!("Adding problems a-h to the contest...");
            for letter in 'a'..='h' {
                copy_to(
                    &TPL_DIR,
                    "problem.rs",
                    &target.join(format!("src/bin/{letter}.rs")),
                )?;
            }
        }

        Ok(())
    }

    fn cargo_vendor(&self, target: &Path) -> Result<()> {
        println!("Running `cargo vendor` to vendor dependencies...");
        let status = std::process::Command::new("cargo")
            .arg("vendor")
            .arg("crates")
            .arg("--quiet")
            .current_dir(target)
            .status()
            .context("failed to run cargo vendor")?;
        if !status.success() {
            return Err(anyhow!("cargo vendor failed with status: {}", status));
        }
        println!(
            "Dependencies vendored successfully: {:?}.",
            target.join("crates")
        );
        Ok(())
    }

    fn manifest_path(&self) -> Option<PathBuf> {
        self.manifest_path
            .as_ref()
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
    }
}
