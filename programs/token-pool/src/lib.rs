use anchor_lang::prelude::*;
use anchor_spl::associated_token::{AssociatedToken};
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::solana_program::system_instruction;
use anchor_lang::solana_program::program::invoke;
use anchor_lang::solana_program::program::invoke_signed;

declare_id!("69CgaMBksU7F5JM2SoRsSHURxmrvYCQqrT487aHMiRhz");

const DEFAULT_REFRESH_TIME: u64 = 86400;
const NATIVE_TOKEN: &str = "SOL";
const MAX_MULTISIG_NAME_LEN: usize = 32;
const HOLE_ADDRESS: Pubkey = pubkey!("11111111111111111111111111111111");

#[program]
pub mod token_pool {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey, bridge_contract_address: Pubkey) -> Result<()> {
        require!(
            !ctx.accounts.pool_config.is_initialized,
            PoolError::AlreadyInitialized
        );
        require!(
            ctx.accounts.admin.key() == admin,
            PoolError::Unauthorized
        );

        let pool_config = &mut ctx.accounts.pool_config;
        pool_config.is_initialized = true;
        pool_config.admin = admin;
        if bridge_contract_address != Pubkey::default() {
            pool_config.bridge_contract = bridge_contract_address;
        }
        pool_config.bump = ctx.bumps.pool_config;

        Ok(())
    }

    pub fn lock(ctx: Context<Lock>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.pool.is_initialized,
            PoolError::PoolNotInitialized
        );
        require!(
            ctx.accounts.authority.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        // require!(
        //     ctx.accounts.owner.key() == ctx.accounts.pool_config.bridge_contract,
        //     PoolError::Unauthorized
        // );
        require!(
            ctx.accounts.pool_config.is_valid_bridge_pda(&ctx.accounts.authority.key, &ctx.accounts.mint.key()),
            PoolError::Unauthorized
        );

        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );

        token::transfer(cpi_ctx, amount)?;

        let pool = &mut ctx.accounts.pool;
        pool.total_amount = pool.total_amount.checked_add(amount).ok_or(PoolError::Overflow)?;

        emit!(Locked {
            amount: amount,
            from: ctx.accounts.user_token_account.key(),
            to: ctx.accounts.pool_token_account.key(),
            sender: ctx.accounts.authority.key(),
            mint: ctx.accounts.mint.key(),
        });
        
        Ok(())
    }

    pub fn lock_native_token(ctx: Context<LockNativeToken>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.authority.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        require!(
            ctx.accounts.pool_config.is_valid_native_bridge_pda(&ctx.accounts.authority.key),
            PoolError::Unauthorized
        );

        let (native_pool, _) = Pubkey::find_program_address(
            &[b"native_pool"],
            ctx.program_id
        );
        require!(
            ctx.accounts.native_pool.key() == native_pool && ctx.accounts.native_pool.owner == &System::id(),
            PoolError::InvalidNativePool
        );

        let transfer_instruction = system_instruction::transfer(
            ctx.accounts.sender.key,
            &ctx.accounts.native_pool.key(),
            amount,
        );

        invoke(
            &transfer_instruction,
            &[
                ctx.accounts.sender.to_account_info(),
                ctx.accounts.native_pool.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        emit!(Locked {
            amount: amount,
            from: ctx.accounts.sender.key(),
            to: ctx.accounts.native_pool.key(),
            sender: ctx.accounts.authority.key(),
            mint: System::id(),
        });
        
        Ok(())
    }

    pub fn release(ctx: Context<Release>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.pool.is_initialized,
            PoolError::PoolNotInitialized
        );
        require!(
            ctx.accounts.authority.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        // require!(
        //     ctx.accounts.authority.key() == ctx.accounts.pool_config.bridge_contract,
        //     PoolError::Unauthorized
        // );
        require!(
            ctx.accounts.pool_config.is_valid_bridge_pda(&ctx.accounts.authority.key, &ctx.accounts.mint.key()),
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.pool.total_amount >= amount,
            PoolError::InsufficientAmount
        );

        let seeds = &[
            b"pool",
            ctx.accounts.pool.mint.as_ref(),
            &[ctx.accounts.pool.bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );

        token::transfer(cpi_ctx, amount)?;

        let pool = &mut ctx.accounts.pool;
        pool.total_amount = pool.total_amount.checked_sub(amount).unwrap();

        emit!(Released {
            amount: amount,
            from: ctx.accounts.pool_token_account.key(),
            to: ctx.accounts.user_token_account.key(),
            sender: ctx.accounts.authority.key(),
            mint: ctx.accounts.mint.key(),
        });
        
        Ok(())
    }

    pub fn release_native_token(ctx: Context<ReleaseNativeToken>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.authority.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        // require!(
        //     ctx.accounts.authority.key() == ctx.accounts.pool_config.bridge_contract,
        //     PoolError::Unauthorized
        // );
        require!(
            ctx.accounts.pool_config.is_valid_native_bridge_pda(&ctx.accounts.authority.key),
            PoolError::Unauthorized
        );
        
        require!(
            ctx.accounts.native_pool.lamports() >= amount,
            PoolError::InsufficientAmount
        );

        let (native_pool, bump) = Pubkey::find_program_address(
            &[b"native_pool"],
            ctx.program_id
        );
        require!(
            ctx.accounts.native_pool.key() == native_pool && ctx.accounts.native_pool.owner == &System::id(),
            PoolError::InvalidNativePool
        );

        let transfer_instruction = system_instruction::transfer(
            &ctx.accounts.native_pool.key(),
            ctx.accounts.receiver.key,
            amount,
        );

        invoke_signed(
            &transfer_instruction,
            &[
                ctx.accounts.native_pool.to_account_info(),
                ctx.accounts.receiver.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[&[b"native_pool", &[bump]]],
        )?;

        emit!(Released {
            amount: amount,
            from: ctx.accounts.native_pool.key(),
            to: ctx.accounts.receiver.key(),
            sender: ctx.accounts.authority.key(),
            mint: System::id(),
        });
        
        Ok(())
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.pool.is_initialized,
            PoolError::PoolNotInitialized
        );
        require!(
            ctx.accounts.sender.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        require!(
            ctx.accounts.user_pool.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUserPool
        );

        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.sender.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );

        token::transfer(cpi_ctx, amount)?;

        let pool = &mut ctx.accounts.pool;
        pool.total_amount = pool.total_amount.checked_add(amount).ok_or(PoolError::Overflow)?;

        let user_pool = &mut ctx.accounts.user_pool;
        user_pool.user = ctx.accounts.sender.key();
        user_pool.mint = ctx.accounts.pool.mint.key();
        user_pool.bump = ctx.bumps.user_pool;
        user_pool.liquidity = user_pool.liquidity.checked_add(amount).ok_or(PoolError::Overflow)?;

        emit!(LiquidityAdded {
            mint: ctx.accounts.pool.mint.key(),
            amount: amount,
            provider: ctx.accounts.user_token_account.key(),
        });
        
        Ok(())
    }

    pub fn add_native_liquidity(ctx: Context<AddNativeLiquidity>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.sender.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );

        let (native_pool, _) = Pubkey::find_program_address(
            &[b"native_pool"],
            ctx.program_id
        );
        require!(
            ctx.accounts.native_pool.key() == native_pool && ctx.accounts.native_pool.owner == &System::id(),
            PoolError::InvalidNativePool
        );

        let transfer_instruction = system_instruction::transfer(
            ctx.accounts.sender.key,
            &ctx.accounts.native_pool.key(),
            amount,
        );

        invoke(
            &transfer_instruction,
            &[
                ctx.accounts.sender.to_account_info(),
                ctx.accounts.native_pool.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        let user_native_pool = &mut ctx.accounts.user_native_pool;
        user_native_pool.user = ctx.accounts.sender.key();
        user_native_pool.bump = ctx.bumps.user_native_pool;
        user_native_pool.liquidity = user_native_pool.liquidity.checked_add(amount).ok_or(PoolError::Overflow)?;

        emit!(LiquidityAdded {
            mint: System::id(),
            amount: amount,
            provider: ctx.accounts.sender.key(),
        });
        
        Ok(())
    }

    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.pool.is_initialized,
            PoolError::PoolNotInitialized
        );
        require!(
            ctx.accounts.sender.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        require!(
            ctx.accounts.pool.total_amount >= amount && ctx.accounts.user_pool.liquidity >= amount,
            PoolError::InsufficientAmount
        );

        let seeds = &[
            b"pool",
            ctx.accounts.pool.mint.as_ref(),
            &[ctx.accounts.pool.bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );

        token::transfer(cpi_ctx, amount)?;

        let pool = &mut ctx.accounts.pool;
        pool.total_amount = pool.total_amount.checked_sub(amount).unwrap();

        let user_pool = &mut ctx.accounts.user_pool;
        user_pool.liquidity = user_pool.liquidity.checked_sub(amount).unwrap();

        emit!(LiquidityRemoved {
            mint: ctx.accounts.pool.mint.key(),
            amount: amount,
            provider: ctx.accounts.sender.key(),
        });
        
        Ok(())
    }

    pub fn remove_native_liquidity(ctx: Context<RemoveNativeLiquidity>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.sender.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        require!(
            ctx.accounts.native_pool.lamports() >= amount && ctx.accounts.user_native_pool.liquidity >= amount,
            PoolError::InsufficientAmount
        );

        let (native_pool, bump) = Pubkey::find_program_address(
            &[b"native_pool"],
            ctx.program_id
        );
        require!(
            ctx.accounts.native_pool.key() == native_pool && ctx.accounts.native_pool.owner == &System::id(),
            PoolError::InvalidNativePool
        );

        let transfer_instruction = system_instruction::transfer(
            &ctx.accounts.native_pool.key(),
            ctx.accounts.sender.key,
            amount,
        );

        invoke_signed(
            &transfer_instruction,
            &[
                ctx.accounts.native_pool.to_account_info(),
                ctx.accounts.sender.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[&[b"native_pool", &[bump]]],
        )?;

        let user_native_pool = &mut ctx.accounts.user_native_pool;
        user_native_pool.liquidity = user_native_pool.liquidity.checked_sub(amount).unwrap();

        emit!(LiquidityRemoved {
            mint: System::id(),
            amount: amount,
            provider: ctx.accounts.sender.key(),
        });
        
        Ok(())
    }

    pub fn set_native_pool(ctx: Context<SetNativePool>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
    
        let (native_pool, bump) = Pubkey::find_program_address(
            &[b"native_pool"],
            ctx.program_id
        );
        require!(
            ctx.accounts.native_pool.key() == native_pool,
            PoolError::InvalidNativePool
        );

        let create_account_ix = system_instruction::create_account(
            &ctx.accounts.admin.key(),
            &ctx.accounts.native_pool.key(),
            Rent::get()?.minimum_balance(0), 
            0,
            &System::id(),
        );

        invoke_signed(
            &create_account_ix,
            &[
                ctx.accounts.admin.to_account_info(),
                ctx.accounts.native_pool.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[&[b"native_pool", &[bump]]],
        )?;

        emit!(NativePoolSet {
            native_pool: ctx.accounts.native_pool.key(),
            sender: ctx.accounts.admin.key(),
        });
    
        Ok(())
    }

    pub fn set_token_pool(ctx: Context<SetTokenPool>, mint: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
    
        ctx.accounts.pool.initialize(
            mint,
            ctx.bumps.pool
        );

        emit!(TokenPoolSet {
            mint: ctx.accounts.mint.key(),
            pool: ctx.accounts.pool.key(),
            pool_token_account: ctx.accounts.pool_token_account.key(),
            sender: ctx.accounts.admin.key(),
        });
    
        Ok(())
    }

    pub fn set_admin(ctx: Context<SetAdmin>, admin: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        // require!(
        //     ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
        //     PoolError::Unauthorized
        // );
        require!(
            //ctx.accounts.admin.key() == ctx.accounts.pool_config.admin && 
            ctx.accounts.pool_config.is_valid_multisig_pda(&ctx.accounts.admin.key),
            PoolError::Unauthorized
        );
        require!(
            admin != Pubkey::default(),
            PoolError::InvalidAddress
        );

        let pool_config = &mut ctx.accounts.pool_config;
        pool_config.admin = admin;
        
        Ok(())
    }

    pub fn set_bridge_contract(ctx: Context<SetBridgeContract>, bridge_contract_address: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );

        require!(
            bridge_contract_address != Pubkey::default(),
            PoolError::InvalidAddress
        );

        let pool_config = &mut ctx.accounts.pool_config;
        pool_config.bridge_contract = bridge_contract_address;
        
        Ok(())
    }

    pub fn set_multisig_contract(ctx: Context<SetMultisigContract>, multisig_name: String, multisig_contract_address: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            !multisig_name.trim().is_empty() && multisig_name.len() <= MAX_MULTISIG_NAME_LEN,
            PoolError::InvalidMultisigName
        );
        require!(
            multisig_contract_address != Pubkey::default(),
            PoolError::InvalidAddress
        );

        let pool_config = &mut ctx.accounts.pool_config;
        pool_config.multisig_name = multisig_name;
        pool_config.multisig_contract = multisig_contract_address;
        
        Ok(())
    }

    pub fn set_daily_limit_config(ctx: Context<SetDailyLimitConfig>, chain_id: u16, limit_type: u8, refresh_time: u64, daily_limit: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        let now = Clock::get()?.unix_timestamp as u64;
        msg!("set_daily_limit_config now: {}", now);
        require!(
            refresh_time % DEFAULT_REFRESH_TIME == 0 && (now >= refresh_time) & ((now - refresh_time) <= DEFAULT_REFRESH_TIME),
            PoolError::InvalidRefreshTime
        );
        require!(
            ctx.accounts.daily_limit_config.to_account_info().owner == ctx.program_id,
            PoolError::InvalidDailyLimitConfig
        );

        let config = &mut ctx.accounts.daily_limit_config;
        if config.refresh_time > 0 && now - config.refresh_time <= DEFAULT_REFRESH_TIME {
            let user_amount: u64 = config.daily_limit - config.remain_token_amount;
            config.remain_token_amount = if daily_limit <= user_amount { 0 } else { daily_limit - user_amount };
        } else {
            config.remain_token_amount = daily_limit;
        }
        
        config.chain_id = chain_id;
        config.mint = ctx.accounts.mint.key();
        config.limit_type = limit_type;
        config.refresh_time = refresh_time;
        config.daily_limit = daily_limit;
        config.bump = ctx.bumps.daily_limit_config;

        emit!(DailyLimitSet {
            chain_id: chain_id,
            mint: ctx.accounts.mint.key(),
            limit_type: limit_type,
            remain_token_amount: config.remain_token_amount,
            refresh_time: refresh_time,
            daily_limit: daily_limit,
        });
        
        Ok(())
    }

    pub fn set_rate_limit_config(ctx: Context<SetRateLimitConfig>, chain_id: u16, limit_type: u8, is_enable: bool, token_capacity: u64, rate: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.rate_limit_config.to_account_info().owner == ctx.program_id,
            PoolError::InvalidRateLimitConfig
        );

        let now = Clock::get()?.unix_timestamp as u64;
        let config = &mut ctx.accounts.rate_limit_config;
        if config.last_updated_time > 0 {
            let time_diff = now - config.last_updated_time;
            if time_diff != 0 {
                config.current_token_amount = std::cmp::min(config.token_capacity, config.current_token_amount + (time_diff * config.rate));
            }
            config.current_token_amount = std::cmp::min(token_capacity, config.token_capacity);
        } else {
            config.current_token_amount = token_capacity;
        }
        
        config.chain_id = chain_id;
        config.mint = ctx.accounts.mint.key();
        config.limit_type = limit_type;
        config.is_enable = is_enable;
        config.last_updated_time = now;
        config.token_capacity = token_capacity;
        config.rate = rate;
        config.bump = ctx.bumps.rate_limit_config;

        emit!(RateLimitSet {
            chain_id: chain_id,
            mint: ctx.accounts.mint.key(),
            limit_type: limit_type,
            is_enable: is_enable,
            last_updated_time: now,
            current_token_amount: config.current_token_amount,
            token_capacity: token_capacity,
            rate: rate,
        });

        Ok(())
    }

    pub fn consume_limit(ctx: Context<ConsumeLimit>, chain_id: u16, limit_type: u8, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.sender.is_signer,
            PoolError::MissingSignature
        );
        require!(
            amount > 0,
            PoolError::InvalidAmount
        );
        // require!(
        //     ctx.accounts.sender.key() == ctx.accounts.pool_config.bridge_contract,
        //     PoolError::Unauthorized
        // );
        require!(
            ctx.accounts.pool_config.is_valid_bridge_pda(&ctx.accounts.sender.key, &ctx.accounts.mint.key())
            || ctx.accounts.pool_config.is_valid_native_bridge_pda(&ctx.accounts.sender.key),
            PoolError::Unauthorized
        );

        let now = Clock::get()?.unix_timestamp as u64;
        let daily_limit_config = &mut ctx.accounts.daily_limit_config;
        if daily_limit_config.refresh_time > 0 {
            let count = (now - daily_limit_config.refresh_time) / DEFAULT_REFRESH_TIME;
            if count > 0 {
                daily_limit_config.refresh_time = daily_limit_config.refresh_time + (DEFAULT_REFRESH_TIME * count);
                daily_limit_config.remain_token_amount = daily_limit_config.daily_limit;
            }
            require!(
                amount <= daily_limit_config.remain_token_amount,
                PoolError::AmountExceedsDailyLimit
            );
            daily_limit_config.remain_token_amount = daily_limit_config.remain_token_amount - amount;
        }

        let rate_limit_config = &mut ctx.accounts.rate_limit_config;
        if rate_limit_config.last_updated_time > 0 && rate_limit_config.is_enable {
            let time_diff = now - rate_limit_config.last_updated_time;
            if time_diff != 0 {
                require!(
                    rate_limit_config.current_token_amount <= rate_limit_config.token_capacity,
                    PoolError::InvalidRateLimitConfig
                );
                rate_limit_config.current_token_amount = std::cmp::min(rate_limit_config.token_capacity, rate_limit_config.current_token_amount + (time_diff * rate_limit_config.rate));
                rate_limit_config.last_updated_time = now;
            }
            require!(
                amount <= rate_limit_config.token_capacity,
                PoolError::AmountExceedsTokenMaxCapacity
            );
            require!(
                amount <= rate_limit_config.current_token_amount,
                PoolError::AmountExceedsCurrentTokenAmount
            );
            rate_limit_config.current_token_amount = rate_limit_config.current_token_amount - amount;
        }

        emit!(LimitConsumed {
            chain_id: chain_id,
            mint: ctx.accounts.mint.key(),
            limit_type: limit_type,
            remain_token_amount: daily_limit_config.remain_token_amount,
            refresh_time: daily_limit_config.refresh_time,
            daily_limit: daily_limit_config.daily_limit,
            is_enable: rate_limit_config.is_enable,
            last_updated_time: rate_limit_config.last_updated_time,
            current_token_amount: rate_limit_config.current_token_amount,
            token_capacity: rate_limit_config.token_capacity,
            rate: rate_limit_config.rate,
        });

        Ok(())
    }

    pub fn init_code_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_code == 0,
            PoolError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_code = Clock::get()?.unix_timestamp as u64 + 60;

        Ok(())
    }

    pub fn init_owner_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_owner == 0,
            PoolError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_owner = Clock::get()?.unix_timestamp as u64 + 60;

        Ok(())
    }

    pub fn init_admin_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_admin == 0,
            PoolError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_admin = Clock::get()?.unix_timestamp as u64 + 60;

        Ok(())
    }

    pub fn cancel_code_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_code > 0,
            PoolError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_code = 0;

        Ok(())
    }

    pub fn cancel_owner_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_owner > 0,
            PoolError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_owner = 0;

        Ok(())
    }

    pub fn cancel_admin_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_admin > 0,
            PoolError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_admin = 0;

        Ok(())
    }

    pub fn finalize_upgrades(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.pool_config.is_initialized,
            PoolError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.pool_config.admin,
            PoolError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            PoolError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_code + ctx.accounts.upgrade.end_owner + ctx.accounts.upgrade.end_admin > 0,
            PoolError::InvalidCall
        );

        let now = Clock::get()?.unix_timestamp as u64;
        let upgrade = &mut ctx.accounts.upgrade;
        if upgrade.end_code > 0 && now >= upgrade.end_code {
            upgrade.end_code = 0;
            upgrade.code = Pubkey::default();
        }
        if upgrade.end_owner > 0 && now >= upgrade.end_owner {
            upgrade.end_owner = 0;
            upgrade.owner = HOLE_ADDRESS;
        }
        if upgrade.end_admin > 0 && now >= upgrade.end_admin {
            upgrade.end_admin = 0;
            upgrade.admin = HOLE_ADDRESS;
        }
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + PoolConfig::LEN,
        seeds = [b"pool_config"],
        bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Lock<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [b"pool", pool.mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        associated_token::mint = pool.mint,
        associated_token::authority = pool
    )]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = owner
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut, signer)]
    pub owner: Signer<'info>,
    /// CHECK: This is the authority.
    #[account(signer)]
    pub authority: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LockNativeToken<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    /// CHECK: This is the native_pool.
    #[account(mut)]
    pub native_pool: AccountInfo<'info>,

    #[account(mut, signer)]
    pub sender: Signer<'info>,
    /// CHECK: This is the authority.
    #[account(signer)]
    pub authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Release<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [b"pool", pool.mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        associated_token::mint = pool.mint,
        associated_token::authority = pool
    )]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut, signer)]
    pub authority: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ReleaseNativeToken<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    /// CHECK: This is the native_pool.
    #[account(mut)]
    pub native_pool: AccountInfo<'info>,

    /// CHECK: This is the receiver.
    #[account(mut)]
    pub receiver: AccountInfo<'info>,

    #[account(mut, signer)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        init_if_needed,
        payer = sender,
        space = 8 + UserPool::LEN,
        seeds = [b"user_pool", sender.key().as_ref(), pool.mint.key().as_ref()],
        bump,
    )]
    pub user_pool: Account<'info, UserPool>,

    #[account(
        mut,
        seeds = [b"pool", pool.mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        mut,
        associated_token::mint = pool.mint,
        associated_token::authority = pool
    )]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = sender,
        associated_token::mint = mint,
        associated_token::authority = sender
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut, signer)]
    pub sender: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddNativeLiquidity<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        init_if_needed,
        payer = sender,
        space = 8 + UserNativePool::LEN,
        seeds = [b"user_native_pool", sender.key().as_ref()],
        bump,
    )]
    pub user_native_pool: Account<'info, UserNativePool>,
    
    /// CHECK: This is the native_pool.
    #[account(mut)]
    pub native_pool: AccountInfo<'info>,

    #[account(mut, signer)]
    pub sender: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [b"user_pool", sender.key().as_ref(), pool.mint.key().as_ref()],
        bump = user_pool.bump,
    )]
    pub user_pool: Account<'info, UserPool>,

    #[account(
        mut,
        seeds = [b"pool", pool.mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        mut,
        associated_token::mint = pool.mint,
        associated_token::authority = pool
    )]
    pub pool_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut, signer)]
    pub sender: Signer<'info>,

    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveNativeLiquidity<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [b"user_native_pool", sender.key().as_ref()],
        bump = user_native_pool.bump,
    )]
    pub user_native_pool: Account<'info, UserNativePool>,

    /// CHECK: This is the native_pool.
    #[account(mut)]
    pub native_pool: AccountInfo<'info>,

    #[account(mut, signer)]
    pub sender: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetNativePool<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    /// CHECK: This is the native_pool.
    #[account(mut)]
    pub native_pool: AccountInfo<'info>,

    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetTokenPool<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + Pool::LEN,
        seeds = [b"pool", mint.key().as_ref()],
        bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        init_if_needed,
        payer = admin,
        associated_token::mint = mint,
        associated_token::authority = pool
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    #[account(mut, signer, address = pool_config.admin)]
    pub admin: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetBridgeContract<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(mut, signer, address = pool_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetMultisigContract<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(mut, signer, address = pool_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(chain_id: u16, limit_type: u8)]
pub struct SetDailyLimitConfig<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + DailyLimitConfig::LEN,
        seeds = [b"daily_limit_config", mint.key().as_ref(), &[limit_type], &chain_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub daily_limit_config: Account<'info, DailyLimitConfig>,

    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(chain_id: u16, limit_type: u8)]
pub struct SetRateLimitConfig<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + RateLimitConfig::LEN,
        seeds = [b"rate_limit_config", mint.key().as_ref(), &[limit_type], &chain_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub rate_limit_config: Account<'info, RateLimitConfig>,

    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(chain_id: u16, limit_type: u8)]
pub struct ConsumeLimit<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [b"daily_limit_config", mint.key().as_ref(), &[limit_type], &chain_id.to_le_bytes().as_ref()],
        bump = daily_limit_config.bump,
    )]
    pub daily_limit_config: Account<'info, DailyLimitConfig>,

    #[account(
        mut,
        seeds = [b"rate_limit_config", mint.key().as_ref(), &[limit_type], &chain_id.to_le_bytes().as_ref()],
        bump = rate_limit_config.bump,
    )]
    pub rate_limit_config: Account<'info, RateLimitConfig>,

    #[account(mut, signer)]
    pub sender: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DoUpgrade<'info> {
    #[account(
        mut,
        seeds = [b"pool_config"],
        bump = pool_config.bump,
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + Upgrade::LEN,
        seeds = [b"upgrade"],
        bump,
    )]
    pub upgrade: Account<'info, Upgrade>,

    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct PoolConfig {
    pub is_initialized: bool,
    pub admin: Pubkey,
    pub bridge_contract: Pubkey,
    pub multisig_name: String,
    pub multisig_contract: Pubkey,
    pub bump: u8,
}

impl PoolConfig {
    pub const LEN: usize = 1 + 32 + 32 + 36 + 32 + 1;
    fn is_valid_multisig_pda(&self, signer: &Pubkey) -> bool {
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"multisig", self.multisig_name.as_bytes()],
            &self.multisig_contract
        );
        msg!("valid multisig signer {}", signer);
        msg!("valid multisig contract {}", self.multisig_contract);
        msg!("valid multisig expected {}", expected_pda);
        signer == &expected_pda
    }
    fn is_valid_bridge_pda(&self, signer: &Pubkey, mint: &Pubkey) -> bool {
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"bridge", mint.as_ref()],
            &self.bridge_contract
        );
        msg!("valid bridge signer {}", signer);
        msg!("valid bridge contract {}", self.bridge_contract);
        msg!("valid bridge expected {}", expected_pda);
        signer == &expected_pda
    }
    fn is_valid_native_bridge_pda(&self, signer: &Pubkey) -> bool {
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"native_bridge", NATIVE_TOKEN.as_bytes()],
            &self.bridge_contract
        );
        msg!("valid native bridge signer {}", signer);
        msg!("valid native bridge contract {}", self.bridge_contract);
        msg!("valid native bridge expected {}", expected_pda);
        signer == &expected_pda
    }
}

#[account]
pub struct Pool {
    pub is_initialized: bool,
    pub mint: Pubkey,        
    pub bump: u8,        
    pub total_amount: u64,  
}

impl Pool {
    pub const LEN: usize = 1 + 32 + 1 + 8;
    pub fn initialize(&mut self, mint: Pubkey, bump: u8) {
        self.is_initialized = true;
        self.mint = mint;
        self.bump = bump;
        self.total_amount = 0;
    }
}

#[account]
pub struct UserPool {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub liquidity: u64,
    pub bump: u8,
}

impl UserPool {
    pub const LEN: usize = 32 + 32 + 8 + 1;
}

#[account]
pub struct UserNativePool {
    pub user: Pubkey,
    pub liquidity: u64,
    pub bump: u8,
}

impl UserNativePool {
    pub const LEN: usize = 32 + 8 + 1;
}

#[account]
pub struct DailyLimitConfig {
    pub chain_id: u16,
    pub mint: Pubkey,
    pub limit_type: u8,
    pub remain_token_amount: u64,
    pub refresh_time: u64,
    pub daily_limit: u64,
    pub bump: u8,
}

impl DailyLimitConfig {
    pub const LEN: usize = 2 + 32 + 1 + 8 + 8 + 8 + 1;
}

#[account]
pub struct RateLimitConfig {
    pub chain_id: u16,
    pub mint: Pubkey,
    pub limit_type: u8,
    pub is_enable: bool,
    pub last_updated_time: u64,
    pub current_token_amount: u64,
    pub token_capacity: u64,
    pub rate: u64,
    pub bump: u8,
}

impl RateLimitConfig {
    pub const LEN: usize = 2 + 32 + 1 + 1 + 8 + 8 + 8 + 8 + 1;
}

#[account]
pub struct Upgrade {
    pub end_code: u64,
    pub code: Pubkey,
    pub end_owner: u64,
    pub owner: Pubkey,
    pub end_admin: u64,
    pub admin: Pubkey,
}

impl Upgrade {
    pub const LEN: usize = 8 + 32 + 8 + 32 + 8 + 32;
}

#[event]
pub struct Locked {
    pub amount: u64,
    pub from: Pubkey,
    pub to: Pubkey,
    pub sender: Pubkey,
    pub mint: Pubkey,
}

#[event]
pub struct Released {
    pub amount: u64,
    pub from: Pubkey,
    pub to: Pubkey,
    pub sender: Pubkey,
    pub mint: Pubkey,
}

#[event]
pub struct NativePoolSet {
    pub native_pool: Pubkey,
    pub sender: Pubkey,
}

#[event]
pub struct TokenPoolSet {
    pub mint: Pubkey,
    pub pool: Pubkey,
    pub pool_token_account: Pubkey,
    pub sender: Pubkey,
}

#[event]
pub struct LiquidityAdded {
    pub mint: Pubkey,
    pub amount: u64,
    pub provider: Pubkey,
}

#[event]
pub struct LiquidityRemoved {
    pub mint: Pubkey,
    pub amount: u64,
    pub provider: Pubkey,
}

#[event]
pub struct DailyLimitSet {
    pub chain_id: u16,
    pub mint: Pubkey,
    pub limit_type: u8,
    pub remain_token_amount: u64,
    pub refresh_time: u64,
    pub daily_limit: u64,
}

#[event]
pub struct RateLimitSet {
    pub chain_id: u16,
    pub mint: Pubkey,
    pub limit_type: u8,
    pub is_enable: bool,
    pub last_updated_time: u64,
    pub current_token_amount: u64,
    pub token_capacity: u64,
    pub rate: u64,
}

#[event]
pub struct LimitConsumed {
    pub chain_id: u16,
    pub mint: Pubkey,
    pub limit_type: u8,
    pub remain_token_amount: u64,
    pub refresh_time: u64,
    pub daily_limit: u64,
    pub is_enable: bool,
    pub last_updated_time: u64,
    pub current_token_amount: u64,
    pub token_capacity: u64,
    pub rate: u64,
}

#[error_code]
pub enum PoolError {
    #[msg("Already initialized")]
    AlreadyInitialized,

    #[msg("Contract not initialized")]
    ContractNotInitialized,

    #[msg("Pool not initialized")]
    PoolNotInitialized,

    #[msg("Unauthorized access")]
    Unauthorized,

    #[msg("Invalid multisig name")]
    InvalidMultisigName,

    #[msg("Missing signature")]
    MissingSignature,

    #[msg("Invalid address")]
    InvalidAddress,

    #[msg("Invalid amount")]
    InvalidAmount,

    #[msg("Insufficient amount")]
    InsufficientAmount,

    #[msg("Invalid user pool")]
    InvalidUserPool,

    #[msg("Invalid native pool")]
    InvalidNativePool,

    #[msg("Overflow occurred")]
    Overflow,

    #[msg("Invalid refresh time")]
    InvalidRefreshTime,

    #[msg("Invalid daily limit config")]
    InvalidDailyLimitConfig,

    #[msg("Invalid rate limit config")]
    InvalidRateLimitConfig,

    #[msg("Amount exceeds daily limit amount")]
    AmountExceedsDailyLimit,

    #[msg("Amount exceeds token max capacity")]
    AmountExceedsTokenMaxCapacity,

    #[msg("Amount exceeds current token amount")]
    AmountExceedsCurrentTokenAmount,

    #[msg("Invalid upgrade")]
    InvalidUpgrade,

    #[msg("Invalid call")]
    InvalidCall,
}

