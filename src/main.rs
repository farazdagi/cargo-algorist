#![doc = include_str!("../README.md")]

mod cmd;

use {
    crate::cmd::MainCmd,
    anyhow::{Context, Result},
};

fn main() -> Result<()> {
    // Allow the CLI to be run as `cargo algorist` or `algorist`.
    let cmd: MainCmd = if std::env::args()
        .nth(1)
        .is_some_and(|s| s.ends_with("algorist"))
    {
        argh::cargo_from_env()
    } else {
        argh::from_env()
    };

    cmd.run().context("failed to run subcommand")
}
