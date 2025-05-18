use anchor_lang::Discriminator;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use token_pool::cpi::accounts::SetAdmin as PoolSetAdmin;
use token_pool::program::TokenPool;
use token_pool::{self, PoolConfig};
use std::collections::HashSet;

declare_id!("CNW4xWTtahYUCSZwdziDaKysN6ms7giod6RP9wZTewNk");

const MAX_INSTRUCTIONS: usize = 16;
const MAX_ACCOUNTS: usize = 6;
const MAX_DATA: usize = 100;

#[program]
pub mod multisig {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey) -> Result<()> {
        require!(
            !ctx.accounts.multisig_config.is_initialized,
            MultisigError::AlreadyInitialized
        );
        require!(
            ctx.accounts.admin.key() == admin,
            MultisigError::Unauthorized
        );

        let multisig_config = &mut ctx.accounts.multisig_config;
        multisig_config.is_initialized = true;
        multisig_config.admin = admin;
        multisig_config.bump = ctx.bumps.multisig_config;

        Ok(())
    }

    pub fn set_admin(ctx: Context<SetAdmin>, admin: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.multisig_config.is_initialized,
            MultisigError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.multisig_config.admin,
            MultisigError::Unauthorized
        );

        require!(
            admin != Pubkey::default(),
            MultisigError::InvalidAddress
        );

        let multisig_config = &mut ctx.accounts.multisig_config;
        msg!("Admin changed from {} to {}", multisig_config.admin, admin);
        multisig_config.admin = admin;
        
        Ok(())
    }

    pub fn create_multisig(ctx: Context<CreateMultisig>, name: String, owners: Vec<Pubkey>, threshold: u8) -> Result<()> {
        require!(
            ctx.accounts.multisig_config.is_initialized,
            MultisigError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.multisig_config.admin,
            MultisigError::Unauthorized
        );
        require!(
            threshold > 0 && threshold <= owners.len() as u8, 
            MultisigError::InvalidThreshold
        );
        require!(
            owners.len() >= 1, 
            MultisigError::InvalidOwners
        );
        
        let owners_set: HashSet<_> = owners.iter().collect();
        require!(
            owners_set.len() == owners.len(),
            MultisigError::DuplicateOwner
        );
        
        let multisig = &mut ctx.accounts.multisig;
        multisig.name = name;
        multisig.owners = owners;
        multisig.threshold = threshold;
        multisig.bump = ctx.bumps.multisig;

        emit!(MultisigCreated {
            name: multisig.name.clone(),
            owners: multisig.owners.clone(),
            threshold: threshold,
        });

        Ok(())
    }

    pub fn update_multisig(ctx: Context<UpdateMultisig>, owners: Vec<Pubkey>, threshold: u8) -> Result<()> {
        require!(
            ctx.accounts.multisig_config.is_initialized,
            MultisigError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.multisig_config.admin,
            MultisigError::Unauthorized
        );
        require!(
            threshold > 0 && threshold <= owners.len() as u8, 
            MultisigError::InvalidThreshold
        );
        require!(
            owners.len() >= 1 && owners.len() <= ctx.accounts.multisig.owners.len(), 
            MultisigError::InvalidOwners
        );
        
        let owners_set: HashSet<_> = owners.iter().collect();
        require!(
            owners_set.len() == owners.len(),
            MultisigError::DuplicateOwner
        );
        
        let multisig = &mut ctx.accounts.multisig;
        multisig.owners = owners;
        multisig.threshold = threshold;

        emit!(MultisigUpdated {
            name: multisig.name.clone(),
            owners: multisig.owners.clone(),
            threshold: threshold,
        });

        Ok(())
    }

    pub fn create_proposal(ctx: Context<CreateProposal>, instructions: Vec<InstructionData>) -> Result<()> {
        require!(
            ctx.accounts.multisig_config.is_initialized,
            MultisigError::ContractNotInitialized
        );
        require!(
            ctx.accounts.proposer.key() == ctx.accounts.multisig_config.admin,
            MultisigError::Unauthorized
        );
        require!(
            instructions.len() >= 1 && instructions.len() <= MAX_INSTRUCTIONS, 
            MultisigError::InvalidInstructions
        );

        let proposal = &mut ctx.accounts.proposal;
        proposal.multisig = ctx.accounts.multisig.key();
        proposal.instructions = instructions;
        proposal.signers = vec![false; ctx.accounts.multisig.owners.len()];
        proposal.executed = false;

        emit!(ProposalCreated {
            multisig: ctx.accounts.multisig.name.clone(),
            instructions: proposal.instructions.clone(),
        });

        Ok(())
    }

    pub fn approve_proposal(ctx: Context<ApproveProposal>) -> Result<()> {
        require!(
            ctx.accounts.multisig_config.is_initialized,
            MultisigError::ContractNotInitialized
        );

        let proposal = &mut ctx.accounts.proposal;
        let multisig = &ctx.accounts.multisig;

        let signer_index = multisig.owners
            .iter()
            .position(|&owner| owner == ctx.accounts.signer.key())
            .ok_or(MultisigError::Unauthorized)?;

        require!(
            signer_index < proposal.signers.len(), 
            MultisigError::InvalidOwnerIndex
        );
        
        require!(
            !proposal.signers[signer_index],
            MultisigError::AlreadyApproved
        );
        
        proposal.signers[signer_index] = true;

        emit!(ProposalApproved {
            signer: ctx.accounts.signer.key(),
        });

        Ok(())
    }

    pub fn execute_proposal(ctx: Context<ExecuteProposal>) -> Result<()> {
        require!(
            ctx.accounts.multisig_config.is_initialized,
            MultisigError::ContractNotInitialized
        );
        require!(
            ctx.accounts.proposer.key() == ctx.accounts.multisig_config.admin,
            MultisigError::Unauthorized
        );

        let proposal = &mut ctx.accounts.proposal;
        let multisig = &ctx.accounts.multisig;

        msg!("execute_proposal executed: {}", proposal.executed);
        require!(
            !proposal.executed, 
            MultisigError::AlreadyExecuted
        );
        
        let approved_count = proposal.signers.iter().filter(|&&s| s).count();
        msg!("execute_proposal approved_count: {}", approved_count);
        require!(
            approved_count >= multisig.threshold as usize, 
            MultisigError::NotEnoughSigners
        );

        for ix_data in &proposal.instructions {
            let accounts: Vec<AccountMeta> = ix_data
                .accounts
                .iter()
                .map(|meta| AccountMeta {
                    pubkey: meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect();

            let ix = Instruction {
                program_id: ix_data.program_id,
                // accounts,
                accounts: vec![
                    AccountMeta::new(accounts[0].pubkey, false),
                    AccountMeta::new_readonly(accounts[1].pubkey, true),
                ],
                data: ix_data.data.clone(),
            };

            if ix.program_id == token_pool::ID {
                let new_admin = parse_set_admin_instruction(&ix_data.data)
                    .ok_or(MultisigError::InvalidInstructions)?;

                msg!("execute_proposal admin, new_admin:{}, {}", ctx.accounts.pool_config.admin, new_admin);
                let cpi_accounts = PoolSetAdmin {
                    pool_config: ctx.accounts.pool_config.to_account_info(),
                    admin: ctx.accounts.multisig_pda.clone(),
                    system_program: ctx.accounts.system_program.to_account_info()
                };
                let seeds = &[
                    b"multisig",
                    multisig.name.as_bytes(),
                    &[multisig.bump],
                ];
                let signer = &[&seeds[..]];
                let cpi_ctx = CpiContext::new_with_signer(
                    ctx.accounts.pool_program.to_account_info(), 
                    cpi_accounts,
                    signer
                );
                token_pool::cpi::set_admin(cpi_ctx, new_admin)?;
            }
        }

        proposal.executed = true;

        emit!(ProposalExecuted {
            executed: true,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 1 + 32 + 1,
        seeds = [b"multisig_config"],
        bump,
    )]
    pub multisig_config: Account<'info, MultisigConfig>,

    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    #[account(
        mut,
        seeds = [b"multisig_config"],
        bump = multisig_config.bump,
    )]
    pub multisig_config: Account<'info, MultisigConfig>,

    #[account(mut, signer, address = multisig_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(name: String, owners: Vec<Pubkey>)]
pub struct CreateMultisig<'info> {
    #[account(
        mut,
        seeds = [b"multisig_config"],
        bump = multisig_config.bump,
    )]
    pub multisig_config: Account<'info, MultisigConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + 4 + name.len() + 4 + (32 * owners.len()) + 1 + 1,
        seeds = [b"multisig", name.as_bytes()],
        bump,
    )]
    pub multisig: Account<'info, Multisig>,
    #[account(mut, signer, address = multisig_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateMultisig<'info> {
    #[account(
        mut,
        seeds = [b"multisig_config"],
        bump = multisig_config.bump,
    )]
    pub multisig_config: Account<'info, MultisigConfig>,

    #[account(
        mut,
        seeds = [b"multisig", multisig.name.as_bytes()],
        bump = multisig.bump,
    )]
    pub multisig: Account<'info, Multisig>,
    #[account(mut, signer, address = multisig_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(instructions: Vec<InstructionData>)]
pub struct CreateProposal<'info> {
    #[account(
        mut,
        seeds = [b"multisig_config"],
        bump = multisig_config.bump,
    )]
    pub multisig_config: Account<'info, MultisigConfig>,

    #[account(
        init,
        payer = proposer,
        space = 8 
            + 32
            + 4
            + (32 + 4 + (34 * MAX_ACCOUNTS) + 4 + MAX_DATA) * instructions.len()
            + 4 
            + multisig.owners.len()
            + 1,
        seeds = [b"proposal", multisig.key().as_ref()],
        bump,
    )]
    pub proposal: Account<'info, Proposal>,
    #[account(mut, signer)]
    pub proposer: Signer<'info>,
    #[account(
        seeds = [b"multisig", multisig.name.as_bytes()],
        bump = multisig.bump,
    )]
    pub multisig: Account<'info, Multisig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApproveProposal<'info> {
    #[account(
        mut,
        seeds = [b"multisig_config"],
        bump = multisig_config.bump,
    )]
    pub multisig_config: Account<'info, MultisigConfig>,

    #[account(mut, has_one = multisig)]
    pub proposal: Account<'info, Proposal>,
    #[account(
        seeds = [b"multisig", multisig.name.as_bytes()],
        bump = multisig.bump,
    )]
    pub multisig: Account<'info, Multisig>,
    #[account(mut, signer)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteProposal<'info> {
    #[account(
        mut,
        seeds = [b"multisig_config"],
        bump = multisig_config.bump,
    )]
    pub multisig_config: Account<'info, MultisigConfig>,

    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
    #[account(
        seeds = [b"multisig", multisig.name.as_bytes()],
        bump = multisig.bump,
    )]
    pub multisig: Account<'info, Multisig>,
    /// CHECK: This is the PDA for admin, validated via seeds and bump.
    #[account(
        mut,
        seeds = [b"multisig", multisig.name.as_bytes()],
        bump = multisig.bump,
    )]
    pub multisig_pda: AccountInfo<'info>,
    #[account(mut, signer)]
    pub proposer: Signer<'info>,
    #[account(mut)]
    pub pool_config: Account<'info, PoolConfig>,
    pub pool_program: Program<'info, TokenPool>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct MultisigConfig {
    pub is_initialized: bool,
    pub admin: Pubkey,
    pub bump: u8,
}

#[account]
pub struct Multisig {
    pub name: String,
    pub owners: Vec<Pubkey>,
    pub threshold: u8,
    pub bump: u8,
}

#[account]
pub struct Proposal {
    pub multisig: Pubkey,
    pub instructions: Vec<InstructionData>,
    pub signers: Vec<bool>,
    pub executed: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InstructionData {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMetaData>,
    pub data: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AccountMetaData {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SetAdminArgs {
    pub admin: Pubkey,
}

fn parse_set_admin_instruction(data: &[u8]) -> Option<Pubkey> {
    let expected_discriminator = token_pool::instruction::SetAdmin::discriminator();
    if data.len() < 8 || &data[0..8] != expected_discriminator {
        return None;
    }
    
    let mut data_slice = &data[8..];
    SetAdminArgs::deserialize(&mut data_slice).ok().map(|args| args.admin)
}

#[event]
pub struct MultisigCreated {
    pub name: String, 
    pub owners: Vec<Pubkey>, 
    pub threshold: u8,
}

#[event]
pub struct MultisigUpdated {
    pub name: String, 
    pub owners: Vec<Pubkey>, 
    pub threshold: u8,
}

#[event]
pub struct ProposalCreated {
    pub multisig: String, 
    pub instructions: Vec<InstructionData>,
}

#[event]
pub struct ProposalApproved {
    pub signer: Pubkey, 
}

#[event]
pub struct ProposalExecuted {
    pub executed: bool, 
}

#[error_code]
pub enum MultisigError {
    #[msg("Already initialized")]
    AlreadyInitialized,

    #[msg("Contract not initialized")]
    ContractNotInitialized,

    #[msg("Unauthorized")]
    Unauthorized,

    #[msg("Invalid address")]
    InvalidAddress,

    #[msg("Threshold must be between 1 and owners length")]
    InvalidThreshold,

    #[msg("Owners list cannot be empty")]
    InvalidOwners,
    
    #[msg("Duplicate owner")]
    DuplicateOwner,

    #[msg("Invalid instructions")]
    InvalidInstructions,

    #[msg("Invalid owner index")]
    InvalidOwnerIndex,

    #[msg("Proposal already approved")]
    AlreadyApproved,

    #[msg("Proposal already executed")]
    AlreadyExecuted,

    #[msg("Not enough signers")]
    NotEnoughSigners,
}

