use anchor_lang::prelude::*;

declare_id!("6VmnbE29VNhm3qgRtfDv2dKyCSMdT7wDFdV9bobjyiAm");

#[program]
pub mod solana_contract {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
