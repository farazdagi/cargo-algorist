mod context;
mod parsed_data;
mod phases;

use {
    crate::cmd::{SubCmd, bundle::context::BundlerContext},
    anyhow::{Context, Result},
    argh::FromArgs,
    phases::BunlingPhase,
};

/// Bundle given problem into a single file.
#[derive(FromArgs)]
#[argh(subcommand, name = "bundle")]
pub struct BundleProblemSubCmd {
    #[argh(positional)]
    /// problem ID
    id: String,
}

impl SubCmd for BundleProblemSubCmd {
    fn run(&self) -> Result<()> {
        let mut ctx = BundlerContext::new(&self.id).context(format!(
            "failed to create bundler context for problem {}",
            self.id
        ))?;

        Bundler::new(&mut ctx)?
            .traverse_crates()?
            .parse_binary()?
            .expand_mods()?
            .complete_bundling()
    }
}

#[derive(Debug)]
struct Bundler<'a, P: BunlingPhase = phases::TraverseCrates> {
    ctx: &'a mut BundlerContext,
    state: P,
}

impl<'a> Bundler<'a> {
    fn new(ctx: &'a mut BundlerContext) -> Result<Self> {
        Ok(Self {
            ctx,
            state: phases::TraverseCrates::default(),
        })
    }
}
