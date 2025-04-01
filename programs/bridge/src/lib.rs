use anchor_lang::prelude::*;
use anchor_spl::associated_token::{AssociatedToken};
use anchor_spl::token::{Mint, Token};
use token_pool::cpi::accounts::{ConsumeLimit, Lock, LockNativeToken, Release};
use token_pool::program::TokenPool;
use solana_program::keccak;

declare_id!("Gc6gUHfuf4sfS9DHtQXoLPtyKJnVbDWK1Xcqrcf7nWgV");

const MAX_MULTISIG_NAME_LEN: usize = 32;
const MAX_SYMBOL_LEN: usize = 16;
const MAX_TOKEN_LIST_LEN: usize = 16;
const MAX_CROSS_CHAIN_LIST_LEN: usize = 3;
const MAX_RECEIPT_HASH_LEN: usize = 32;
const SOLANA_CHAIN_ID: u16 = 1;
const DEFAULT_RECEIPT_INDEX: u64 = 0;
const NATIVE_TOKEN: &str = "SOL";
const HOLE_ADDRESS: Pubkey = pubkey!("11111111111111111111111111111111");

#[program]
pub mod bridge {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey, pause_controller: Pubkey, token_pool_contract_address: Pubkey, ramp_contract_address: Pubkey) -> Result<()> {
        require!(
            !ctx.accounts.bridge_config.is_initialized,
            BridgeError::AlreadyInitialized
        );
        require!(
            ctx.accounts.admin.key() == admin,
            BridgeError::Unauthorized
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.is_initialized = true;
        bridge_config.admin = admin;
        bridge_config.is_contract_pause = false;
        if pause_controller != Pubkey::default() {
            bridge_config.pause_controller = pause_controller;
        }
        if token_pool_contract_address != Pubkey::default() {
            bridge_config.token_pool_contract = token_pool_contract_address;
        }
        if ramp_contract_address != Pubkey::default() {
            bridge_config.ramp_contract = ramp_contract_address;
        }
        bridge_config.bump = ctx.bumps.bridge_config;

        Ok(())
    }

    pub fn create_receipt(ctx: Context<CreateReceipt>, target_chain_id: u16, symbol: [u8; MAX_SYMBOL_LEN], target_address: String, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            !ctx.accounts.bridge_config.is_contract_pause,
            BridgeError::ContractAlreadyPaused
        );
        let symbol_len: usize = symbol.iter().position(|&x| x == 0).unwrap_or(MAX_SYMBOL_LEN);
        require!(
            symbol_len <= MAX_SYMBOL_LEN,
            BridgeError::InvalidSymbol
        );
        require!(
            !ctx.accounts.bridge_config.token_whitelist.is_empty() 
            && ctx.accounts.bridge_config.token_whitelist.iter().any(|t| t.symbol == symbol && t.chain_id == target_chain_id),
            BridgeError::NotFoundInWhitelist
        );
        require!(
            amount > 0,
            BridgeError::InvalidAmount
        );

        let cpi_accounts = ConsumeLimit {
            pool_config: ctx.accounts.pool_config.clone(),
            daily_limit_config: ctx.accounts.daily_limit_config.clone(),
            rate_limit_config: ctx.accounts.rate_limit_config.clone(),
            sender: ctx.accounts.bridge.to_account_info(),
            mint: ctx.accounts.mint.clone(),
            system_program: ctx.accounts.system_program.to_account_info()
        };
        let seeds = &[
            b"bridge", 
            ctx.accounts.bridge.mint.as_ref(),
            &[ctx.accounts.bridge.bump],
        ];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.pool_program.to_account_info(), 
            cpi_accounts,
            signer
        );
        token_pool::cpi::consume_limit(cpi_ctx, target_chain_id, 0, amount)?;

        let cpi_accounts_lock = Lock {
            pool_config: ctx.accounts.pool_config.clone(),
            pool: ctx.accounts.pool.clone(),
            pool_token_account: ctx.accounts.pool_token_account.clone(),
            user_token_account: ctx.accounts.user_token_account.clone(),
            owner: ctx.accounts.sender.to_account_info(),
            authority: ctx.accounts.bridge.to_account_info(),
            mint: ctx.accounts.mint.clone(),
            associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info()
        };
        let cpi_ctx_lock = CpiContext::new_with_signer(
            ctx.accounts.pool_program.to_account_info(), 
            cpi_accounts_lock,
            signer
        );
        token_pool::cpi::lock(cpi_ctx_lock, amount)?;

        let receipt = &mut ctx.accounts.receipt;
        receipt.target_chain_id = target_chain_id;
        receipt.mint = ctx.accounts.mint.key();
        receipt.count = receipt.count + 1;

        msg!("create receipt count: {}", receipt.count);

        let symbol_str = String::from_utf8_lossy(&symbol).trim_end_matches('\0').to_string();
        let message = ReceiptMessage {
            source_chain_id: SOLANA_CHAIN_ID,
            target_chain_id: target_chain_id,
            symbol: symbol_str,
            amount: amount,
            target_address: target_address.clone().into_bytes(),
            receipt_index: receipt.count,
        };

        let receipt_message = message.encode();

        let receipt_id_token: [u8; 32] = receipt_message[32..64].try_into().unwrap();
        let receipt_id_hex: String = receipt_id_token.iter().map(|byte| format!("{:02x}", byte)).collect();
        emit!(ReceiptCreated {
            receipt_id: format!("{}.{}", receipt_id_hex, receipt.count),
            amount: amount,
            owner: ctx.accounts.sender.key(),
            symbol: message.symbol,
            target_address: target_address.clone(),
            target_chain_id: target_chain_id,
            mint: ctx.accounts.mint.key(),
        });

        emit!(RequestSend {
            target_chain_id: target_chain_id,
            receiver: target_address,
            message: receipt_message,
            token_amount: TokenAmount {
                target_chain_id: target_chain_id,
                target_contract_address: ctx.accounts.bridge_config.ramp_contract,
                token_address: ctx.accounts.mint.key(),
                symbol: String::from_utf8_lossy(&symbol).trim_end_matches('\0').to_string(),
                amount: amount,
            }
        });
        
        Ok(())
    }

    pub fn create_native_receipt(ctx: Context<CreateNativeReceipt>, target_chain_id: u16, target_address: String, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            !ctx.accounts.bridge_config.is_contract_pause,
            BridgeError::ContractAlreadyPaused
        );
        require!(
            amount > 0,
            BridgeError::InvalidAmount
        );

        let (_, bump) = Pubkey::find_program_address(
            &[
                b"native_bridge",
                NATIVE_TOKEN.as_bytes(),
            ],
            &ctx.accounts.bridge_config.ramp_contract
        );

        let cpi_accounts = ConsumeLimit {
            pool_config: ctx.accounts.pool_config.clone(),
            daily_limit_config: ctx.accounts.daily_limit_config.clone(),
            rate_limit_config: ctx.accounts.rate_limit_config.clone(),
            sender: ctx.accounts.bridge.to_account_info(),
            mint: ctx.accounts.mint.clone(),
            system_program: ctx.accounts.system_program.to_account_info()
        };
        let seeds = &[
            b"native_bridge", 
            NATIVE_TOKEN.as_bytes(),
            &[bump],
        ];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.pool_program.to_account_info(), 
            cpi_accounts,
            signer,
        );
        token_pool::cpi::consume_limit(cpi_ctx, target_chain_id, 0, amount)?;

        let cpi_accounts_lock_native = LockNativeToken {
            pool_config: ctx.accounts.pool_config.clone(),
            native_pool: ctx.accounts.native_pool.clone(),
            sender: ctx.accounts.sender.to_account_info(),
            authority: ctx.accounts.bridge.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info()
        };
        let cpi_ctx_lock_native = CpiContext::new_with_signer(
            ctx.accounts.pool_program.to_account_info(), 
            cpi_accounts_lock_native,
            signer,
        );
        token_pool::cpi::lock_native_token(cpi_ctx_lock_native, amount)?;

        let receipt = &mut ctx.accounts.receipt;
        receipt.target_chain_id = target_chain_id;
        receipt.mint = Pubkey::default();
        receipt.count = receipt.count + 1;

        msg!("create native receipt count: {}", receipt.count);

        let message = ReceiptMessage {
            source_chain_id: SOLANA_CHAIN_ID,
            target_chain_id: target_chain_id,
            symbol: NATIVE_TOKEN.to_string(),
            amount: amount,
            target_address: target_address.clone().into_bytes(),
            receipt_index: receipt.count,
        };

        let receipt_message = message.encode();

        let receipt_id_token: [u8; 32] = receipt_message[32..64].try_into().unwrap();
        let receipt_id_hex: String = receipt_id_token.iter().map(|byte| format!("{:02x}", byte)).collect();
        emit!(ReceiptCreated {
            receipt_id: format!("{}.{}", receipt_id_hex, receipt.count),
            amount: amount,
            owner: ctx.accounts.sender.key(),
            symbol: NATIVE_TOKEN.to_string(),
            target_address: target_address.clone(),
            target_chain_id: target_chain_id,
            mint: ctx.accounts.mint.key(),
        });

        emit!(RequestSend {
            target_chain_id: target_chain_id,
            receiver: target_address,
            message: receipt_message,
            token_amount: TokenAmount {
                target_chain_id: target_chain_id,
                target_contract_address: ctx.accounts.bridge_config.ramp_contract,
                token_address: ctx.accounts.mint.key(),
                symbol: NATIVE_TOKEN.to_string(),
                amount: amount,
            }
        });

        Ok(())
    }

    pub fn forward_message(ctx: Context<ForwardMessage>, target_chain_id: u16, receipt_hash: [u8; MAX_RECEIPT_HASH_LEN], source_chain_id: u16, receiver: Vec<u8>, message: Vec<u8>, token_mount: TokenAmount) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            !ctx.accounts.bridge_config.is_contract_pause,
            BridgeError::ContractAlreadyPaused
        );
        require!(
            target_chain_id > 0 && target_chain_id == SOLANA_CHAIN_ID && target_chain_id == token_mount.target_chain_id,
            BridgeError::InvalidTargetChainId
        );
        require!(
            !receiver.is_empty(),
            BridgeError::InvalidReceiver
        );
        require!(
            !message.is_empty() && message.len() == 32 * 5,
            BridgeError::InvalidMessage
        );
        
        require!(
            !ctx.accounts.bridge_config.token_whitelist.is_empty() 
            && ctx.accounts.bridge_config.token_whitelist.iter().any(|t| String::from_utf8_lossy(&t.symbol).trim_end_matches('\0') == token_mount.symbol && t.chain_id == target_chain_id)
            && ctx.accounts.bridge_config.token_whitelist.iter().any(|t| String::from_utf8_lossy(&t.symbol).trim_end_matches('\0') == token_mount.symbol && t.chain_id == source_chain_id),
            BridgeError::NotFoundInWhitelist
        );
        require!(
            !ctx.accounts.bridge_config.cross_chainlist.is_empty()
            && ctx.accounts.bridge_config.cross_chainlist.iter().any(|t| t.chain_id == token_mount.target_chain_id && t.contract_address == token_mount.target_contract_address.to_string()),
            BridgeError::InvalidTargetContractAddress
        );
        require!(
            token_mount.token_address == ctx.accounts.mint.key(),
            BridgeError::InvalidTokenAddress
        );
        require!(
            token_mount.amount > 0,
            BridgeError::InvalidAmount
        );
        require!(
            ctx.accounts.bridge_config.ramp_contract != Pubkey::default(), 
            BridgeError::InvalidRampContractAddress
        );
        require!(
            //ctx.accounts.sender.key() == ctx.accounts.bridge_config.admin,
            ctx.accounts.bridge_config.is_valid_ramp_signer(&ctx.accounts.ramp.key, &ctx.accounts.mint.key()),
            BridgeError::Unauthorized
        );
        // require!(
        //     ctx.accounts.ramp.is_signer,
        //     BridgeError::MissingSignature
        // );
        
        let is_valid = ReceiptMessageValidator::validate(
            &message,
            &ReceiptMessage {
                source_chain_id: target_chain_id,
                target_chain_id: source_chain_id,
                symbol: token_mount.symbol.clone(),
                amount: token_mount.amount,
                target_address: receiver,
                receipt_index: DEFAULT_RECEIPT_INDEX,
            }
        );

        msg!("forward message valid: {}", is_valid);
        require!(
            is_valid,
            BridgeError::InvalidMessage
        );
        let receipt_hash_expected: [u8; 32] = message[128..160].try_into().unwrap();
        require!(
            &receipt_hash_expected == &receipt_hash,
            BridgeError::InvalidReceiptHash
        );

        let receipt_record = &mut ctx.accounts.receipt_record;
        require!(
            !receipt_record.is_initialized,
            BridgeError::ReceiptHashAlreadyRecorded
        );

        let cpi_accounts = ConsumeLimit {
            pool_config: ctx.accounts.pool_config.clone(),
            daily_limit_config: ctx.accounts.daily_limit_config.clone(),
            rate_limit_config: ctx.accounts.rate_limit_config.clone(),
            sender: ctx.accounts.bridge.to_account_info(),
            mint: ctx.accounts.mint.clone(),
            system_program: ctx.accounts.system_program.to_account_info()
        };
        let seeds = &[
            b"bridge", 
            ctx.accounts.bridge.mint.as_ref(),
            &[ctx.accounts.bridge.bump],
        ];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.pool_program.to_account_info(), 
            cpi_accounts,
            signer
        );
        token_pool::cpi::consume_limit(cpi_ctx, target_chain_id, 1, token_mount.amount)?;
        
        let cpi_accounts_release = Release {
            pool_config: ctx.accounts.pool_config.clone(),
            pool: ctx.accounts.pool.clone(),
            pool_token_account: ctx.accounts.pool_token_account.clone(),
            user_token_account: ctx.accounts.user_token_account.clone(),
            authority: ctx.accounts.bridge.to_account_info(),
            mint: ctx.accounts.mint.clone(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx_release = CpiContext::new_with_signer(
            ctx.accounts.pool_program.to_account_info(), 
            cpi_accounts_release,
            signer
        );
        token_pool::cpi::release(cpi_ctx_release, token_mount.amount)?;

        receipt_record.is_initialized = true;
        receipt_record.receipt_hash = receipt_hash;

        let receipt_index: u64 = u64::from_be_bytes(message[24..32].try_into().unwrap());
        let receipt_id_token: [u8; 32] = message[32..64].try_into().unwrap();
        let receipt_id_hex: String = receipt_id_token.iter().map(|byte| format!("{:02x}", byte)).collect();
        emit!(MessageForwarded {
            amount: token_mount.amount,
            address: ctx.accounts.user_token_account.key(),
            symbol: token_mount.symbol,
            receipt_id: format!("{}.{}", receipt_id_hex, receipt_index),
            source_chain_id: source_chain_id,
            mint: ctx.accounts.mint.key(),
        });
        
        Ok(())
    }

    pub fn pause(ctx: Context<Pause>)-> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.pause_controller,
            BridgeError::Unauthorized
        );
        require!(
            !ctx.accounts.bridge_config.is_contract_pause,
            BridgeError::ContractAlreadyPaused
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.is_contract_pause = true;

        emit!(Paused {
            sender: ctx.accounts.bridge_config.pause_controller,
        });

        Ok(())
    }

    pub fn restart(ctx: Context<Restart>)-> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.bridge_config.is_contract_pause,
            BridgeError::ContractAlreadyStarted
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.is_contract_pause = false;

        emit!(Unpaused {
            sender: ctx.accounts.bridge_config.admin,
        });

        Ok(())
    }

    pub fn set_bridge(ctx: Context<SetBridge>, mint: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
    
        ctx.accounts.bridge.initialize(
            mint,
            ctx.bumps.bridge
        );
    
        Ok(())
    }

    pub fn set_admin(ctx: Context<SetAdmin>, admin: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            admin != Pubkey::default(),
            BridgeError::InvalidAddress
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.admin = admin;
        
        Ok(())
    }

    pub fn set_pause_controller(ctx: Context<SetPauseController>, pause_controller: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            pause_controller != Pubkey::default(),
            BridgeError::InvalidAddress
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.pause_controller = pause_controller;
        
        Ok(())
    }

    pub fn set_token_whitelist(ctx: Context<SetTokenWhitelist>, tokens: Vec<TokenInfo>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            !tokens.is_empty() && tokens.len() <= MAX_TOKEN_LIST_LEN,
            BridgeError::InvalidTokens
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        for token in tokens {
            let symbol_len: usize = token.symbol.iter().position(|&x| x == 0).unwrap_or(MAX_SYMBOL_LEN);
            if symbol_len > MAX_SYMBOL_LEN {
                continue;
            }
            if bridge_config.token_whitelist.is_empty() {
                bridge_config.token_whitelist.push(token);
                continue;
            }
            let exists = bridge_config.token_whitelist.iter().any(|t| t.symbol == token.symbol && t.chain_id == token.chain_id);
            if !exists {
                bridge_config.token_whitelist.push(token);
            }
        }

        Ok(())
    }

    pub fn remove_token(ctx: Context<RemoveToken>, token: TokenInfo) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        let symbol_len: usize = token.symbol.iter().position(|&x| x == 0).unwrap_or(MAX_SYMBOL_LEN);
        require!(
            symbol_len <= MAX_SYMBOL_LEN,
            BridgeError::InvalidSymbol
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        if let Some(pos) = bridge_config.token_whitelist.iter().position(|t| t.symbol == token.symbol && t.chain_id == token.chain_id) {
            bridge_config.token_whitelist.remove(pos);
        }
        
        Ok(())
    }

    pub fn set_cross_chain_config(ctx: Context<SetCrossChainConfig>, chain_id: u16, contract_address: String) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            chain_id > 0,
            BridgeError::InvalidChainId
        );
        require!(
            !contract_address.is_empty(),
            BridgeError::InvalidContractAddress
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        if bridge_config.cross_chainlist.is_empty() {
            bridge_config.cross_chainlist.push(CrossChainInfo {
                chain_id,
                contract_address,
            });
        } else {
            let pos = bridge_config.cross_chainlist.iter().position(|t: &CrossChainInfo| t.chain_id == chain_id || t.contract_address == contract_address);
            if let Some(index) = pos {
                bridge_config.cross_chainlist[index].chain_id = chain_id;
                bridge_config.cross_chainlist[index].contract_address = contract_address;
            } else {
                require!(
                    bridge_config.cross_chainlist.len() < MAX_CROSS_CHAIN_LIST_LEN,
                    BridgeError::Overflow
                );
                bridge_config.cross_chainlist.push(CrossChainInfo {
                    chain_id,
                    contract_address,
                });
            }
        }
        
        Ok(())
    }

    pub fn set_token_pool_contract(ctx: Context<SetTokenPoolContract>, token_pool_contract_address: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            token_pool_contract_address != Pubkey::default(),
            BridgeError::InvalidAddress
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.token_pool_contract = token_pool_contract_address;
        
        Ok(())
    }

    pub fn set_ramp_contract(ctx: Context<SetTokenPoolContract>, ramp_contract_address: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ramp_contract_address != Pubkey::default(),
            BridgeError::InvalidAddress
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.ramp_contract = ramp_contract_address;
        
        Ok(())
    }

    pub fn set_multisig_contract(ctx: Context<SetMultisigContract>, multisig_name: String, multisig_contract_address: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            !multisig_name.trim().is_empty() && multisig_name.len() <= MAX_MULTISIG_NAME_LEN,
            BridgeError::InvalidMultisigName
        );
        require!(
            multisig_contract_address != Pubkey::default(),
            BridgeError::InvalidAddress
        );

        let bridge_config = &mut ctx.accounts.bridge_config;
        bridge_config.multisig_name = multisig_name;
        bridge_config.multisig_contract = multisig_contract_address;
        
        Ok(())
    }

    pub fn init_code_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            BridgeError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_code == 0,
            BridgeError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_code = Clock::get()?.unix_timestamp as u64 + 60;

        Ok(())
    }

    pub fn init_owner_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            BridgeError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_owner == 0,
            BridgeError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_owner = Clock::get()?.unix_timestamp as u64 + 60;
        
        Ok(())
    }

    pub fn init_admin_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            BridgeError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_admin == 0,
            BridgeError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_admin = Clock::get()?.unix_timestamp as u64 + 60;

        Ok(())
    }

    pub fn cancel_code_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            BridgeError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_code > 0,
            BridgeError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_code = 0;

        Ok(())
    }

    pub fn cancel_owner_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            BridgeError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_owner > 0,
            BridgeError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_owner = 0;

        Ok(())
    }

    pub fn cancel_admin_upgrade(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            BridgeError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_admin > 0,
            BridgeError::InvalidCall
        );

        let upgrade = &mut ctx.accounts.upgrade;
        upgrade.end_admin = 0;

        Ok(())
    }

    pub fn finalize_upgrades(ctx: Context<DoUpgrade>) -> Result<()> {
        require!(
            ctx.accounts.bridge_config.is_initialized,
            BridgeError::ContractNotInitialized
        );
        require!(
            ctx.accounts.admin.key() == ctx.accounts.bridge_config.admin,
            BridgeError::Unauthorized
        );
        require!(
            ctx.accounts.upgrade.to_account_info().owner == ctx.program_id,
            BridgeError::InvalidUpgrade
        );
        require!(
            ctx.accounts.upgrade.end_code + ctx.accounts.upgrade.end_owner + ctx.accounts.upgrade.end_admin > 0,
            BridgeError::InvalidCall
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
        space = 8 + BridgeConfig::LEN,
        seeds = [b"bridge_config"],
        bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,
    #[account(mut, signer)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(target_chain_id: u16)]
pub struct CreateReceipt<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [b"bridge", bridge.mint.as_ref()],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, Bridge>,

    #[account(
        init_if_needed,
        payer = sender,
        space = 8 + CrossChainReceipt::LEN,
        seeds = [b"receipt_info", bridge.mint.key().as_ref(), &target_chain_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub receipt: Account<'info, CrossChainReceipt>,
    #[account(mut, signer)]
    pub sender: Signer<'info>,
    /// CHECK: This is the mint.
    pub mint: AccountInfo<'info>,
    /// CHECK: This is the pool PDA.
    #[account(mut)]
    pub pool: AccountInfo<'info>,
    /// CHECK: This is the pool_config PDA.
    #[account(mut)]
    pub pool_config: AccountInfo<'info>,
    /// CHECK: This is the daily_limit_config PDA.
    #[account(mut)]
    pub daily_limit_config: AccountInfo<'info>,
    /// CHECK: This is the rate_limit_config PDA.
    #[account(mut)]
    pub rate_limit_config: AccountInfo<'info>,
    /// CHECK: This is the user_token_account.
    #[account(mut)]
    pub user_token_account: AccountInfo<'info>,
    /// CHECK: This is the pool_token_account.
    #[account(mut)]
    pub pool_token_account: AccountInfo<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub pool_program: Program<'info, TokenPool>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(target_chain_id: u16)]
pub struct CreateNativeReceipt<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(
        init_if_needed,
        payer = sender,
        space = 8 + Bridge::LEN,
        seeds = [b"native_bridge", NATIVE_TOKEN.as_bytes()],
        bump,
    )]
    pub bridge: Account<'info, Bridge>,

    #[account(
        init_if_needed,
        payer = sender,
        space = 8 + CrossChainReceipt::LEN,
        seeds = [b"native_receipt_info", NATIVE_TOKEN.as_bytes(), &target_chain_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub receipt: Account<'info, CrossChainReceipt>,
    #[account(mut, signer)]
    pub sender: Signer<'info>,
    /// CHECK: This is the mint.
    #[account(address = anchor_spl::token::spl_token::native_mint::ID)]
    pub mint: AccountInfo<'info>,
    /// CHECK: This is the native_pool PDA.
    #[account(mut)]
    pub native_pool: AccountInfo<'info>,
    /// CHECK: This is the pool_config PDA.
    #[account(mut)]
    pub pool_config: AccountInfo<'info>,
    /// CHECK: This is the daily_limit_config PDA.
    #[account(mut)]
    pub daily_limit_config: AccountInfo<'info>,
    /// CHECK: This is the rate_limit_config PDA.
    #[account(mut)]
    pub rate_limit_config: AccountInfo<'info>,
    /// CHECK: This is the pool_token_account.
    pub pool_program: Program<'info, TokenPool>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(target_chain_id: u16, receipt_hash: [u8; 32])]
pub struct ForwardMessage<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [b"bridge", bridge.mint.as_ref()],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, Bridge>,

    #[account(
        init_if_needed,
        payer = sender,
        space = 8 + ReceiptRecord::LEN,
        seeds = [b"receipt_record", receipt_hash.as_ref()],
        bump,
    )]
    receipt_record: Account<'info, ReceiptRecord>,

    #[account(mut, signer)]
    pub sender: Signer<'info>,
    /// CHECK: This is the ramp.
    pub ramp: AccountInfo<'info>,
    /// CHECK: This is the mint.
    pub mint: AccountInfo<'info>,
    /// CHECK: This is the pool PDA.
    #[account(mut)]
    pub pool: AccountInfo<'info>,
    /// CHECK: This is the pool_config PDA.
    #[account(mut)]
    pub pool_config: AccountInfo<'info>,
    /// CHECK: This is the daily_limit_config PDA.
    #[account(mut)]
    pub daily_limit_config: AccountInfo<'info>,
    /// CHECK: This is the rate_limit_config PDA.
    #[account(mut)]
    pub rate_limit_config: AccountInfo<'info>,
    /// CHECK: This is the user_token_account.
    #[account(mut)]
    pub user_token_account: AccountInfo<'info>,
    /// CHECK: This is the pool_token_account.
    #[account(mut)]
    pub pool_token_account: AccountInfo<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub pool_program: Program<'info, TokenPool>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.pause_controller)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Restart<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetBridge<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + Bridge::LEN,
        seeds = [b"bridge", mint.key().as_ref()],
        bump,
    )]
    pub bridge: Account<'info, Bridge>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub mint: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetPauseController<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetTokenWhitelist<'info> {
    #[account(
        mut,
        has_one = admin @ BridgeError::Unauthorized,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveToken<'info> {
    #[account(
        mut,
        has_one = admin @ BridgeError::Unauthorized,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetCrossChainConfig<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetTokenPoolContract<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetRampContract<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetMultisigContract<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut, signer, address = bridge_config.admin)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DoUpgrade<'info> {
    #[account(
        mut,
        seeds = [b"bridge_config"],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

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
pub struct BridgeConfig {
    pub is_initialized: bool,
    pub admin: Pubkey,
    pub is_contract_pause: bool,
    pub pause_controller: Pubkey,
    pub token_pool_contract: Pubkey,
    pub ramp_contract: Pubkey,
    pub multisig_name: String,
    pub multisig_contract: Pubkey,
    pub token_whitelist: Vec<TokenInfo>,
    pub cross_chainlist: Vec<CrossChainInfo>,
    pub bump: u8,
}

impl BridgeConfig {
    pub const LEN: usize = 1 + 32 + 1 + 32 * 3 + 36 + 32 + 4 + 18 * MAX_TOKEN_LIST_LEN + 4 + 58 * MAX_CROSS_CHAIN_LIST_LEN + 1;
    fn is_valid_ramp_signer(&self, signer: &Pubkey, mint: &Pubkey) -> bool {
        // let (expected_pda, _) = Pubkey::find_program_address(
        //     &[b"pool", mint.as_ref()],
        //     &self.token_pool_contract
        // );
        let (expected_pda, _) = Pubkey::find_program_address(
            &[b"ramp", mint.as_ref()],
            &self.ramp_contract
        );
        msg!("valid ramp signer {}", signer);
        msg!("valid ramp contract {}", self.ramp_contract);
        msg!("valid ramp expected {}", expected_pda);
        signer == &expected_pda
    }
}

#[account]
pub struct Bridge {
    pub is_initialized: bool,
    pub mint: Pubkey,        
    pub bump: u8,
}

impl Bridge {
    pub const LEN: usize = 1 + 32 + 1;
    pub fn initialize(&mut self, mint: Pubkey, bump: u8) {
        self.is_initialized = true;
        self.mint = mint;
        self.bump = bump;
    }
}

#[account]
struct ReceiptRecord {
    is_initialized: bool,
    receipt_hash: [u8; 32],
}

impl ReceiptRecord {
    const LEN: usize = 1 + 32;
}

#[account]
pub struct CrossChainReceipt {
    pub target_chain_id: u16,
    pub mint: Pubkey,
    pub count: u64,
}

impl CrossChainReceipt {
    pub const LEN: usize = 2 + 32 + 8;
}

#[account]
pub struct ReceiptMessage {
    pub source_chain_id: u16,
    pub target_chain_id: u16,
    pub symbol: String,
    pub amount: u64,
    pub target_address: Vec<u8>,
    pub receipt_index: u64,
}

impl ReceiptMessage {
    fn encode(&self) -> Vec<u8> {
        let receipt_index_bytes = self.receipt_index.to_be_bytes();
        let mut receipt_index_padded = [0u8; 32];
        receipt_index_padded[24..32].copy_from_slice(&receipt_index_bytes);

        let receipt_id_token = self.generate_receipt_id_token();
        let receipt_hash = self.generate_receipt_hash(receipt_id_token);
        let hashes = [
            receipt_index_padded,
            receipt_id_token,
            self.hash_field(&self.amount.to_be_bytes()),
            self.hash_field(&self.target_address),
            receipt_hash,
        ];

        hashes.concat()
    }

    fn generate_receipt_id_token(&self) -> [u8; 32] {
        let chain_hash = self.hash_field(&self.source_chain_id.to_be_bytes());
        let target_chain_hash = self.hash_field(&self.target_chain_id.to_be_bytes());
        let symbol_hash = self.hash_field(self.symbol.as_bytes());
        
        self.hash_field(
            &[chain_hash, target_chain_hash, symbol_hash].concat()
        )
    }

    fn generate_receipt_hash(&self, receipt_id_token: [u8; 32]) -> [u8; 32] {
        let receipt_id_hash = self.hash_field(&[
            receipt_id_token,
            self.hash_field(&self.receipt_index.to_be_bytes())].concat()
        );
        let hashes = [
            receipt_id_hash,
            self.hash_field(&self.amount.to_be_bytes()),
            self.hash_field(&self.target_address),
        ];
        
        self.hash_field(&hashes.concat())
    }

    fn generate_receipt_hash_extra(&self, receipt_id_token: [u8; 32], receipt_index: u64) -> [u8; 32] {
        let receipt_id_hash = self.hash_field(&[
            receipt_id_token,
            self.hash_field(&receipt_index.to_be_bytes())].concat()
        );
        let hashes = [
            receipt_id_hash,
            self.hash_field(&self.amount.to_be_bytes()),
            self.hash_field(&self.target_address),
        ];
        
        self.hash_field(&hashes.concat())
    }

    // fn hash_field(&self, data: &[u8]) -> [u8; 32] {
    //     let mut hasher = Keccak256::new();
    //     hasher.update(data);
    //     hasher.finalize().into()
    // }
    fn hash_field(&self, data: &[u8]) -> [u8; 32] {
        keccak::hash(data).to_bytes()
    }
}

pub struct ReceiptMessageValidator;

impl ReceiptMessageValidator {
    fn validate(
        message: &Vec<u8>,
        receipt_message: &ReceiptMessage,
    ) -> bool {
        let receipt_index_expected: u64 = u64::from_be_bytes(message[24..32].try_into().unwrap());
        let receipt_id_token_expected: [u8; 32] = message[32..64].try_into().unwrap();
        let amount_expected: [u8; 32] = message[64..96].try_into().unwrap();
        let target_address_expected: [u8; 32] = message[96..128].try_into().unwrap();
        let receipt_expected: [u8; 32] = message[128..160].try_into().unwrap();
        
        let receipt_id_token = receipt_message.generate_receipt_id_token();
        let receipt_hash = receipt_message.generate_receipt_hash_extra(receipt_id_token, receipt_index_expected);

        let valid_token = &receipt_id_token == &receipt_id_token_expected;
        let valid_amount = Self::hash_matches(&receipt_message.amount.to_be_bytes(), &amount_expected);
        let valid_address = Self::hash_matches(&receipt_message.target_address, &target_address_expected);
        let valid_receipt_hash = &receipt_hash == &receipt_expected;

        valid_token && valid_amount && valid_address && valid_receipt_hash
    }
    fn hash_matches(data: &[u8], expected_hash: &[u8; 32]) -> bool {
        // let mut hasher = Keccak256::new();
        // hasher.update(data);
        // hasher.finalize().as_slice() == expected_hash

        keccak::hash(data).to_bytes() == *expected_hash
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct TokenInfo {
    pub symbol: [u8; MAX_SYMBOL_LEN],
    pub chain_id: u16, 
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CrossChainInfo {
    pub chain_id: u16, 
    pub contract_address: String,
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TokenAmount {
    pub target_chain_id: u16,
    pub target_contract_address: Pubkey,
    pub token_address: Pubkey,
    pub symbol: String,
    pub amount: u64,
}

#[event]
pub struct ReceiptCreated {
    pub receipt_id: String,
    pub amount: u64,
    pub owner: Pubkey,
    pub symbol: String,
    pub target_address: String,
    pub target_chain_id: u16,
    pub mint: Pubkey,
}

#[event]
pub struct RequestSend {
    pub target_chain_id: u16,
    pub receiver: String,
    pub message: Vec<u8>,
    pub token_amount: TokenAmount,
}

#[event]
pub struct MessageForwarded {
    pub amount: u64,
    pub address: Pubkey,
    pub symbol: String,
    pub receipt_id: String,
    pub source_chain_id: u16,
    pub mint: Pubkey,
}

#[event]
pub struct Paused {
    pub sender: Pubkey,
}

#[event]
pub struct Unpaused {
    pub sender: Pubkey,
}

#[error_code]
pub enum BridgeError {
    #[msg("Already initialized")]
    AlreadyInitialized,

    #[msg("Contract not initialized")]
    ContractNotInitialized,

    #[msg("Pool not initialized")]
    PoolNotInitialized,

    #[msg("Unauthorized access")]
    Unauthorized,

    #[msg("Contract has already been paused")]
    ContractAlreadyPaused,

    #[msg("Contract has already been started")]
    ContractAlreadyStarted,

    #[msg("Not found token in the whitelist")]
    NotFoundInWhitelist,

    #[msg("Invalid token address")]
    InvalidTokenAddress,

    #[msg("Invalid multisig name")]
    InvalidMultisigName,

    #[msg("Missing signature")]
    MissingSignature,

    #[msg("Invalid address")]
    InvalidAddress,

    #[msg("Tokens must be between 1 and tokens length")]
    InvalidTokens,

    #[msg("Invalid symbol")]
    InvalidSymbol,

    #[msg("Invalid amount")]
    InvalidAmount,

    #[msg("Invalid target chain id")]
    InvalidTargetChainId,

    #[msg("Invalid receiver")]
    InvalidReceiver,

    #[msg("Invalid message")]
    InvalidMessage,

    #[msg("Invalid Receipt Hash")]
    InvalidReceiptHash,

    #[msg("Invalid chain id")]
    InvalidChainId,
    
    #[msg("Invalid contract address")]
    InvalidContractAddress,

    #[msg("Invalid target contract address")]
    InvalidTargetContractAddress,

    #[msg("Invalid ramp contract address")]
    InvalidRampContractAddress,

    #[msg("Receipt hash has already been recorded")]
    ReceiptHashAlreadyRecorded,

    #[msg("Overflow occurred")]
    Overflow,

    #[msg("Invalid upgrade")]
    InvalidUpgrade,

    #[msg("Invalid call")]
    InvalidCall,
}
