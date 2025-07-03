pub mod add;
pub mod bundle;
pub mod create;
pub mod run;

use {
    add::AddProblemSubCmd,
    anyhow::Result,
    argh::FromArgs,
    bundle::BundleProblemSubCmd,
    create::CreateContestSubCmd,
    include_dir::{Dir, include_dir},
    run::RunProblemSubCmd,
    std::{fs, path::Path},
};

pub trait SubCmd {
    fn run(&self) -> anyhow::Result<()>;
}

/// The algorist CLI tool.
#[derive(FromArgs)]
#[argh(help_triggers("-h", "--help", "help"))]
pub struct MainCmd {
    #[argh(subcommand)]
    nested: Cmd,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Cmd {
    NewContest(CreateContestSubCmd),
    BundleProblem(BundleProblemSubCmd),
    AddProblem(AddProblemSubCmd),
    RunProblem(RunProblemSubCmd),
}

impl MainCmd {
    /// Run the nested command.
    pub fn run(&self) -> Result<()> {
        match &self.nested {
            Cmd::NewContest(cmd) => cmd.run(),
            Cmd::BundleProblem(cmd) => cmd.run(),
            Cmd::AddProblem(cmd) => cmd.run(),
            Cmd::RunProblem(cmd) => cmd.run(),
        }
    }
}

pub static TPL_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/tpl");
pub static RUSTFMT_TOML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/rustfmt.toml"));
pub static GITIGNORE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/.gitignore"));

pub fn copy(dir: &Dir, glob: &str, target: &Path) -> std::io::Result<()> {
    let entries = dir.find(glob).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Failed to find glob pattern '{glob}': {e}"),
        )
    })?;
    for entry in entries {
        if let Some(file) = entry.as_file() {
            let rel_path = file.path();
            let dest_path = target.join(rel_path);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(dest_path, file.contents())?;
        }
    }
    Ok(())
}

pub fn copy_to(dir: &Dir, src: &str, target: &Path) -> std::io::Result<()> {
    let file = dir
        .get_file(src)
        .unwrap_or_else(|| panic!("file should exist in template directory: {src}"));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, file.contents())
}
