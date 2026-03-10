# Muzica — On-Chain Music Royalty Protocol

A Solana program (Anchor) for registering music tracks, distributing royalties to contributors, minting stem NFTs, and trading them via a marketplace and bonding curve.

**Program ID:** `24iCyiUg1Vd5eGPEua31dVn7rYAuLiYBDkgpHbLhC9ob`

---

## Overview

Muzica lets artists and collaborators:

- Register a track on-chain with an IPFS CID, a master hash, and a contributor/share table
- Collect royalty payments into an escrow and atomically distribute them pro-rata
- Mint per-stem NFTs that represent each contributor's participation
- List and trade stem NFTs in a permissioned marketplace with configurable fees
- Price fungible share tokens along a quadratic bonding curve (pump.fun-style)

---

## Instructions

| Instruction | Description |
|---|---|
| `initialize_track` | Creates a `Track` PDA. Stores title, IPFS CID, master hash, contributors, and BPS shares that must total 10 000. |
| `stem_mint` | Registers an external stem mint address on the track (up to 64). |
| `update_shares` | Replaces the contributor/share table and bumps `royalty_version`. Authority-gated. |
| `create_escrow_ata` | Creates the track-owned ATA that holds escrowed tokens. |
| `escrow_deposit` | Transfers SPL tokens from a payer into the track escrow. |
| `escrow_distribute` | Splits the escrowed balance across all contributors according to their BPS shares. |
| `mint_stem_nft` | Mints a 1-of-1 NFT to a contributor. Mint is a PDA seeded by `["stem_mint", track, index]`. |
| `initialize_market` | Creates the singleton `Market` and `Treasury` PDAs. Sets fee (max 10 %). |
| `update_market_config` | Admin-only — change fee BPS or pause/unpause the market. |
| `withdraw_marketplace_fees` | Admin withdraws accumulated SOL fees from the treasury. |
| `list_track` | Seller creates a `Listing` PDA for a stem NFT at a chosen SOL price. |
| `buy_track` | Buyer pays the seller (minus fee), NFT transfers, listing closed. |
| `cancel_listing` | Seller deactivates their listing. |
| `update_price` | Seller changes the listing price (min 0.001 SOL). |
| `initialize_curve_for_track` | Deploys a quadratic bonding curve (`base_price + k·s²`) for a track's fungible share token. |
| `buy_tokens` | Buyer mints share tokens; cost is the integral under the curve. Slippage-protected. |
| `sell_tokens` | Seller burns share tokens and receives SOL from the vault. Slippage-protected. |
| `buy_nft_with_curve_price` | Mints a stem NFT at the current bonding-curve spot price × `tokens_per_nft`. |

---

## Account PDAs

| Account | Seeds |
|---|---|
| `Track` | `["track", authority, track_id (le64)]` |
| `Marketplace` | `["market"]` |
| `Treasury` | `["treasury"]` |
| `Listing` | `["listing", track, seller]` |
| `CurveState` | `["curve", track]` |
| `CurveMint` | `["curve_mint", track]` |
| `CurveVault` | `["curve_vault", track]` |
| Stem NFT mint | `["stem_mint", track, index (le64)]` |

---

## Royalty Shares

Shares are expressed in **basis points** (BPS). The sum across all contributors must equal exactly **10 000** (100 %). Up to **16 contributors** per track. The `royalty_version` counter is incremented on every `update_shares` call so listeners can detect stale listings.

---

## Bonding Curve

Price model: $P(s) = \text{base\_price} + k \cdot s^2$

Cost to buy $\Delta$ tokens from supply $s$:

$$\text{cost} = \text{base\_price} \cdot \Delta + \frac{k}{3}\left[(s+\Delta)^3 - s^3\right]$$

Refund for selling $\Delta$ tokens is the symmetric integral. Both `buy_tokens` and `sell_tokens` accept slippage guards (`max_lamports` / `min_lamports`).

---

## Getting Started

### Prerequisites

- Rust + `cargo` (Solana BPF target)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor CLI](https://www.anchor-lang.com/docs/installation) `0.31.x`
- Node.js + Yarn

### Install

```bash
yarn install
```

### Build

```bash
anchor build
```

### Test (localnet)

```bash
anchor test
```

### Deploy

```bash
# devnet
anchor deploy --provider.cluster devnet
```

---

## Project Structure

```
programs/muzica/src/lib.rs   # All on-chain logic
tests/muzica.ts              # Integration tests (Anchor + web3.js)
target/idl/muzica.json       # Generated IDL
target/types/muzica.ts       # Generated TypeScript types
migrations/deploy.ts         # Anchor migration hook
```

---

## Constants

| Constant | Value |
|---|---|
| `MAX_TITLE_LEN` | 64 bytes |
| `MAX_CID_LEN` | 128 bytes |
| `MAX_CONTRIBUTORS` | 16 |
| `MIN_PRICE_LAMPORTS` | 1 000 000 (0.001 SOL) |
| Max marketplace fee | 1 000 BPS (10 %) |

---

## License

ISC
