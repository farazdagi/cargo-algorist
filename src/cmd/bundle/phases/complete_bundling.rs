use {
    crate::cmd::bundle::{Bundler, phases::BunlingPhase},
    anyhow::Result,
};

/// Marks the end of the bundling process.
pub struct CompleteBundling;

impl BunlingPhase for CompleteBundling {}

impl<'a> Bundler<'a, CompleteBundling> {
    pub fn complete_bundling(self) -> Result<()> {
        println!(
            "Problem {:?} bundled successfully into {:?}",
            self.ctx.problem_id, self.ctx.dst
        );

        Ok(())
    }
}
