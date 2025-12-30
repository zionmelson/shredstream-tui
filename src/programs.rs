use std::collections::HashMap;
use solana_sdk::pubkey::Pubkey;

/// Well-known program IDs for MEV-relevant protocols
pub struct KnownPrograms;

impl KnownPrograms {
    // DEX Programs
    pub const JUPITER_V6: &'static str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";
    pub const JUPITER_LIMIT: &'static str = "jupoNjAxXgZ4rjzxzPMP4oxduvQsQtZzyknqvzYNrNu";
    pub const RAYDIUM_V4: &'static str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
    pub const RAYDIUM_CLMM: &'static str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
    pub const RAYDIUM_CP: &'static str = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C";
    pub const ORCA_WHIRLPOOL: &'static str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
    pub const ORCA_TOKEN_SWAP: &'static str = "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP";
    pub const METEORA_DLMM: &'static str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";
    pub const METEORA_POOLS: &'static str = "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB";
    pub const LIFINITY_V2: &'static str = "2wT8Yq49kHgDzXuPxZSaeLaH1qbmGXtEyPy64bL7aD3c";
    pub const PHOENIX: &'static str = "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY";
    pub const OPENBOOK_V2: &'static str = "opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb";
    
    // Lending/Liquidation Programs
    pub const MARGINFI: &'static str = "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA";
    pub const KAMINO_LENDING: &'static str = "KLend2g3cP87ber41DLZqb3z4DfMaBqax8Tv1Kqpvwj";
    pub const SOLEND: &'static str = "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo";
    pub const DRIFT: &'static str = "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH";
    
    // Staking/LST Programs  
    pub const MARINADE: &'static str = "MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD";
    pub const JITO_STAKE: &'static str = "Jito4APyf642JPZPx3hGc6WWJ8zPKtRbRs4P815Awbb";
    pub const SANCTUM: &'static str = "5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx";
    
    // MEV/Bundle Programs
    pub const JITO_TIP: &'static str = "T1pyyaTNZsKv2WcRAB8oVnk93mLJw2XzjtVYqCsaHqt";
    pub const JITO_BUNDLE: &'static str = "BundLEbyuDmhRKZJd7t5a3FiVqbzmdMBJhYLQbSCfvP";
    
    // Token Programs
    pub const TOKEN_PROGRAM: &'static str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    pub const TOKEN_2022: &'static str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
    pub const ASSOCIATED_TOKEN: &'static str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
    
    pub fn get_all() -> HashMap<Pubkey, ProgramInfo> {
        let mut map = HashMap::new();
        
        // DEXes
        map.insert(Self::parse(Self::JUPITER_V6), ProgramInfo::new("Jupiter V6", ProgramCategory::Dex));
        map.insert(Self::parse(Self::JUPITER_LIMIT), ProgramInfo::new("Jupiter Limit", ProgramCategory::Dex));
        map.insert(Self::parse(Self::RAYDIUM_V4), ProgramInfo::new("Raydium V4", ProgramCategory::Dex));
        map.insert(Self::parse(Self::RAYDIUM_CLMM), ProgramInfo::new("Raydium CLMM", ProgramCategory::Dex));
        map.insert(Self::parse(Self::RAYDIUM_CP), ProgramInfo::new("Raydium CP", ProgramCategory::Dex));
        map.insert(Self::parse(Self::ORCA_WHIRLPOOL), ProgramInfo::new("Orca Whirlpool", ProgramCategory::Dex));
        map.insert(Self::parse(Self::ORCA_TOKEN_SWAP), ProgramInfo::new("Orca Swap", ProgramCategory::Dex));
        map.insert(Self::parse(Self::METEORA_DLMM), ProgramInfo::new("Meteora DLMM", ProgramCategory::Dex));
        map.insert(Self::parse(Self::METEORA_POOLS), ProgramInfo::new("Meteora Pools", ProgramCategory::Dex));
        map.insert(Self::parse(Self::LIFINITY_V2), ProgramInfo::new("Lifinity V2", ProgramCategory::Dex));
        map.insert(Self::parse(Self::PHOENIX), ProgramInfo::new("Phoenix", ProgramCategory::Dex));
        map.insert(Self::parse(Self::OPENBOOK_V2), ProgramInfo::new("OpenBook V2", ProgramCategory::Dex));
        
        // Lending
        map.insert(Self::parse(Self::MARGINFI), ProgramInfo::new("MarginFi", ProgramCategory::Lending));
        map.insert(Self::parse(Self::KAMINO_LENDING), ProgramInfo::new("Kamino", ProgramCategory::Lending));
        map.insert(Self::parse(Self::SOLEND), ProgramInfo::new("Solend", ProgramCategory::Lending));
        map.insert(Self::parse(Self::DRIFT), ProgramInfo::new("Drift", ProgramCategory::Lending));
        
        // Staking
        map.insert(Self::parse(Self::MARINADE), ProgramInfo::new("Marinade", ProgramCategory::Staking));
        map.insert(Self::parse(Self::JITO_STAKE), ProgramInfo::new("Jito Stake", ProgramCategory::Staking));
        map.insert(Self::parse(Self::SANCTUM), ProgramInfo::new("Sanctum", ProgramCategory::Staking));
        
        // MEV
        map.insert(Self::parse(Self::JITO_TIP), ProgramInfo::new("Jito Tips", ProgramCategory::Mev));
        map.insert(Self::parse(Self::JITO_BUNDLE), ProgramInfo::new("Jito Bundle", ProgramCategory::Mev));
        
        map
    }
    
    fn parse(s: &str) -> Pubkey {
        s.parse().unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgramCategory {
    Dex,
    Lending,
    Staking,
    Mev,
    Token,
    Other,
}

impl std::fmt::Display for ProgramCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgramCategory::Dex => write!(f, "DEX"),
            ProgramCategory::Lending => write!(f, "Lending"),
            ProgramCategory::Staking => write!(f, "Staking"),
            ProgramCategory::Mev => write!(f, "MEV"),
            ProgramCategory::Token => write!(f, "Token"),
            ProgramCategory::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgramInfo {
    pub name: String,
    pub category: ProgramCategory,
}

impl ProgramInfo {
    pub fn new(name: &str, category: ProgramCategory) -> Self {
        Self {
            name: name.to_string(),
            category,
        }
    }
}

/// Known MEV bot addresses (add more as discovered)
pub struct KnownBots;

impl KnownBots {
    pub fn get_all() -> HashMap<Pubkey, BotInfo> {
        let mut map = HashMap::new();
        // Add known bot addresses here as they're discovered
        // Example: map.insert(pubkey, BotInfo::new("Bot Name", BotType::Arbitrage));
        map
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotType {
    Arbitrage,
    Liquidation,
    Sandwich,
    Backrun,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct BotInfo {
    pub name: String,
    pub bot_type: BotType,
}

impl BotInfo {
    pub fn new(name: &str, bot_type: BotType) -> Self {
        Self {
            name: name.to_string(),
            bot_type,
        }
    }
}

/// Jito tip accounts for bundle detection
pub const JITO_TIP_ACCOUNTS: [&str; 8] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4bVa5Zp9xzzLnX5BQ6qB3m9",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPPaKc",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];
