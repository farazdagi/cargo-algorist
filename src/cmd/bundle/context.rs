use {
    crate::cmd::{
        TPL_DIR,
        bundle::parsed_data::{Crates, ParsedPaths},
        copy_to,
    },
    anyhow::{Context, Result},
    std::{
        fs::{self, File},
        io::BufWriter,
        path::{Path, PathBuf},
    },
};

#[derive(Debug)]
pub struct BundlerContext {
    /// Problem ID, used to locate the source file.
    pub problem_id: String,

    /// List of crates available in the project.
    ///
    /// Basically, folder names in `crates` directory.
    /// Any import that is not from these crates will be ignored.
    pub crates: Crates,

    /// Set of used modules, collected from the binary file.
    pub used_paths: ParsedPaths,

    /// Root path of the project, in canonical form.
    pub root_path: String,

    /// Source file path, in canonical form.
    pub src: PathBuf,

    /// Destination file path, in canonical form.
    pub dst: PathBuf,

    /// Output file writer.
    /// All bundled code will be written to this file.
    pub out: BufWriter<File>,
}

impl BundlerContext {
    pub fn new(problem_id: &str) -> Result<Self> {
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
            Crates::new(Path::new("crates")).context("failed to get library crate names")?;

        Ok(Self {
            problem_id: problem_id.to_string(),
            crates,
            used_paths: ParsedPaths::new(),
            root_path: root_path.display().to_string(),
            src,
            dst,
            out,
        })
    }
}
