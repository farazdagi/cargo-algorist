use {
    crate::cmd::{SubCmd, TPL_DIR, copy_to},
    anyhow::{Context, Result, anyhow},
    argh::FromArgs,
    std::{fs, path::PathBuf},
};

/// Add a problem template to `src/bin/`.
#[derive(FromArgs)]
#[argh(subcommand, name = "add")]
pub struct AddProblemSubCmd {
    #[argh(positional)]
    /// problem ID
    id: String,
}

impl SubCmd for AddProblemSubCmd {
    fn run(&self) -> Result<()> {
        // The `./src` directory must be present.
        let src_dir = PathBuf::from("./")
            .canonicalize()
            .context("failed to canonicalize root directory path")?
            .join("src");

        if !src_dir.exists() {
            return Err(anyhow!("Source directory does not exist: {:?}", src_dir));
        }

        // The `src/bin` will be created if it doesn't exist.
        let bin_dir = src_dir.join("bin");
        if !bin_dir.exists() {
            fs::create_dir(&bin_dir).context("failed to create src/bin directory")?;
        }

        // Copy template file to the `src/bin` directory.
        // If the file already exists, emit an error.
        let target_file = bin_dir.join(format!("{}.rs", self.id.trim_end_matches(".rs")));
        if target_file.exists() {
            return Err(anyhow!("Problem file already exists: {:?}", target_file));
        }

        copy_to(&TPL_DIR, "problem.rs", &target_file)?;

        println!("Problem template added at {target_file:?}");

        Ok(())
    }
}
