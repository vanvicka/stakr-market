use {
    anchor_lang::{
        solana_program::{
            instruction::{AccountMeta, Instruction},
            pubkey::Pubkey,
        },
        Discriminator, InstructionData,
    },
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    stakr_market::state::{Market, WINNING_OUTCOME_NONE},
};

#[test]
fn test_resolve_market() {
    let program_id = stakr_market::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/stakr_market.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let created_at: i64 = 0;
    let created_at_le = created_at.to_le_bytes();
    let authority = payer.pubkey();
    let seeds = &[
        b"market".as_slice(),
        authority.as_ref(),
        created_at_le.as_slice(),
    ];
    let (market_pda, bump) = Pubkey::find_program_address(seeds, &program_id);

    let market = Market {
        authority: payer.pubkey(),
        description: "test".to_string(),
        rules: "test".to_string(),
        created_at,
        deadline: 0,
        resolved: false,
        resolved_at: 0,
        winning_outcome: WINNING_OUTCOME_NONE,
        q_yes: 0,
        q_no: 0,
        b: 1_000_000_000,
        pool_balance: 0,
        mint: program_id,
        mint_decimals: 0,
        market_bump: bump,
        category: "Lottery".to_string(),
        avatar_url: String::new(),
    };

    let mut data = Market::DISCRIMINATOR.to_vec();
    data.extend(market.try_to_vec().unwrap());
    let lamports = svm.minimum_balance_for_rent_exemption(data.len());
    svm.set_account(
        market_pda,
        Account {
            lamports,
            data,
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let instruction = Instruction::new_with_bytes(
        program_id,
        &stakr_market::instruction::ResolveMarket { winning_outcome: 0 }.data(),
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(market_pda, false),
        ],
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "resolve_market should succeed: {:?}", res.err());
}
