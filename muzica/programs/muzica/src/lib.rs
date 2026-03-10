// Signature: 2eiavRruVkTDrDn3q5HuQ2wSancmh4QpX2mqVFBdVWtQ6VkzKNT6WN6eqmR3ffPHiJipMisiXRxb92YAL1HG6hhf

#![allow(unexpected_cfgs)]
#![allow(unused_imports)]
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount, Transfer, SetAuthority, set_authority, mint_to, spl_token::instruction::AuthorityType};
use anchor_spl::associated_token;
use anchor_spl::associated_token::AssociatedToken;

declare_id!("24iCyiUg1Vd5eGPEua31dVn7rYAuLiYBDkgpHbLhC9ob");

pub const MAX_TITLE_LEN: usize = 64;
pub const MAX_CID_LEN: usize = 128;
pub const MAX_CONTRIBUTORS: usize = 16; 
pub const MIN_PRICE_LAMPORTS: u64 = 1_000_000;



#[program]
pub mod muzica {

   

    use anchor_spl::token::{self, Burn, MintTo};

    use super::*;

  
    pub fn initialize_track(
        ctx: Context<InitializeTrack>,
        track_id: u64,
        title: String,
        cid: String,
        master_hash: [u8; 32],
        contributors: Vec<Pubkey>,
        shares_bps: Vec<u16>,
    ) -> Result<()> {
        let track = &mut ctx.accounts.track;

        require!(title.len() <= MAX_TITLE_LEN, ErrorCode::TitleTooLong);
        require!(cid.len() <= MAX_CID_LEN, ErrorCode::CidTooLong);
        require!(contributors.len() == shares_bps.len(), ErrorCode::InvalidArgs);
        require!(contributors.len() > 0, ErrorCode::NoContributors);
        require!(contributors.len() <= MAX_CONTRIBUTORS, ErrorCode::TooManyContributors);

        let sum: u64 = shares_bps.iter().map(|s| *s as u64).sum();
        require!(sum == 10000, ErrorCode::InvalidShareTotal);

        track.authority = *ctx.accounts.authority.key;
        track.track_id = track_id;
        track.title = title;
        track.cid = cid;
        track.master_hash = master_hash;
        track.contributors = contributors.clone();
        track.shares = shares_bps.clone();
        track.stem_mints = Vec::new();
        track.royalty_version = 0;
        track.bump = ctx.bumps.track;

        emit!(TrackInitialized {
            track_id,
            authority: track.authority,
            contributors,
            shares: shares_bps,
        });

        Ok(())
    }

    pub fn stem_mint(ctx: Context<StemMint>, _track_id: u64, stem_mint: Pubkey) -> Result<()> {

        let track = &mut ctx.accounts.track;
        require!(track.stem_mints.len() < 64, ErrorCode::TooManyStems);
        track.stem_mints.push(stem_mint);

        Ok(())

    }

    pub fn update_shares(ctx: Context<UpdateShares>, track_id: u64, new_shares_bps: Vec<u16>, contributors: Vec<Pubkey>) -> Result<()> {

        let track = &mut ctx.accounts.track;
        require!(track.track_id == track_id, ErrorCode::InvalidArgs);
        require!(new_shares_bps.len() == contributors.len(), ErrorCode::InvalidRecipientCount);

        let sum: u64 = new_shares_bps.iter().map(|s| *s as u64).sum();
        require!(sum == 10000, ErrorCode::InvalidShareTotal);

        let old_version = track.royalty_version;
        track.shares = new_shares_bps.clone();

        track.royalty_version = old_version.checked_add(1).unwrap();

        emit!(SharesUpdated {
            track_id: track.track_id,
            new_shares: new_shares_bps,
            old_version,
            new_version: track.royalty_version,
        });

        Ok(())
    }

    pub fn create_escrow_ata(ctx: Context<CreateEscrowAta>, track_id: u64, authority: Pubkey) -> Result<()> {

        require!(ctx.accounts.track.track_id == track_id, ErrorCode::InvalidArgs);
        require!(ctx.accounts.track.authority == authority, ErrorCode::InvalidArgs);

        let cpi_accounts = associated_token::Create {
            payer: ctx.accounts.payer.to_account_info(),
            associated_token: ctx.accounts.escrow_token_account.to_account_info(),
            authority: ctx.accounts.track.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };        

        let cpi_program = ctx.accounts.associated_token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        associated_token::create(cpi_ctx)?;

        Ok(())
    }

    pub fn escrow_deposit(ctx: Context<EscrowDeposit>, amount: u64, track_id: u64, authority: Pubkey) -> Result<()> {

        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(ctx.accounts.track.track_id == track_id, ErrorCode::InvalidArgs);
        require!(ctx.accounts.track.authority == authority, ErrorCode::InvalidArgs);
        require!(ctx.accounts.escrow_token_account.owner == ctx.accounts.track.key(), ErrorCode::InvalidTokenAccountOwner);

            let cpi_accounts = Transfer {
                from: ctx.accounts.payer_token_account.to_account_info(),
                to: ctx.accounts.escrow_token_account.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
            };

            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            anchor_spl::token::transfer(cpi_ctx, amount)?;

            emit!(EscrowDeposited {
                track_id: ctx.accounts.track.track_id,
                depositor: ctx.accounts.payer.key(),
                amount,
                mint: ctx.accounts.escrow_token_account.mint,
            });

        Ok(())
    }


    pub fn escrow_distribute<'info>(ctx: Context<'_, '_, '_, 'info, EscrowDistribute<'info>>, amount: u64, track_id: u64) -> Result<()> {

        //whenever you are reading from multiple accounts in a loop you have to clone the data you need first to avoid borrow checker issues
        // trust me i tried for 2 days

        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(ctx.accounts.track.track_id == track_id, ErrorCode::InvalidArgs);
        require!(ctx.accounts.escrow_token_account.owner == ctx.accounts.track.key(), ErrorCode::InvalidTokenAccountOwner);

        let track = &ctx.accounts.track;
        let total_bps: u64 = track.shares.iter().map(|s| *s as u64).sum();
        require!(total_bps == 10000, ErrorCode::InvalidShareTotal);

        let contributors = track.contributors.clone();
        let shares = track.shares.clone();
        let track_bump = track.bump;
        let escrow_mint = ctx.accounts.escrow_token_account.mint;

        let escrow_account_info = ctx.accounts.escrow_token_account.to_account_info();
        let track_account_info = ctx.accounts.track.to_account_info();
        let token_program_info = ctx.accounts.token_program.to_account_info();

        for (i, contributor) in contributors.iter().enumerate() {
            let share_bps = shares[i] as u64;
            let share_amount = amount * share_bps / 10000;

            if share_amount == 0 {
                continue;
            }

            let contributor_token_account = anchor_spl::associated_token::get_associated_token_address(
                contributor,
                &escrow_mint,
            );

            
            let to_account = ctx.remaining_accounts
                .iter()
                .find(|acc| acc.key() == contributor_token_account)
                .ok_or(ErrorCode::InvalidArgs)?;

            let authority_key = ctx.accounts.authority.key();
            let seeds = &[
                b"track".as_ref(),
                authority_key.as_ref(),
                &track_id.to_le_bytes(),
                &[track_bump],
            ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: escrow_account_info.clone(),
                to: to_account.clone(),
                authority: track_account_info.clone(),
            };
            let cpi_ctx = CpiContext::new_with_signer(token_program_info.clone(), cpi_accounts, signer);
            anchor_spl::token::transfer(cpi_ctx, share_amount)?;
        }

        Ok(())
    }



    pub fn mint_stem_nft(ctx: Context<StemMintNFT>, track_id: u64, nft_index: u64) -> Result<()> {

        let track = &mut ctx.accounts.track;
        let track_authority = track.authority;
        let track_bump = track.bump;
        let mint_pubkey = ctx.accounts.mint.key();
        let recipient = ctx.accounts.authority.key();

        require!(track.track_id == track_id, ErrorCode::InvalidArgs);
        
        // Find the actual contributor index for the recipient
        let actual_index = track.contributors
            .iter()
            .position(|c| c == &recipient)
            .ok_or(ErrorCode::NotAContributor)? as u64;
        
        require!(nft_index == actual_index, ErrorCode::InvalidArgs);

        let track_id_in_bytes = track_id.to_le_bytes();
        let seeds = &[
            b"track".as_ref(),
            track_authority.as_ref(),
            &track_id_in_bytes,
            &[track_bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts_mint = anchor_spl::token::MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: track.to_account_info(),
        };

        let cpi_program_mint = ctx.accounts.token_program.to_account_info();
        let cpi_ctx_mint = CpiContext::new_with_signer(
            cpi_program_mint,
            cpi_accounts_mint,
            signer,
        );
        mint_to(cpi_ctx_mint, 1)?;

        track.stem_mints.push(mint_pubkey);

        emit!(StemNFTMinted {
            track_id: track.track_id,
            mint: mint_pubkey,
            recipient: ctx.accounts.recipient_token_account.owner,
        });

        Ok(())
    
    }

    pub fn list_track(
    ctx: Context<ListTrack>,
    _track_id: u64,
    _authority: Pubkey,
    price: u64,
) -> Result<()> {
    let listing = &mut ctx.accounts.listing;
    let track = &ctx.accounts.track;

    listing.seller = ctx.accounts.seller.key();
    listing.track = track.key();
    listing.market = ctx.accounts.market.key();
    listing.price = price;

    listing.payment_mint = Pubkey::default(); // SOL
    listing.royalty_version = track.royalty_version;
    listing.is_active = true;
    listing.created_at = Clock::get()?.unix_timestamp;
    listing.bump = ctx.bumps.listing;

    Ok(())
}

pub fn initialize_market(
    ctx: Context<InitializeMarket>,
    fee_bps: u16,
    default_payment_mint: Pubkey,
) -> Result<()> {
    require!(fee_bps <= 1000, ErrorCode::FeeTooHigh); // max 10%

    let market = &mut ctx.accounts.market;
    require!(!market.is_paused, ErrorCode::MarketPaused);

    market.admin = ctx.accounts.admin.key();
    market.treasury = ctx.accounts.treasury.key();
    market.fee_bps = fee_bps;
    market.max_fee_bps = 1000;
    market.default_payment_mint = default_payment_mint;
    market.is_paused = false;
    market.bump = ctx.bumps.market;
    market.treasury_bump = ctx.bumps.treasury;

    emit!(MarketplaceInitialized {
        admin: market.admin,
        treasury: market.treasury,
        fee_bps,
    });

    Ok(())
}

pub fn update_market_config(
    ctx: Context<UpdateMarketConfig>,
    new_fee_bps: Option<u16>,
    pause: Option<bool>,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    require!(!market.is_paused || pause.is_some(), ErrorCode::MarketPaused);

    if let Some(fee) = new_fee_bps {
        require!(fee <= market.max_fee_bps, ErrorCode::FeeTooHigh);
        market.fee_bps = fee;
    }

    if let Some(pause) = pause {
        market.is_paused = pause;
    }

    emit!(MarketConfigUpdated {
        admin: ctx.accounts.admin.key(),
        new_fee_bps,
        paused: market.is_paused,
    });

    Ok(())
}

pub fn withdraw_marketplace_fees(
    ctx: Context<WithdrawMarketplaceFees>,
    amount: u64,
) -> Result<()> {
    let _market = &ctx.accounts.market; // Used for constraints
    let treasury = &ctx.accounts.treasury;

    let treasury_balance = treasury.to_account_info().lamports();
    let rent_exempt = Rent::get()?.minimum_balance(8);
    let available = treasury_balance.checked_sub(rent_exempt).unwrap_or(0);

    require!(amount <= available, ErrorCode::InsufficientFunds);

    // Transfer from treasury PDA to admin
    **treasury.to_account_info().try_borrow_mut_lamports()? -= amount;
    **ctx.accounts.admin.to_account_info().try_borrow_mut_lamports()? += amount;

    emit!(FeesWithdrawn {
        admin: ctx.accounts.admin.key(),
        amount,
        remaining_balance: treasury_balance - amount,
    });

    Ok(())
}

pub fn buy_track(ctx: Context<BuyTrack>) -> Result<()> {
    let listing = &ctx.accounts.listing;
    let market = &ctx.accounts.market;
    let price = listing.price;

    require!(
        ctx.accounts.buyer.key() != ctx.accounts.seller.key(),
        ErrorCode::SelfPurchase
    );

    let fee = (price as u128 * market.fee_bps as u128 / 10000) as u64;
    let seller_amount = price.checked_sub(fee).ok_or(ErrorCode::MathError)?;

    // Pay seller
    let ix_seller = anchor_lang::solana_program::system_instruction::transfer(
        &ctx.accounts.buyer.key(),
        &ctx.accounts.seller.key(),
        seller_amount,
    );

    anchor_lang::solana_program::program::invoke(
        &ix_seller,
        &[
            ctx.accounts.buyer.to_account_info(),
            ctx.accounts.seller.to_account_info(),
        ],
    )?;

    // Pay marketplace fee to treasury
    if fee > 0 {
        let ix_fee = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.buyer.key(),
            &ctx.accounts.treasury.key(),
            fee,
        );

        anchor_lang::solana_program::program::invoke(
            &ix_fee,
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.treasury.to_account_info(),
            ],
        )?;
    }

    let cpi_accounts = anchor_spl::token::Transfer {
        from: ctx.accounts.seller_nft_ata.to_account_info(),
        to: ctx.accounts.buyer_nft_ata.to_account_info(),
        authority: ctx.accounts.seller.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    anchor_spl::token::transfer(cpi_ctx, 1)?;

    ctx.accounts.listing.is_active = false;

    emit!(NFTPurchased {
        buyer: ctx.accounts.buyer.key(),
        seller: ctx.accounts.seller.key(),
        nft_mint: ctx.accounts.nft_mint.key(),
        price,
        fee,
    });

    Ok(())
}

pub fn cancel_listing(ctx: Context<CancelListing>) -> Result<()> {
    let listing = &mut ctx.accounts.listing;

    require!(listing.is_active, ErrorCode::ListingInactive);

    listing.is_active = false;

    emit!(ListingCancelled {
        seller: ctx.accounts.seller.key(),
        track: ctx.accounts.track.key(),
        nft_mint: ctx.accounts.nft_mint.key(),
    });

    Ok(())
}

pub fn update_price(ctx: Context<UpdatePrice>, new_price: u64) -> Result<()> {
    let listing = &mut ctx.accounts.listing;

    require!(new_price > 0, ErrorCode::InvalidPrice);

    require!(new_price >= MIN_PRICE_LAMPORTS, ErrorCode::PriceTooLow);

    listing.price = new_price;

    emit!(PriceUpdated {
    seller: ctx.accounts.seller.key(),
    nft_mint: ctx.accounts.nft_mint.key(),
    old_price: listing.price,
    new_price,
});

    Ok(())
}

pub fn initialize_curve_for_track(
    ctx: Context<InitializeCurveForTrack>,
    base_price: u64,
    k: u64,
    tokens_per_nft: u64,
) -> Result<()> {
    let curve = &mut ctx.accounts.curve;

    require!(base_price > 0, ErrorCode::InvalidBasePrice);
    require!(k > 0, ErrorCode::InvalidCurveParam);
    require!(tokens_per_nft > 0, ErrorCode::InvalidArgs);

    curve.track = ctx.accounts.track.key();
    curve.mint = ctx.accounts.curve_mint.key();
    curve.vault = Pubkey::default(); // Vault is derived PDA, not stored

    curve.supply = 0;
    curve.reserve = 0;

    // Pump.fun params
    curve.base_price = base_price;
    curve.k = k;
    curve.tokens_per_nft = tokens_per_nft;

    curve.bump = ctx.bumps.curve;

    Ok(())
}

pub fn buy_tokens(
    ctx: Context<BuyTokens>,
    amount: u64,
    max_lamports: u64,
) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let curve = &mut ctx.accounts.curve;
    let market = &ctx.accounts.market;

    let cost = cost_to_buy(curve, amount);

    require!(cost <= max_lamports, ErrorCode::SlippageExceeded);

    let fee = (cost as u128 * market.fee_bps as u128 / 10000) as u64;
    let vault_amount = cost.checked_sub(fee).ok_or(ErrorCode::MathError)?;

    let ix_vault = anchor_lang::solana_program::system_instruction::transfer(
        &ctx.accounts.buyer.key(),
        &ctx.accounts.treasury.key(),
        vault_amount,
    );

    anchor_lang::solana_program::program::invoke(
        &ix_vault,
        &[
            ctx.accounts.buyer.to_account_info(),
            ctx.accounts.treasury.to_account_info(),
        ],
    )?;

    if fee > 0 {
        let ix_fee = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.buyer.key(),
            &ctx.accounts.marketplace_treasury.key(),
            fee,
        );

        anchor_lang::solana_program::program::invoke(
            &ix_fee,
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.marketplace_treasury.to_account_info(),
            ],
        )?;
    }

    let track_key = ctx.accounts.track.key();
    let curve_bump = curve.bump;
    let seeds = &[
        b"curve",
        track_key.as_ref(),
        &[curve_bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_accounts = MintTo {
        mint: ctx.accounts.share_mint.to_account_info(),
        to: ctx.accounts.buyer_token_account.to_account_info(),
        authority: curve.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer,
    );

    token::mint_to(cpi_ctx, amount)?;

    curve.supply = curve.supply.checked_add(amount).unwrap();

    emit!(TokensBought {
        buyer: ctx.accounts.buyer.key(),
        track: ctx.accounts.track.key(),
        amount,
        cost,
        fee,
        new_supply: curve.supply,
    });

    Ok(())
}

pub fn sell_tokens(
    ctx: Context<SellTokens>,
    amount: u64,
    min_lamports: u128,
) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidAmount);

    let curve = &mut ctx.accounts.curve;
    let market = &ctx.accounts.market;

    //  Calculate refund
    let refund = refund_for_sell(curve, amount)?;

    //  Slippage protection
    require!(refund >= min_lamports, ErrorCode::SlippageExceeded);

    //  Calculate marketplace fee
    let fee = (refund * market.fee_bps as u128 / 10000) as u64;
    let seller_amount = (refund as u64).checked_sub(fee).ok_or(ErrorCode::MathError)?;

    //  Burn seller tokens
    let burn_accounts = Burn {
        mint: ctx.accounts.share_mint.to_account_info(),
        from: ctx.accounts.seller_token_account.to_account_info(),
        authority: ctx.accounts.seller.to_account_info(),
    };

    let burn_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        burn_accounts,
    );

    token::burn(burn_ctx, amount)?;

    //  Send SOL from treasury → seller (minus fee)
    let track_key = ctx.accounts.track.key();
    let vault_seeds = &[
        b"curve_vault".as_ref(),
        track_key.as_ref(),
        &[ctx.bumps.treasury],
    ];
    let vault_signer = &[&vault_seeds[..]];

    let ix_seller = anchor_lang::solana_program::system_instruction::transfer(
        &ctx.accounts.treasury.key(),
        &ctx.accounts.seller.key(),
        seller_amount,
    );
    anchor_lang::solana_program::program::invoke_signed(
        &ix_seller,
        &[
            ctx.accounts.treasury.to_account_info(),
            ctx.accounts.seller.to_account_info(),
        ],
        vault_signer,
    )?;

    //  Send marketplace fee to marketplace treasury
    if fee > 0 {
        let ix_fee = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.treasury.key(),
            &ctx.accounts.marketplace_treasury.key(),
            fee,
        );
        anchor_lang::solana_program::program::invoke_signed(
            &ix_fee,
            &[
                ctx.accounts.treasury.to_account_info(),
                ctx.accounts.marketplace_treasury.to_account_info(),
            ],
            vault_signer,
        )?;
    }

    //  Decrease supply
    curve.supply = curve.supply.checked_sub(amount).unwrap();

    emit!(TokensSold {
        seller: ctx.accounts.seller.key(),
        track: ctx.accounts.track.key(),
        amount,
        refund: refund as u64,
        fee,
        new_supply: curve.supply,
    });

    Ok(())
}

pub fn buy_nft_with_curve_price(
    ctx: Context<BuyNftWithCurvePrice>,
    nft_index: u64,
) -> Result<()> {
    let curve = &ctx.accounts.curve;

    // 🎯 Get spot curve price
    let spot_price = current_price(curve);

    // 🎧 NFT dynamic price
    let nft_price = spot_price
        .checked_mul(curve.tokens_per_nft as u128)
        .ok_or(ErrorCode::Overflow)? as u64;

    // 💸 Transfer SOL → treasury
    let buyer = &ctx.accounts.buyer;
    let treasury = &ctx.accounts.treasury;

    **buyer.to_account_info().try_borrow_mut_lamports()? -= nft_price;
    **treasury.to_account_info().try_borrow_mut_lamports()? += nft_price;

    // 🎨 Mint NFT to buyer
    let track_key = ctx.accounts.track.key();
    let _seeds = &[
        b"stem_mint",
        track_key.as_ref(),
        &nft_index.to_le_bytes(),
    ];
    
    let cpi_accounts = MintTo {
        mint: ctx.accounts.stem_mint.to_account_info(),
        to: ctx.accounts.buyer_nft_account.to_account_info(),
        authority: ctx.accounts.mint_authority.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );

    token::mint_to(cpi_ctx, 1)?;

    emit!(NFTBoughtWithCurvePrice {
        buyer: buyer.key(),
        track: track_key,
        nft_mint: ctx.accounts.stem_mint.key(),
        price: nft_price,
        spot_price: spot_price as u64,
    });

    Ok(())
}


}

/// Spot price
pub fn current_price(curve: &CurveState) -> u128 {
    let s = curve.supply as u128;
    curve.base_price as u128 + curve.k as u128 * s * s as u128
}

/// Exact lamports required to buy `amount`
pub fn cost_to_buy(curve: &CurveState, amount: u64) -> u64 {
    let s = curve.supply as u128;
    let d = amount as u128;

    let base = curve.base_price as u128;
    let k = curve.k as u128;

    // base * Δ
    let base_cost = base * d;

    // (k/3) * [(s+Δ)^3 - s^3]
    let new = (s + d).pow(3);
    let old = s.pow(3);
    let curve_cost = k * (new - old) / 3;

    (base_cost + curve_cost) as u64
}


/// Lamports received when selling
pub fn refund_for_sell(curve: &CurveState, amount: u64) -> Result<u128> {
    require!(amount <= curve.supply, ErrorCode::InsufficientLiquidity);

    let s = curve.supply as u128;
    let d = amount as u128;

    let base = curve.base_price as u128;
    let k = curve.k as u128;

    let base_refund = base * d;

    let remaining = s - d;

    let new = s.saturating_pow(3);
    let old = remaining.saturating_pow(3);

    let curve_refund = k * (new - old) / 3;

    Ok((base_refund + curve_refund) as u128)
}

pub fn buy_with_slippage_check(
    curve: &CurveState,
    amount: u64,
    max_lamports: u64,
) -> Result<u64> {
    let cost = cost_to_buy(curve, amount);
    require!(cost <= max_lamports, ErrorCode::SlippageExceeded);
    Ok(cost)
}

pub fn sell_with_slippage_check(
    curve: &CurveState,
    amount: u64,
    min_lamports: u128,
) -> Result<u128> {
    let refund = refund_for_sell(curve, amount)?;
    require!(refund >= min_lamports, ErrorCode::SlippageExceeded);
    Ok(refund)
}


    #[event]
    pub struct StemNFTMinted {
        pub track_id: u64,
        pub mint: Pubkey,
        pub recipient: Pubkey,
    }

    #[event]
    pub struct NFTPurchased {
        pub buyer: Pubkey,
        pub seller: Pubkey,
        pub nft_mint: Pubkey,
        pub price: u64,
        pub fee: u64,
    }

    #[event]
    pub struct ListingCancelled {
        pub seller: Pubkey,
        pub track: Pubkey,
        pub nft_mint: Pubkey,
    }

    #[event]
    pub struct TokensBought {
        pub buyer: Pubkey,
        pub track: Pubkey,
        pub amount: u64,
        pub cost: u64,
        pub fee: u64,
        pub new_supply: u64,
    }

    #[event]
    pub struct TokensSold {
        pub seller: Pubkey,
        pub track: Pubkey,
        pub amount: u64,
        pub refund: u64,
        pub fee: u64,
        pub new_supply: u64,
    }

    #[event]
    pub struct NFTBoughtWithCurvePrice {
        pub buyer: Pubkey,
        pub track: Pubkey,
        pub nft_mint: Pubkey,
        pub price: u64,
        pub spot_price: u64,
    }

    #[event]
    pub struct MarketplaceInitialized {
        pub admin: Pubkey,
        pub treasury: Pubkey,
        pub fee_bps: u16,
    }

    #[event]
    pub struct MarketConfigUpdated {
        pub admin: Pubkey,
        pub new_fee_bps: Option<u16>,
        pub paused: bool,
    }

    #[event]
    pub struct FeesWithdrawn {
        pub admin: Pubkey,
        pub amount: u64,
        pub remaining_balance: u64,
    }



    #[derive(Accounts)]
    #[instruction(track_id: u64, nft_index: u64)]
    pub struct StemMintNFT<'info> {

        #[account(mut)]
        pub payer: Signer<'info>,

        // Track account - just verify it's mutable and matches the track_id
        // We don't use PDA seeds here because the track was created by a different authority
        // The client must pass the correct track address
        #[account(
            mut,
            constraint = track.track_id == track_id @ ErrorCode::InvalidArgs,
        )]
        pub track: Account<'info, Track>,

        #[account(
            init,
            payer = payer,
            seeds = [
                b"stem_mint".as_ref(),
                track.key().as_ref(),
                &nft_index.to_le_bytes(),
            ],
            bump,
            mint::decimals = 0,
            mint::authority = track,
        )]
        pub mint: Account<'info, Mint>,

        #[account(
            init_if_needed,
            payer = payer,
            associated_token::mint = mint,
            associated_token::authority = authority,
        )]
        pub recipient_token_account: Account<'info, TokenAccount>,

        pub authority: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
        pub system_program: Program<'info, System>,

    }


    #[derive(Accounts)]
    #[instruction(amount: u64, track_id: u64)]
    pub struct EscrowDistribute<'info> {

        #[account(
            mut,
            seeds = [
                b"track".as_ref(), 
                authority.key().as_ref(), 
                track_id.to_le_bytes().as_ref()
                ],
            bump,
            has_one = authority,
        )]
        pub track: Account<'info, Track>,

        #[account(mut)]
        pub escrow_token_account: Account<'info, TokenAccount>,

        ///CHECK: This is the PDA authority for the track
        #[account(
            seeds = [
                b"track".as_ref(), 
                authority.key().as_ref(), 
                track_id.to_le_bytes().as_ref()
                ],
            bump = track.bump
        )]
        pub track_authority: UncheckedAccount<'info>,

        pub authority: Signer<'info>,

        pub token_program: Program<'info, Token>,
    }



    #[event]
    pub struct EscrowDeposited {
        pub track_id: u64,
        pub depositor: Pubkey,
        pub amount: u64,
        pub mint: Pubkey,
    }

    #[derive(Accounts)]
    #[instruction(amount: u64, track_id: u64, authority: Pubkey)]
    pub struct EscrowDeposit<'info> {
        #[account(mut)]
        pub payer: Signer<'info>,

        #[account(
            mut,
            seeds = [
                b"track".as_ref(), 
                authority.key().as_ref(), 
                track_id.to_le_bytes().as_ref()
                ],
            bump,
        )]
        pub track: Account<'info, Track>,

        #[account(mut)]
        pub escrow_token_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub payer_token_account: Account<'info, TokenAccount>,


        pub token_program: Program<'info, Token>,

    }



    #[derive(Accounts)]
    #[instruction(track_id: u64, authority: Pubkey)]
    pub struct CreateEscrowAta<'info> {
        #[account(mut)]
        pub payer: Signer<'info>,

        #[account(
            mut,
            seeds = [
                b"track".as_ref(), 
                authority.key().as_ref(), 
                track_id.to_le_bytes().as_ref()
                ],
            bump,
        )]
        pub track: Account<'info, Track>,

        ///CHECK: ATA for mint
        #[account(mut)]
        pub escrow_token_account: UncheckedAccount<'info>,

        pub mint: Account<'info, Mint>,

        pub associated_token_program: Program<'info, associated_token::AssociatedToken>,
        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,

    }

    // Removed CreateEscrowAta struct - no longer needed for SOL-based escrow

    #[event]
    pub struct SharesUpdated {
        pub track_id: u64,
        pub new_shares: Vec<u16>,
        pub old_version: u32,
        pub new_version: u32,
    }

    #[derive(Accounts)]
    #[instruction(track_id: u64)]
    pub struct UpdateShares<'info> {
        #[account(mut)]
        pub authority: Signer<'info>,

        #[account(
            mut,
            seeds = [
                b"track".as_ref(), 
                authority.key().as_ref(), 
                track_id.to_le_bytes().as_ref()
                ],
            bump,
        )]
        pub track: Account<'info, Track>,
    }



    #[derive(Accounts)]
    #[instruction(track_id: u64)]
    pub struct StemMint<'info> {
        #[account(mut)]
        pub authority: Signer<'info>,

        #[account(
            mut,
            seeds = [
                b"track".as_ref(), 
                authority.key().as_ref(), 
                track_id.to_le_bytes().as_ref()
                ],
            bump,
        )]
        pub track: Account<'info, Track>,
    }

    #[event]
    pub struct TrackInitialized {
        pub track_id: u64,
        pub authority: Pubkey,
        pub contributors: Vec<Pubkey>,
        pub shares: Vec<u16>,
    }

    #[derive(Accounts)]
    #[instruction(track_id: u64)]
    pub struct InitializeTrack<'info>{

        #[account(mut)]
        pub authority: Signer<'info>,

        #[account(
            init,
            payer = authority,
            space = Track::INIT_SPACE,
            seeds = [b"track".as_ref(), authority.key().as_ref(), &track_id.to_le_bytes().as_ref()],
            bump,
        )]
        pub track: Account<'info, Track>,
        
        pub system_program: Program<'info, System>,
    }

    #[account]
    #[derive(InitSpace)]
    pub struct Track {
        pub authority: Pubkey,
        pub track_id: u64,

        #[max_len(MAX_TITLE_LEN)]
        pub title: String,

        #[max_len(MAX_CID_LEN)]
        pub cid: String,
        pub master_hash: [u8; 32],

        #[max_len(MAX_CONTRIBUTORS * 32)]
        pub contributors: Vec<Pubkey>,

        #[max_len(MAX_CONTRIBUTORS * 2)]
        pub shares: Vec<u16>,

        #[max_len(64 * 32)]
        pub stem_mints: Vec<Pubkey>,
        pub royalty_version: u32,
        pub bump: u8,
    }

    #[error_code]
pub enum ErrorCode {
    #[msg("Invalid arguments provided")]
    InvalidArgs,
    #[msg("Sum of shares must equal 10000 (100%)")]
    InvalidShareTotal,
    #[msg("Too many contributors provided")]
    TooManyContributors,
    #[msg("Math overflow or division error")]
    MathError,
    #[msg("Too many stems")]
    TooManyStems,
    #[msg("Title exceeds maximum length")]
    TitleTooLong,
    #[msg("CID exceeds maximum length")]
    CidTooLong,
    #[msg("At least one contributor is required")]
    NoContributors,
    #[msg("Invalid amount: must be greater than 0")]
    InvalidAmount,
    #[msg("Token account owner must be the track PDA")]
    InvalidTokenAccountOwner,
    #[msg("Recipient count must match contributor count")]
    InvalidRecipientCount,
    #[msg("The signer is not a contributor to this track")]
    NotAContributor,
    #[msg("Marketplace fee cannot exceed 1000 bps (10%)")]
    FeeTooHigh,
    #[msg("Marketplace is currently paused")]
    MarketPaused,
    #[msg("Listing is not active")]
    ListingInactive,
    #[msg("Buyer cannot be the same as seller")]
    SelfPurchase,
    #[msg("Invalid listing state")]
    InvalidListing,
    #[msg("Price must be greater than 0")]
    InvalidPrice,
    #[msg("Price is too low, must be above minimum threshold")]
    PriceTooLow,
    #[msg("Not a fixed price listing")]
    NotFixedPrice,
    #[msg("Invalid base price for bonding curve")]
    InvalidBasePrice,
    #[msg("Invalid curve parameter k")]
    InvalidCurveParam,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Insufficient liquidity in bonding curve")]
    InsufficientLiquidity,
    #[msg("Math overflow or division error")]
    Overflow,
    #[msg("Insufficient funds in treasury")]
    InsufficientFunds,


}


#[derive(Accounts)]
#[instruction(track_id: u64, authority: Pubkey)]
pub struct ListTrack<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(
        mut,
        seeds = [
            b"track",
            authority.key().as_ref(),
            track_id.to_le_bytes().as_ref()
        ],
        bump,
        has_one = authority,
    )]
    pub track: Account<'info, Track>,

    #[account(
        seeds = [b"market"],
        bump,
    )]
    pub market: Account<'info, MarketConfig>,

    #[account(
        init,
        payer = seller,
        space = 8 + Listing::INIT_SPACE,
        seeds = [b"listing", track.key().as_ref(), seller.key().as_ref()],
        bump
    )]
    pub listing: Account<'info, Listing>,

    pub system_program: Program<'info, System>,
}





#[account]
#[derive(InitSpace)]
pub struct Listing {
    pub seller: Pubkey,
    pub track: Pubkey,
    pub market: Pubkey,

    pub price: u64,
    pub payment_mint: Pubkey, // SOL = native mint or SPL

    pub royalty_version: u32, // snapshot from Track
    pub is_active: bool,
    pub pricing_model: u8, // 0 = fixed, 1 = curve

    pub created_at: i64,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct InitializeMarket<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + MarketConfig::INIT_SPACE,
        seeds = [b"market"],
        bump
    )]
    pub market: Account<'info, MarketConfig>,

    /// CHECK: Treasury PDA for holding marketplace fees
    #[account(
        init,
        payer = admin,
        space = 8,
        seeds = [b"treasury"],
        bump
    )]
    pub treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateMarketConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"market"],
        bump = market.bump,
        has_one = admin
    )]
    pub market: Account<'info, MarketConfig>,
}

#[derive(Accounts)]
pub struct WithdrawMarketplaceFees<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"market"],
        bump = market.bump,
        has_one = admin
    )]
    pub market: Account<'info, MarketConfig>,

    /// CHECK: Treasury PDA that holds marketplace fees
    #[account(
        mut,
        seeds = [b"treasury"],
        bump = market.treasury_bump
    )]
    pub treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct MarketConfig {
    pub admin: Pubkey,          // marketplace owner
    pub treasury: Pubkey,       // fee receiver (PDA)

    pub fee_bps: u16,           // marketplace fee (basis points)
    pub max_fee_bps: u16,       // safety cap (optional but smart)

    pub default_payment_mint: Pubkey, // SOL = Pubkey::default()

    pub is_paused: bool,        // emergency stop

    pub bump: u8,
    pub treasury_bump: u8,      // treasury PDA bump
}

#[derive(Accounts)]
pub struct BuyTrack<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: Validated by `listing` constraints (`has_one = seller` and PDA seeds include `seller.key()`), and by `seller_nft_ata.owner == seller.key()` before NFT transfer.
    #[account(mut)]
    pub seller: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"listing", track.key().as_ref(), seller.key().as_ref()],
        bump = listing.bump,
        has_one = seller,
        constraint = listing.is_active @ ErrorCode::ListingInactive,
        close = seller
    )]
    pub listing: Account<'info, Listing>,

    pub track: Account<'info, Track>,

    pub market: Account<'info, MarketConfig>,

    /// CHECK: Marketplace treasury PDA validated by `seeds = [b"treasury"]` and `bump = market.treasury_bump`; only receives lamports via system transfer.
    #[account(mut)]
    pub treasury: UncheckedAccount<'info>,

    pub nft_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = seller_nft_ata.owner == seller.key(),
        constraint = seller_nft_ata.mint == nft_mint.key(),
        constraint = seller_nft_ata.amount >= 1 @ ErrorCode::InvalidAmount
    )]
    pub seller_nft_ata: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = nft_mint,
        associated_token::authority = buyer
    )]
    pub buyer_nft_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelListing<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    pub track: Account<'info, Track>,

    #[account(
        mut,
        seeds = [b"listing", track.key().as_ref(), seller.key().as_ref()],
        bump = listing.bump,
        has_one = seller,
        close = seller
    )]
    pub listing: Account<'info, Listing>,

    pub nft_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    pub track: Account<'info, Track>,

    #[account(
        mut,
        seeds = [b"listing", track.key().as_ref(), seller.key().as_ref()],
        bump = listing.bump,
        has_one = seller,
        constraint = listing.is_active @ ErrorCode::ListingInactive,
        constraint = listing.pricing_model == 0 @ ErrorCode::NotFixedPrice
    )]
    pub listing: Account<'info, Listing>,

    pub nft_mint: Account<'info, Mint>,
}

#[event]
pub struct PriceUpdated {
    pub seller: Pubkey,
    pub nft_mint: Pubkey,
    pub old_price: u64,
    pub new_price: u64,
}

#[account]
#[derive(InitSpace)]
pub struct CurveState {
    pub track: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,

    pub supply: u64,
    pub reserve: u64,

    // Pump.fun params
    pub base_price: u64, 
    pub k: u64,          
    pub tokens_per_nft: u64,

    pub bump: u8,
}

#[derive(Accounts)]
pub struct InitializeCurveForTrack<'info> {
    // Track authority
    #[account(mut)]
    pub authority: Signer<'info>,

    // Track PDA
    #[account(
        mut,
        has_one = authority
    )]
    pub track: Account<'info, Track>,

    // Curve State PDA
    #[account(
        init,
        payer = authority,
        space = 8 + CurveState::INIT_SPACE,
        seeds = [b"curve", track.key().as_ref()],
        bump
    )]
    pub curve: Account<'info, CurveState>,

    // Curve Token Mint PDA
    #[account(
        init,
        payer = authority,
        seeds = [b"curve_mint", track.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = curve,
        mint::freeze_authority = curve
    )]
    pub curve_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// Track this curve belongs to
    pub track: Account<'info, Track>,

    #[account(
        mut,
        seeds = [b"curve", track.key().as_ref()],
        bump = curve.bump
    )]
    pub curve: Account<'info, CurveState>,

    /// Marketplace config (for fees)
    #[account(
        seeds = [b"market"],
        bump = market.bump,
    )]
    pub market: Account<'info, MarketConfig>,

    /// CHECK: Curve vault PDA validated by `seeds = [b"curve_vault", track.key().as_ref()]`; used only as lamports recipient/sender.
    #[account(
        mut,
        seeds = [b"curve_vault", track.key().as_ref()],
        bump
    )]
    pub treasury: UncheckedAccount<'info>,

    /// CHECK: Marketplace treasury PDA validated by `seeds = [b"treasury"]` and `bump = market.treasury_bump`; only receives fee transfers.
    #[account(
        mut,
        seeds = [b"treasury"],
        bump = market.treasury_bump
    )]
    pub marketplace_treasury: UncheckedAccount<'info>,

    /// Mint of continuous tokens
    #[account(mut)]
    pub share_mint: Account<'info, Mint>,

    /// Buyer token ATA
    #[account(mut)]
    pub buyer_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SellTokens<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    pub track: Account<'info, Track>,

    #[account(
        mut,
        seeds = [b"curve", track.key().as_ref()],
        bump = curve.bump
    )]
    pub curve: Account<'info, CurveState>,

    // Marketplace config (for fees)
    #[account(
        seeds = [b"market"],
        bump = market.bump,
    )]
    pub market: Account<'info, MarketConfig>,

    /// CHECK: Curve vault PDA validated by `seeds = [b"curve_vault", track.key().as_ref()]`; used for SOL liquidity transfers only.
    #[account(
        mut,
        seeds = [b"curve_vault", track.key().as_ref()],
        bump
    )]
    pub treasury: UncheckedAccount<'info>,

    /// CHECK: Marketplace treasury PDA validated by `seeds = [b"treasury"]` and `bump = market.treasury_bump`; receives marketplace fees.
    #[account(
        mut,
        seeds = [b"treasury"],
        bump = market.treasury_bump,
    )]
    pub marketplace_treasury: UncheckedAccount<'info>,

    #[account(mut)]
    pub share_mint: Account<'info, Mint>,

    #[account(mut)]
    pub seller_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyNftWithCurvePrice<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    pub track: Account<'info, Track>,

    #[account(
        seeds = [b"curve", track.key().as_ref()],
        bump = curve.bump
    )]
    pub curve: Account<'info, CurveState>,

    /// CHECK: Curve vault PDA validated by `seeds = [b"curve_vault", track.key().as_ref()]`; used as SOL treasury for curve pricing.
    #[account(
        mut,
        seeds = [b"curve_vault", track.key().as_ref()],
        bump
    )]
    pub treasury: UncheckedAccount<'info>,

    #[account(mut)]
    pub stem_mint: Account<'info, Mint>,

    #[account(mut)]
    pub buyer_nft_account: Account<'info, TokenAccount>,

    /// CHECK: PDA mint authority
    pub mint_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}