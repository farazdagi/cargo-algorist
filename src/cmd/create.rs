use {
    crate::cmd::{GITIGNORE, RUSTFMT_TOML, SubCmd, TPL_DIR, copy, copy_to},
    anyhow::{Context, Result, anyhow},
    argh::FromArgs,
    serde_json::json,
    sha2::{Digest, Sha256},
    std::{
        collections::BTreeMap,
        fs::{self, File},
        io::{BufReader, Read, Write},
        path::{Path, PathBuf},
    },
};

const ALGORIST_VERSION: &str = "0.10";

/// Create a new contest project.
#[derive(FromArgs)]
#[argh(subcommand, name = "create")]
pub struct CreateContestSubCmd {
    #[argh(positional)]
    /// contest ID
    id: String,

    #[argh(option, short = 'p')]
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

        // Vendor dependencies using `cargo vendor`.
        self.cargo_vendor(&target_dir)
            .context("failed to run cargo vendor")?;

        println!("New contest created at {target_dir:?}");
        Ok(())
    }
}

impl CreateContestSubCmd {
    fn create_project(&self, target: &Path) -> std::io::Result<()> {
        // Copy the necessary library files for contest project.
        println!("Copying template files to the contest directory...");
        copy(&TPL_DIR, ".cargo/**/*", &target.join(""))?;
        copy_to(&TPL_DIR, "Cargo.toml.tpl", &target.join("Cargo.toml"))?;

        // Update the Cargo.toml, inject either path to crate with algorithms and data
        // structures, or select the version of `algorist` crate to use.
        println!("Injecting algorithms library crate into Cargo.toml...");
        let cargo_toml = target.join("Cargo.toml");
        let mut content = fs::read_to_string(&cargo_toml)?;
        if let Some((crate_name, crate_path)) =
            external_crate(target, self.manifest_path.as_deref())?
        {
            println!(
                "- Using external crate: {:?} ({:?})",
                crate_name, crate_path
            );
            let import_line = format!(
                "{crate_name} = {{ path = \"{}\" }}",
                // if/when `cargo vendor` supports paths, use `crate_path.to_string_lossy()`
                format!("crates/{}", crate_name)
            );
            content = content.replace("{{EXTERNAL_CRATE}}", &import_line);
        } else {
            println!("- Using `algorist` crate from crates.io.");
            content = content.replace(
                "{{EXTERNAL_CRATE}}",
                format!("algorist = \"{}\"", ALGORIST_VERSION).as_str(),
            );
        }
        fs::write(cargo_toml, content)?;

        // Copy files from root directory.
        fs::write(target.join(".gitignore"), GITIGNORE)?;
        fs::write(target.join("rustfmt.toml"), RUSTFMT_TOML)?;

        // Create files for problems a-h.
        if self.empty {
            // If `empty` flag is set, create a single `main.rs` file.
            copy_to(&TPL_DIR, "main.rs", &target.join(format!("src/main.rs")))?;
        } else {
            println!("Adding problems a-h to the contest...");
            for letter in 'a'..='h' {
                copy_to(
                    &TPL_DIR,
                    "problem.rs",
                    &target.join(format!("src/bin/{letter}.rs")),
                )?;
            }
        }

        // Create empty `inputs/{a-h}.txt` or `inputs/input.txt` (when `--empty` flag is
        // used) files.
        let inputs_dir = target.join("inputs");
        fs::create_dir_all(&inputs_dir)?;
        if self.empty {
            println!("Creating empty input file...");
            fs::write(inputs_dir.join("input.txt"), "")?;
        } else {
            println!("Creating empty input files for problems a-h...");
            for letter in 'a'..='h' {
                fs::write(target.join(format!("inputs/{letter}.txt")), "")?;
            }
        }

        Ok(())
    }

    fn cargo_vendor(&self, target: &Path) -> Result<()> {
        println!("Running `cargo vendor` to vendor dependencies...");
        let status = std::process::Command::new("cargo")
            .arg("vendor")
            .arg("crates")
            .arg("--no-delete")
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
}

/// Checks the provided manifest path, and copies external crate into the
/// contest project.
///
/// Returns the crate name and path to the external crate.
///
/// If the manifest path is not `None`, the function checks if the path exists
/// and is either a `Cargo.toml` file or a directory containing a `Cargo.toml`
/// file.
fn external_crate(
    target: &Path,
    manifest_path: Option<&str>,
) -> std::io::Result<Option<(String, PathBuf)>> {
    if let Some(manifest_path) = manifest_path {
        // Ensure that the manifest path exists.
        let manifest_path = PathBuf::from(manifest_path);
        if !manifest_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Manifest path not found: {:?}", manifest_path),
            ));
        }

        // Ensure that path either ends with `Cargo.toml` or is a directory, that
        // contains `Cargo.toml`.
        if !manifest_path.ends_with("Cargo.toml") && !manifest_path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Manifest path must be a Cargo.toml file or a directory containing Cargo.toml",
            ));
        }

        // If the manifest path is a directory, find the Cargo.toml file in it.
        let manifest_path = if manifest_path.is_dir() {
            manifest_path.join("Cargo.toml")
        } else {
            manifest_path
        };
        if !manifest_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Cargo.toml file not found at: {:?}", manifest_path),
            ));
        }

        // Once we located the `Cargo.toml` file, we can determine the crate name and
        // the path to the external crate.
        let crate_name = crate_name(&manifest_path)?;
        if let Some(parent) = manifest_path.parent() {
            let crate_path = parent.to_path_buf().canonicalize()?;

            // Copy the external crate into the contest project's `crates` directory.
            // Files are copied to `crates/<crate_name>`. Git artifacts are not copied.
            let target_crate_path = target.join("crates").join(&crate_name);
            if target_crate_path.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "External crate directory already exists: {:?}",
                        target_crate_path
                    ),
                ));
            }
            println!(
                "- Copying external crate from {:?} to {:?}",
                crate_path, target_crate_path
            );
            fs::create_dir_all(&target_crate_path)?;
            copy_crate(&crate_path, &target_crate_path)?;
            update_checksum_json(&target_crate_path)?;

            return Ok(Some((crate_name, crate_path)));
        }
    }
    Ok(None)
}

/// Given path to `Cargo.toml`, returns the crate name.
fn crate_name(path: &Path) -> std::io::Result<String> {
    use toml::Value;
    if path.exists() {
        let content = fs::read_to_string(path)?;
        if let Ok(value) = content.parse::<Value>() {
            if let Some(name) = value
                .get("package")
                .and_then(|pkg| pkg.get("name"))
                .and_then(|n| n.as_str())
            {
                return Ok(name.to_string());
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Cargo.toml does not contain a package name",
                ));
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("Cargo.toml not found at: {:?}", path),
    ))
}

const IGNORED_FILES: [&str; 3] = [".git", "target", "Cargo.lock"];

/// Copy the external crate from the source path to the target path.
///
/// Ignored files/directories: `.git`, `target`, `Cargo.lock`.
fn copy_crate(source: &Path, target: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        if let Some(file_name) = file_name.to_str() {
            if IGNORED_FILES.contains(&file_name) {
                continue;
            }
        }
        let target_path = target.join(file_name);
        if path.is_dir() {
            fs::create_dir_all(&target_path)?;
            copy_crate(&path, &target_path)?;
        } else if path.is_file() {
            fs::copy(&path, &target_path)?;
        }
    }
    Ok(())
}

/// Updates or creates `.cargo-checksum.json` in the given crate directory.
/// Skips `.cargo-checksum.json` itself.
pub fn update_checksum_json(crate_dir: &Path) -> std::io::Result<()> {
    let mut files = BTreeMap::new();

    for entry in walkdir::WalkDir::new(crate_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let rel_path = entry.path().strip_prefix(crate_dir).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Failed to strip prefix from path",
            )
        })?;
        if rel_path == Path::new(".cargo-checksum.json") {
            continue;
        }
        let mut file = BufReader::new(File::open(entry.path())?);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let hash = format!("{:x}", hasher.finalize());
        files.insert(rel_path.to_string_lossy().replace('\\', "/"), hash);
    }

    let json_obj = json!({
        "files": files,
        "package": null
    });

    let checksum_path = crate_dir.join(".cargo-checksum.json");
    let mut out = File::create(checksum_path)?;
    out.write_all(serde_json::to_string_pretty(&json_obj)?.as_bytes())?;
    Ok(())
}
