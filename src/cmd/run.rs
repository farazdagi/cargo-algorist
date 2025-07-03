use {
    crate::cmd::SubCmd,
    anyhow::{Context, Result},
    argh::FromArgs,
    std::{fs, path::PathBuf, process},
};

/// Run a given problem using the `cargo run` command.
#[derive(FromArgs)]
#[argh(subcommand, name = "run")]
pub struct RunProblemSubCmd {
    #[argh(switch, short = 'i')]
    /// read input from `inputs/{id}.txt` file, if it exists
    from_file: bool,

    #[argh(positional)]
    /// problem ID
    id: String,
}

impl SubCmd for RunProblemSubCmd {
    fn run(&self) -> Result<()> {
        let id = self.id.trim_end_matches(".rs");
        if self.from_file {
            let inputs_dir = PathBuf::from("inputs");
            let input_file = inputs_dir.join(format!("{}.txt", self.id.trim_end_matches(".rs")));
            if input_file.exists() {
                println!("Running problem {id:?} with input from {input_file:?}",);
                println!(
                    "Executing: cargo run --bin {id} -- < {}",
                    input_file.display()
                );
                let input = fs::File::open(input_file)?;
                process::Command::new("cargo")
                    .arg("run")
                    .arg("--bin")
                    .arg(id)
                    .stdin(process::Stdio::from(input))
                    .status()
                    .context("failed to run cargo command")?;
                return Ok(());
            }
        }

        // By default, run the problem without input redirection.
        println!("Running problem {id:?} without input redirection");
        println!("Executing: cargo run --bin {id}");
        process::Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg(id)
            .status()
            .context("failed to run cargo command")?;

        Ok(())
    }
}
