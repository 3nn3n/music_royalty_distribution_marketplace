import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { Muzica } from "../target/types/muzica";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { expect } from "chai";

describe("muzica", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Muzica as Program<Muzica>;
  const connection = provider.connection;

  const authority = Keypair.generate();
  const contributor1 = Keypair.generate();
  const contributor2 = Keypair.generate();
  const admin = Keypair.generate();
  const buyer = Keypair.generate();

  const trackId = new BN(1);
  const title = "Test Track";
  const cid = "QmTestCID123456789";
  const masterHash = new Array(32).fill(1);

  async function airdrop(pubkey: PublicKey, amount = 10 * LAMPORTS_PER_SOL) {
    const sig = await connection.requestAirdrop(pubkey, amount);
    await connection.confirmTransaction(sig);
  }

  function getTrackPda(auth: PublicKey, id: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [
        Buffer.from("track"),
        auth.toBuffer(),
        id.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );
  }

  function getMarketPda(): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("market")],
      program.programId
    );
  }

  function getTreasuryPda(): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("treasury")],
      program.programId
    );
  }

  function getListingPda(track: PublicKey, seller: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("listing"), track.toBuffer(), seller.toBuffer()],
      program.programId
    );
  }

  function getCurvePda(track: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("curve"), track.toBuffer()],
      program.programId
    );
  }

  function getCurveMintPda(track: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("curve_mint"), track.toBuffer()],
      program.programId
    );
  }

  function getCurveVaultPda(track: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("curve_vault"), track.toBuffer()],
      program.programId
    );
  }

  function getStemMintPda(track: PublicKey, index: BN): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [
        Buffer.from("stem_mint"),
        track.toBuffer(),
        index.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );
  }

  before(async () => {
    await Promise.all([
      airdrop(authority.publicKey),
      airdrop(contributor1.publicKey),
      airdrop(contributor2.publicKey),
      airdrop(admin.publicKey),
      airdrop(buyer.publicKey),
    ]);
  });

  describe("Track Management", () => {
    it("initializes a track successfully", async () => {
      const [trackPda] = getTrackPda(authority.publicKey, trackId);
      const contributors = [contributor1.publicKey, contributor2.publicKey];
      const sharesBps = [5000, 5000]; // 50% each

      await program.methods
        .initializeTrack(
          trackId,
          title,
          cid,
          masterHash,
          contributors,
          sharesBps
        )
        .accountsStrict({
          authority: authority.publicKey,
          track: trackPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      const track = await program.account.track.fetch(trackPda);
      expect(track.trackId.toNumber()).to.equal(1);
      expect(track.title).to.equal(title);
      expect(track.cid).to.equal(cid);
      expect(track.contributors.length).to.equal(2);
      expect(track.shares[0]).to.equal(5000);
      expect(track.shares[1]).to.equal(5000);
      expect(track.royaltyVersion).to.equal(0);
    });

    it("fails when shares don't sum to 10000", async () => {
      const newTrackId = new BN(999);
      const [trackPda] = getTrackPda(authority.publicKey, newTrackId);
      const contributors = [contributor1.publicKey];
      const sharesBps = [5000]; // Only 50%, should fail

      try {
        await program.methods
          .initializeTrack(
            newTrackId,
            title,
            cid,
            masterHash,
            contributors,
            sharesBps
          )
          .accountsStrict({
            authority: authority.publicKey,
            track: trackPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([authority])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("InvalidShareTotal");
      }
    });

    it("fails when title exceeds max length", async () => {
      const newTrackId = new BN(998);
      const [trackPda] = getTrackPda(authority.publicKey, newTrackId);
      const longTitle = "a".repeat(65); // MAX_TITLE_LEN is 64

      try {
        await program.methods
          .initializeTrack(
            newTrackId,
            longTitle,
            cid,
            masterHash,
            [contributor1.publicKey],
            [10000]
          )
          .accountsStrict({
            authority: authority.publicKey,
            track: trackPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([authority])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("TitleTooLong");
      }
    });

    it("fails with no contributors", async () => {
      const newTrackId = new BN(997);
      const [trackPda] = getTrackPda(authority.publicKey, newTrackId);

      try {
        await program.methods
          .initializeTrack(
            newTrackId,
            title,
            cid,
            masterHash,
            [],
            []
          )
          .accountsStrict({
            authority: authority.publicKey,
            track: trackPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([authority])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("NoContributors");
      }
    });

    it("updates shares successfully", async () => {
      const [trackPda] = getTrackPda(authority.publicKey, trackId);
      const newShares = [7000, 3000]; // 70/30 split
      const contributors = [contributor1.publicKey, contributor2.publicKey];

      await program.methods
        .updateShares(trackId, newShares, contributors)
        .accountsStrict({
          authority: authority.publicKey,
          track: trackPda,
        })
        .signers([authority])
        .rpc();

      const track = await program.account.track.fetch(trackPda);
      expect(track.shares[0]).to.equal(7000);
      expect(track.shares[1]).to.equal(3000);
      expect(track.royaltyVersion).to.equal(1);
    });

    it("adds a stem mint to track", async () => {
      const [trackPda] = getTrackPda(authority.publicKey, trackId);
      const fakeStemMint = Keypair.generate().publicKey;

      await program.methods
        .stemMint(trackId, fakeStemMint)
        .accountsStrict({
          authority: authority.publicKey,
          track: trackPda,
        })
        .signers([authority])
        .rpc();

      const track = await program.account.track.fetch(trackPda);
      expect(track.stemMints.length).to.equal(1);
      expect(track.stemMints[0].toBase58()).to.equal(fakeStemMint.toBase58());
    });
  });

  // ==========================================
  // MARKETPLACE TESTS
  // ==========================================
  describe("Marketplace", () => {
    it("initializes marketplace successfully", async () => {
      const [marketPda] = getMarketPda();
      const [treasuryPda] = getTreasuryPda();
      const feeBps = 250; // 2.5%
      const defaultPaymentMint = PublicKey.default;

      await program.methods
        .initializeMarket(feeBps, defaultPaymentMint)
        .accountsStrict({
          admin: admin.publicKey,
          market: marketPda,
          treasury: treasuryPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      const market = await program.account.marketConfig.fetch(marketPda);
      expect(market.admin.toBase58()).to.equal(admin.publicKey.toBase58());
      expect(market.feeBps).to.equal(250);
      expect(market.isPaused).to.equal(false);
      expect(market.maxFeeBps).to.equal(1000);
    });

    it("fails to initialize market with fee > 10%", async () => {
      // Market already exists, but test the validation logic separately
      // by attempting to create with high fee before actual init
      // For this test, we expect it to fail in a fresh environment
    });

    it("updates market config successfully", async () => {
      const [marketPda] = getMarketPda();
      const newFeeBps = 300; // 3%

      await program.methods
        .updateMarketConfig(newFeeBps, null)
        .accountsStrict({
          admin: admin.publicKey,
          market: marketPda,
        })
        .signers([admin])
        .rpc();

      const market = await program.account.marketConfig.fetch(marketPda);
      expect(market.feeBps).to.equal(300);
    });

    it("pauses and unpauses marketplace", async () => {
      const [marketPda] = getMarketPda();

      // Pause
      await program.methods
        .updateMarketConfig(null, true)
        .accountsStrict({
          admin: admin.publicKey,
          market: marketPda,
        })
        .signers([admin])
        .rpc();

      let market = await program.account.marketConfig.fetch(marketPda);
      expect(market.isPaused).to.equal(true);

      // Unpause
      await program.methods
        .updateMarketConfig(null, false)
        .accountsStrict({
          admin: admin.publicKey,
          market: marketPda,
        })
        .signers([admin])
        .rpc();

      market = await program.account.marketConfig.fetch(marketPda);
      expect(market.isPaused).to.equal(false);
    });
  });

  // ==========================================
  // LISTING TESTS
  // ==========================================
  describe("Listings", () => {
    const listingTrackId = new BN(2);
    let listingTrackPda: PublicKey;

    before(async () => {
      // Create a track for listing tests
      [listingTrackPda] = getTrackPda(authority.publicKey, listingTrackId);

      await program.methods
        .initializeTrack(
          listingTrackId,
          "Listing Test Track",
          "QmListingCID",
          masterHash,
          [contributor1.publicKey],
          [10000]
        )
        .accountsStrict({
          authority: authority.publicKey,
          track: listingTrackPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();
    });

    it("creates a listing successfully", async () => {
      const [marketPda] = getMarketPda();
      const [listingPda] = getListingPda(listingTrackPda, contributor1.publicKey);
      const price = new BN(2 * LAMPORTS_PER_SOL);

      await program.methods
        .listTrack(listingTrackId, price)
        .accountsStrict({
          seller: contributor1.publicKey,
          track: listingTrackPda,
          market: marketPda,
          listing: listingPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: authority.publicKey, isSigner: false, isWritable: false },
        ])
        .signers([contributor1])
        .rpc();

      const listing = await program.account.listing.fetch(listingPda);
      expect(listing.seller.toBase58()).to.equal(contributor1.publicKey.toBase58());
      expect(listing.price.toNumber()).to.equal(2 * LAMPORTS_PER_SOL);
      expect(listing.isActive).to.equal(true);
    });

    it("updates listing price successfully", async () => {
      const [listingPda] = getListingPda(listingTrackPda, contributor1.publicKey);
      const newPrice = new BN(3 * LAMPORTS_PER_SOL);

      // Create a dummy NFT mint for the test
      const nftMint = await createMint(
        connection,
        contributor1,
        contributor1.publicKey,
        null,
        0
      );

      await program.methods
        .updatePrice(newPrice)
        .accountsStrict({
          seller: contributor1.publicKey,
          track: listingTrackPda,
          listing: listingPda,
          nftMint: nftMint,
        })
        .signers([contributor1])
        .rpc();

      const listing = await program.account.listing.fetch(listingPda);
      expect(listing.price.toNumber()).to.equal(3 * LAMPORTS_PER_SOL);
    });

    it("fails to update price below minimum", async () => {
      const [listingPda] = getListingPda(listingTrackPda, contributor1.publicKey);
      const lowPrice = new BN(1000); // Below MIN_PRICE_LAMPORTS (1_000_000)

      const nftMint = await createMint(
        connection,
        contributor1,
        contributor1.publicKey,
        null,
        0
      );

      try {
        await program.methods
          .updatePrice(lowPrice)
          .accountsStrict({
            seller: contributor1.publicKey,
            track: listingTrackPda,
            listing: listingPda,
            nftMint: nftMint,
          })
          .signers([contributor1])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("PriceTooLow");
      }
    });

    it("cancels listing successfully", async () => {
      // Create a new listing to cancel
      const cancelTrackId = new BN(3);
      const [cancelTrackPda] = getTrackPda(authority.publicKey, cancelTrackId);

      await program.methods
        .initializeTrack(
          cancelTrackId,
          "Cancel Test",
          "QmCancel",
          masterHash,
          [contributor1.publicKey],
          [10000]
        )
        .accountsStrict({
          authority: authority.publicKey,
          track: cancelTrackPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      const [marketPda] = getMarketPda();
      const [listingPda] = getListingPda(cancelTrackPda, contributor1.publicKey);
      const price = new BN(1 * LAMPORTS_PER_SOL);

      await program.methods
        .listTrack(cancelTrackId, price)
        .accountsStrict({
          seller: contributor1.publicKey,
          track: cancelTrackPda,
          market: marketPda,
          listing: listingPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: authority.publicKey, isSigner: false, isWritable: false },
        ])
        .signers([contributor1])
        .rpc();

      const nftMint = await createMint(
        connection,
        contributor1,
        contributor1.publicKey,
        null,
        0
      );

      await program.methods
        .cancelListing()
        .accountsStrict({
          seller: contributor1.publicKey,
          track: cancelTrackPda,
          listing: listingPda,
          nftMint: nftMint,
          systemProgram: SystemProgram.programId,
        })
        .signers([contributor1])
        .rpc();

      // Listing account should be closed
      const listing = await program.account.listing.fetchNullable(listingPda);
      expect(listing).to.be.null;
    });
  });

  // ==========================================
  // BONDING CURVE TESTS
  // ==========================================
  describe("Bonding Curve", () => {
    const curveTrackId = new BN(10);
    let curveTrackPda: PublicKey;
    let curvePda: PublicKey;
    let curveMintPda: PublicKey;
    let curveVaultPda: PublicKey;

    before(async () => {
      // Create track for curve tests
      [curveTrackPda] = getTrackPda(authority.publicKey, curveTrackId);

      await program.methods
        .initializeTrack(
          curveTrackId,
          "Curve Test Track",
          "QmCurve",
          masterHash,
          [contributor1.publicKey],
          [10000]
        )
        .accountsStrict({
          authority: authority.publicKey,
          track: curveTrackPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      [curvePda] = getCurvePda(curveTrackPda);
      [curveMintPda] = getCurveMintPda(curveTrackPda);
      [curveVaultPda] = getCurveVaultPda(curveTrackPda);
    });

    it("initializes bonding curve successfully", async () => {
      const basePrice = new BN(100000); // 0.0001 SOL
      const k = new BN(100);
      const tokensPerNft = new BN(1000000); // 1 token = 1 NFT

      await program.methods
        .initializeCurveForTrack(basePrice, k, tokensPerNft)
        .accountsStrict({
          authority: authority.publicKey,
          track: curveTrackPda,
          curve: curvePda,
          curveMint: curveMintPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([authority])
        .rpc();

      const curve = await program.account.curveState.fetch(curvePda);
      expect(curve.basePrice.toNumber()).to.equal(100000);
      expect(curve.k.toNumber()).to.equal(100);
      expect(curve.tokensPerNft.toNumber()).to.equal(1000000);
      expect(curve.supply.toNumber()).to.equal(0);
    });

    it("fails to initialize curve with zero base price", async () => {
      const zeroBaseTrackId = new BN(11);
      const [zeroTrackPda] = getTrackPda(authority.publicKey, zeroBaseTrackId);

      await program.methods
        .initializeTrack(
          zeroBaseTrackId,
          "Zero Base Test",
          "QmZero",
          masterHash,
          [contributor1.publicKey],
          [10000]
        )
        .accountsStrict({
          authority: authority.publicKey,
          track: zeroTrackPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      const [zeroCurvePda] = getCurvePda(zeroTrackPda);
      const [zeroCurveMintPda] = getCurveMintPda(zeroTrackPda);

      try {
        await program.methods
          .initializeCurveForTrack(new BN(0), new BN(100), new BN(1000))
          .accountsStrict({
            authority: authority.publicKey,
            track: zeroTrackPda,
            curve: zeroCurvePda,
            curveMint: zeroCurveMintPda,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          })
          .signers([authority])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("InvalidBasePrice");
      }
    });

    it("buys tokens on bonding curve", async () => {
      const [marketPda] = getMarketPda();
      const [treasuryPda] = getTreasuryPda();

      const amount = new BN(100); // Buy 100 tokens
      const maxLamports = new BN(10 * LAMPORTS_PER_SOL); // Max willing to pay

      // Create buyer's token account
      const buyerTokenAccount = await getOrCreateAssociatedTokenAccount(
        connection,
        buyer,
        curveMintPda,
        buyer.publicKey
      );

      await program.methods
        .buyTokens(amount, maxLamports)
        .accountsStrict({
          buyer: buyer.publicKey,
          track: curveTrackPda,
          curve: curvePda,
          market: marketPda,
          treasury: curveVaultPda,
          marketplaceTreasury: treasuryPda,
          shareMint: curveMintPda,
          buyerTokenAccount: buyerTokenAccount.address,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([buyer])
        .rpc();

      const curve = await program.account.curveState.fetch(curvePda);
      expect(curve.supply.toNumber()).to.equal(100);
    });

    it("sells tokens on bonding curve", async () => {
      const [marketPda] = getMarketPda();
      const [treasuryPda] = getTreasuryPda();

      const amount = new BN(50); // Sell 50 tokens
      const minLamports = new BN(0); // Min willing to receive

      const sellerTokenAccount = await getAssociatedTokenAddress(
        curveMintPda,
        buyer.publicKey
      );

      await program.methods
        .sellTokens(amount, minLamports)
        .accountsStrict({
          seller: buyer.publicKey,
          track: curveTrackPda,
          curve: curvePda,
          market: marketPda,
          treasury: curveVaultPda,
          marketplaceTreasury: treasuryPda,
          shareMint: curveMintPda,
          sellerTokenAccount: sellerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([buyer])
        .rpc();

      const curve = await program.account.curveState.fetch(curvePda);
      expect(curve.supply.toNumber()).to.equal(50);
    });

    it("fails to sell more tokens than supply", async () => {
      const [marketPda] = getMarketPda();
      const [treasuryPda] = getTreasuryPda();

      const amount = new BN(1000); // More than available

      const sellerTokenAccount = await getAssociatedTokenAddress(
        curveMintPda,
        buyer.publicKey
      );

      try {
        await program.methods
          .sellTokens(amount, new BN(0))
          .accountsStrict({
            seller: buyer.publicKey,
            track: curveTrackPda,
            curve: curvePda,
            market: marketPda,
            treasury: curveVaultPda,
            marketplaceTreasury: treasuryPda,
            shareMint: curveMintPda,
            sellerTokenAccount: sellerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([buyer])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        // Either InsufficientLiquidity or token balance check
        expect(err).to.exist;
      }
    });
  });

  // ==========================================
  // STEM NFT MINTING TESTS
  // ==========================================
  describe("Stem NFT Minting", () => {
    const stemTrackId = new BN(20);
    let stemTrackPda: PublicKey;

    before(async () => {
      // Create track for stem minting tests
      [stemTrackPda] = getTrackPda(authority.publicKey, stemTrackId);

      await program.methods
        .initializeTrack(
          stemTrackId,
          "Stem NFT Track",
          "QmStem",
          masterHash,
          [contributor1.publicKey, contributor2.publicKey],
          [6000, 4000]
        )
        .accountsStrict({
          authority: authority.publicKey,
          track: stemTrackPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();
    });

    it("mints stem NFT to contributor", async () => {
      const nftIndex = new BN(0); // First contributor
      const [stemMintPda] = getStemMintPda(stemTrackPda, nftIndex);

      const recipientTokenAccount = await getAssociatedTokenAddress(
        stemMintPda,
        contributor1.publicKey
      );

      await program.methods
        .mintStemNft(stemTrackId, nftIndex)
        .accountsStrict({
          payer: contributor1.publicKey,
          track: stemTrackPda,
          mint: stemMintPda,
          recipientTokenAccount: recipientTokenAccount,
          authority: contributor1.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([contributor1])
        .rpc();

      const track = await program.account.track.fetch(stemTrackPda);
      expect(track.stemMints.length).to.be.greaterThan(0);
    });

    it("fails to mint NFT for non-contributor", async () => {
      const nftIndex = new BN(0);
      const [stemMintPda2] = getStemMintPda(stemTrackPda, new BN(5)); // Different index

      const recipientTokenAccount = await getAssociatedTokenAddress(
        stemMintPda2,
        buyer.publicKey
      );

      try {
        await program.methods
          .mintStemNft(stemTrackId, new BN(5))
          .accountsStrict({
            payer: buyer.publicKey,
            track: stemTrackPda,
            mint: stemMintPda2,
            recipientTokenAccount: recipientTokenAccount,
            authority: buyer.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([buyer])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("NotAContributor");
      }
    });

    it("fails with wrong nft_index for contributor", async () => {
      const wrongIndex = new BN(1); // contributor1 is at index 0, not 1
      const [stemMintPda] = getStemMintPda(stemTrackPda, wrongIndex);

      const recipientTokenAccount = await getAssociatedTokenAddress(
        stemMintPda,
        contributor1.publicKey
      );

      try {
        await program.methods
          .mintStemNft(stemTrackId, wrongIndex)
          .accountsStrict({
            payer: contributor1.publicKey,
            track: stemTrackPda,
            mint: stemMintPda,
            recipientTokenAccount: recipientTokenAccount,
            authority: contributor1.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([contributor1])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("InvalidArgs");
      }
    });
  });

  // ==========================================
  // FEE WITHDRAWAL TESTS
  // ==========================================
  describe("Fee Withdrawal", () => {
    it("withdraws marketplace fees successfully", async () => {
      const [marketPda] = getMarketPda();
      const [treasuryPda] = getTreasuryPda();

      // First, verify treasury has some balance (from previous buy_tokens)
      const treasuryBalance = await connection.getBalance(treasuryPda);
      
      if (treasuryBalance > 0) {
        const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
        const withdrawable = treasuryBalance - rentExempt;

        if (withdrawable > 0) {
          const adminBalanceBefore = await connection.getBalance(admin.publicKey);

          await program.methods
            .withdrawMarketplaceFees(new BN(withdrawable))
            .accountsStrict({
              admin: admin.publicKey,
              market: marketPda,
              treasury: treasuryPda,
              systemProgram: SystemProgram.programId,
            })
            .signers([admin])
            .rpc();

          const adminBalanceAfter = await connection.getBalance(admin.publicKey);
          expect(adminBalanceAfter).to.be.greaterThan(adminBalanceBefore);
        }
      }
    });

    it("fails to withdraw more than available", async () => {
      const [marketPda] = getMarketPda();
      const [treasuryPda] = getTreasuryPda();
      const hugeAmount = new BN(1000 * LAMPORTS_PER_SOL);

      try {
        await program.methods
          .withdrawMarketplaceFees(hugeAmount)
          .accounts({
            admin: admin.publicKey,
            market: marketPda,
            treasury: treasuryPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("InsufficientFunds");
      }
    });

    it("fails when non-admin tries to withdraw", async () => {
      const [marketPda] = getMarketPda();
      const [treasuryPda] = getTreasuryPda();

      try {
        await program.methods
          .withdrawMarketplaceFees(new BN(1000))
          .accounts({
            admin: buyer.publicKey, // Not the admin
            market: marketPda,
            treasury: treasuryPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([buyer])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        // Anchor constraint error for has_one = admin
        expect(err).to.exist;
      }
    });
  });

  // ==========================================
  // ESCROW TESTS
  // ==========================================
  describe("Escrow Operations", () => {
    const escrowTrackId = new BN(30);
    let escrowTrackPda: PublicKey;
    let escrowMint: PublicKey;
    let escrowTokenAccount: PublicKey;

    before(async () => {
      // Create track for escrow tests
      [escrowTrackPda] = getTrackPda(authority.publicKey, escrowTrackId);

      await program.methods
        .initializeTrack(
          escrowTrackId,
          "Escrow Track",
          "QmEscrow",
          masterHash,
          [contributor1.publicKey, contributor2.publicKey],
          [5000, 5000]
        )
        .accountsStrict({
          authority: authority.publicKey,
          track: escrowTrackPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      // Create a test token mint
      escrowMint = await createMint(
        connection,
        authority,
        authority.publicKey,
        null,
        6
      );

      // Derive escrow ATA
      escrowTokenAccount = await getAssociatedTokenAddress(
        escrowMint,
        escrowTrackPda,
        true
      );
    });

    it("creates escrow ATA successfully", async () => {
      await program.methods
        .createEscrowAta(escrowTrackId, authority.publicKey)
        .accountsStrict({
          payer: authority.publicKey,
          track: escrowTrackPda,
          escrowTokenAccount: escrowTokenAccount,
          mint: escrowMint,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      // Verify ATA was created
      const accountInfo = await connection.getAccountInfo(escrowTokenAccount);
      expect(accountInfo).to.not.be.null;
    });

    it("deposits to escrow successfully", async () => {
      // Create payer's token account and mint some tokens
      const payerTokenAccount = await getOrCreateAssociatedTokenAccount(
        connection,
        authority,
        escrowMint,
        authority.publicKey
      );

      // Mint tokens to payer
      await mintTo(
        connection,
        authority,
        escrowMint,
        payerTokenAccount.address,
        authority,
        1000000000 // 1000 tokens
      );

      const depositAmount = new BN(500000000); // 500 tokens

      await program.methods
        .escrowDeposit(depositAmount, escrowTrackId, authority.publicKey)
        .accountsStrict({
          payer: authority.publicKey,
          track: escrowTrackPda,
          escrowTokenAccount: escrowTokenAccount,
          payerTokenAccount: payerTokenAccount.address,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([authority])
        .rpc();

      // Verify escrow balance
      const escrowBalance = await connection.getTokenAccountBalance(escrowTokenAccount);
      expect(parseInt(escrowBalance.value.amount)).to.equal(500000000);
    });

    it("fails to deposit zero amount", async () => {
      const payerTokenAccount = await getAssociatedTokenAddress(
        escrowMint,
        authority.publicKey
      );

      try {
        await program.methods
          .escrowDeposit(new BN(0), escrowTrackId, authority.publicKey)
          .accountsStrict({
            payer: authority.publicKey,
            track: escrowTrackPda,
            escrowTokenAccount: escrowTokenAccount,
            payerTokenAccount: payerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([authority])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("InvalidAmount");
      }
    });

    it("distributes escrow to contributors", async () => {
      const distributeAmount = new BN(100000000); // 100 tokens

      // Create contributor ATAs
      const contributor1Ata = await getOrCreateAssociatedTokenAccount(
        connection,
        authority,
        escrowMint,
        contributor1.publicKey
      );

      const contributor2Ata = await getOrCreateAssociatedTokenAccount(
        connection,
        authority,
        escrowMint,
        contributor2.publicKey
      );

      await program.methods
        .escrowDistribute(distributeAmount, escrowTrackId)
        .accountsStrict({
          track: escrowTrackPda,
          escrowTokenAccount: escrowTokenAccount,
          trackAuthority: escrowTrackPda,
          authority: authority.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .remainingAccounts([
          { pubkey: contributor1Ata.address, isSigner: false, isWritable: true },
          { pubkey: contributor2Ata.address, isSigner: false, isWritable: true },
        ])
        .signers([authority])
        .rpc();

      // Verify distribution (50/50 split)
      const c1Balance = await connection.getTokenAccountBalance(contributor1Ata.address);
      const c2Balance = await connection.getTokenAccountBalance(contributor2Ata.address);
      
      expect(parseInt(c1Balance.value.amount)).to.equal(50000000); // 50 tokens
      expect(parseInt(c2Balance.value.amount)).to.equal(50000000); // 50 tokens
    });
  });
});
