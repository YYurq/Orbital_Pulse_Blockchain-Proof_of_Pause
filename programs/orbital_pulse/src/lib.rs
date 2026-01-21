use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
use anchor_lang::solana_program::hash::hashv;
use anchor_lang::solana_program::sysvar::slot_hashes;

// Ваш официальный Program ID в сети Solana
declare_id!("3o6We5WQoGDM6wpQMPq5VE3fjvC7zgCUD56X12vLn917");

#[program]
pub mod orbital_engine {
    use super::*;

    /// Инициализация ядра и выдача Генезис-награды (3 $ORBIT)
    pub fn initialize(ctx: Context<Initialize>, epsilon: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.authority = ctx.accounts.signer.key();
        state.epsilon = epsilon;
        state.last_noise = 0;
        state.pause_count = 0;
        state.current_orbit = 0;

        let seeds = &[b"orbital-genesis".as_ref(), &[ctx.bumps.mint]];
        let signer = &[&seeds[..]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.mint.to_account_info(),
                },
                signer,
            ),
            3 * 1_000_000_000, // Генезис: 3 целых токена (9 знаков после запятой)
        )?; 

        msg!("Ядро активно. Генезис-награда 3 $ORBIT выдана.");
        Ok(())
    }

    /// Попытка орбитального перехода согласно Закону Допуска
    pub fn try_transition(ctx: Context<Transition>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        
        // Проверка прав доступа (ТФАПСИ: контроль источника)
        require_keys_eq!(ctx.accounts.signer.key(), state.authority, ErrorCode::UnauthorizedAccess);

        // Получение энтропии (шума) из блокчейна
        let slot_hashes_info = &ctx.accounts.slot_hashes;
        let data = slot_hashes_info.try_borrow_data()?;
        let recent_hash_bytes: [u8; 32] = data[12..44].try_into().map_err(|_| ErrorCode::HashNotFound)?;

        // Вычисление фазового сдвига (Noise)
        let noise_hash = hashv(&[&recent_hash_bytes, &ctx.program_id.to_bytes(), &state.authority.to_bytes()]);
        let noise = u64::from_le_bytes(noise_hash.as_ref()[0..8].try_into().unwrap());
        
        // Расчет дельты (Закон Допуска)
        let delta = if noise > state.last_noise { noise - state.last_noise } else { state.last_noise - noise };

        // Если дельта меньше порога epsilon — наступает состояние ПАУЗЫ
        if delta < state.epsilon {
            state.pause_count += 1;
            state.current_orbit = (state.current_orbit + 1) % 5;
            
            let seeds = &[b"orbital-genesis".as_ref(), &[ctx.bumps.mint]];
            let signer = &[&seeds[..]];

            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    MintTo {
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.token_account.to_account_info(),
                        authority: ctx.accounts.mint.to_account_info(),
                    },
                    signer,
                ),
                100_000_000 // Награда за переход: 0.1 $ORBIT
            )?; 
            msg!("ПАУЗА! Состояние стабилизировано. Добыто 0.1 $ORBIT.");
        }
        
        state.last_noise = noise;
        Ok(())
    }
}

#[account]
pub struct PulseState {
    pub authority: Pubkey,   
    pub last_noise: u64,
    pub epsilon: u64,
    pub pause_count: u64,
    pub current_orbit: u8,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = signer, space = 8 + 32 + 8 + 8 + 8 + 1)]
    pub state: Account<'info, PulseState>,
    
    #[account(
        init, 
        payer = signer, 
        mint::decimals = 9, 
        mint::authority = mint, 
        seeds = [b"orbital-genesis"], 
        bump
    )]
    pub mint: Account<'info, Mint>,
    
    #[account(
        init, 
        payer = signer, 
        associated_token::mint = mint, 
        associated_token::authority = signer
    )]
    pub token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub signer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Transition<'info> {
    #[account(mut)]
    pub state: Account<'info, PulseState>,
    
    #[account(mut, seeds = [b"orbital-genesis"], bump)]
    pub mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    
    #[account(address = slot_hashes::ID)]
    pub slot_hashes: AccountInfo<'info>,
    
    #[account(mut)]
    pub signer: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Хеш не найден")] 
    HashNotFound,
    #[msg("Доступ запрещен")] 
    UnauthorizedAccess,
  }
