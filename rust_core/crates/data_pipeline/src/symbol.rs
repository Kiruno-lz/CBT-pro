//! Symbol normalization for exchange-specific trading pair formats.
//!
//! Converts exchange-specific formats (e.g. `BTCUSDT`, `BTC-USDT`) into the
//! CBT-Pro standard `BASE/QUOTE` format.

use std::collections::HashSet;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during symbol normalization.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum NormalizationError {
    #[error("Unknown symbol format: {0}")]
    UnknownFormat(String),
    #[error("Ambiguous parsing for '{input}': possible candidates are {candidates:?}")]
    AmbiguousParsing {
        input: String,
        candidates: Vec<String>,
    },
    #[error("Invalid base token: {0}")]
    InvalidBase(String),
    #[error("Invalid quote token: {0}")]
    InvalidQuote(String),
    #[error("Unknown exchange: {0}")]
    UnknownExchange(String),
}

/// Errors that can occur during symbol validation.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ValidationError {
    #[error("Invalid base token: {0}")]
    InvalidBase(String),
    #[error("Invalid quote token: {0}")]
    InvalidQuote(String),
    #[error("Invalid symbol format: {0}")]
    InvalidFormat(String),
}

// ---------------------------------------------------------------------------
// Known token lists
// ---------------------------------------------------------------------------

const KNOWN_BASE_TOKENS: &[&str] = &[
    "BTC", "ETH", "SOL", "BNB", "XRP", "ADA", "DOT", "MATIC", "LINK", "LTC", "BCH", "XLM", "TRX",
    "EOS", "XMR", "ETC",
];

const KNOWN_QUOTE_TOKENS: &[&str] = &["USDT", "USDC", "BUSD", "BTC", "ETH", "BNB", "EUR"];

fn known_base_tokens_set() -> HashSet<&'static str> {
    KNOWN_BASE_TOKENS.iter().copied().collect()
}

fn known_quote_tokens_set() -> HashSet<&'static str> {
    KNOWN_QUOTE_TOKENS.iter().copied().collect()
}

// ---------------------------------------------------------------------------
// Exchange format
// ---------------------------------------------------------------------------

/// Exchange-specific trading pair formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeFormat {
    /// Binance format: `BTCUSDT` (no separator)
    Binance,
    /// OKX format: `BTC-USDT`
    OKX,
    /// Bybit format: `BTCUSDT` or `BTC/USDT`
    Bybit,
    /// Custom format with optional separator.
    Custom { separator: Option<char> },
}

impl ExchangeFormat {
    /// Parse a raw symbol string into (base, quote) according to this format.
    pub fn parse(&self, raw: &str) -> Option<(String, String)> {
        match self {
            ExchangeFormat::Binance => {
                // No separator — try to split at the last 4 chars (common quote length)
                // but better: try known quote tokens
                parse_unseparated(raw)
            }
            ExchangeFormat::OKX => {
                let parts: Vec<_> = raw.split('-').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_ascii_uppercase(), parts[1].to_ascii_uppercase()))
                } else {
                    None
                }
            }
            ExchangeFormat::Bybit => {
                if raw.contains('/') {
                    let parts: Vec<_> = raw.split('/').collect();
                    if parts.len() == 2 {
                        Some((parts[0].to_ascii_uppercase(), parts[1].to_ascii_uppercase()))
                    } else {
                        None
                    }
                } else {
                    parse_unseparated(raw)
                }
            }
            ExchangeFormat::Custom { separator } => match separator {
                Some(sep) => {
                    let parts: Vec<_> = raw.split(*sep).collect();
                    if parts.len() == 2 {
                        Some((parts[0].to_ascii_uppercase(), parts[1].to_ascii_uppercase()))
                    } else {
                        None
                    }
                }
                None => parse_unseparated(raw),
            },
        }
    }

    /// Format a (base, quote) pair into the exchange-specific format.
    pub fn format(&self, base: &str, quote: &str) -> String {
        let base = base.to_ascii_uppercase();
        let quote = quote.to_ascii_uppercase();
        match self {
            ExchangeFormat::Binance => format!("{}{}", base, quote),
            ExchangeFormat::OKX => format!("{}-{}", base, quote),
            ExchangeFormat::Bybit => format!("{}{}", base, quote),
            ExchangeFormat::Custom { separator } => match separator {
                Some(sep) => format!("{}{}{}", base, sep, quote),
                None => format!("{}{}", base, quote),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Parse an unseparated string like `BTCUSDT` into (base, quote).
/// Uses the known quote token list to find the longest matching suffix.
fn parse_unseparated(raw: &str) -> Option<(String, String)> {
    let raw_upper = raw.to_ascii_uppercase();

    // Try each known quote token as a suffix, preferring longest match
    let mut matches: Vec<(usize, &str)> = Vec::new();
    for &quote in KNOWN_QUOTE_TOKENS {
        if raw_upper.ends_with(quote) {
            matches.push((quote.len(), quote));
        }
    }

    // Sort by length descending to prefer longest match
    matches.sort_by(|a, b| b.0.cmp(&a.0));

    for (quote_len, quote) in matches {
        let base = &raw_upper[..raw_upper.len() - quote_len];
        if !base.is_empty() {
            return Some((base.to_string(), quote.to_string()));
        }
    }

    None
}

fn exchange_format_from_name(name: &str) -> Result<ExchangeFormat, NormalizationError> {
    match name.to_ascii_lowercase().as_str() {
        "binance" => Ok(ExchangeFormat::Binance),
        "okx" => Ok(ExchangeFormat::OKX),
        "bybit" => Ok(ExchangeFormat::Bybit),
        _ => Err(NormalizationError::UnknownExchange(name.to_string())),
    }
}

// ---------------------------------------------------------------------------
// SymbolNormalizer
// ---------------------------------------------------------------------------

/// Normalizes exchange-specific trading pair symbols to a standard format.
#[derive(Debug, Clone)]
pub struct SymbolNormalizer;

impl SymbolNormalizer {
    /// Normalize a raw exchange symbol to the standard `BASE/QUOTE` format.
    ///
    /// # Examples
    /// - `BTCUSDT` (binance) → `BTC/USDT`
    /// - `BTC-USDT` (okx) → `BTC/USDT`
    /// - `btcusdt` → `BTC/USDT`
    pub fn normalize(raw: &str, exchange: &str) -> Result<String, NormalizationError> {
        if raw.is_empty() {
            return Err(NormalizationError::UnknownFormat(
                "empty string".to_string(),
            ));
        }

        // If already standard format, just uppercase it
        if raw.contains('/') {
            let parts: Vec<_> = raw.split('/').collect();
            if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                let base = parts[0].to_ascii_uppercase();
                let quote = parts[1].to_ascii_uppercase();
                Self::validate(&base, &quote).map_err(|e| match e {
                    ValidationError::InvalidBase(b) => NormalizationError::InvalidBase(b),
                    ValidationError::InvalidQuote(q) => NormalizationError::InvalidQuote(q),
                    ValidationError::InvalidFormat(f) => NormalizationError::UnknownFormat(f),
                })?;
                return Ok(format!("{}/{}", base, quote));
            } else {
                return Err(NormalizationError::UnknownFormat(raw.to_string()));
            }
        }

        let format = exchange_format_from_name(exchange)?;
        let (base, quote) = format
            .parse(raw)
            .ok_or_else(|| NormalizationError::UnknownFormat(raw.to_string()))?;

        Self::validate(&base, &quote).map_err(|e| match e {
            ValidationError::InvalidBase(b) => NormalizationError::InvalidBase(b),
            ValidationError::InvalidQuote(q) => NormalizationError::InvalidQuote(q),
            ValidationError::InvalidFormat(f) => NormalizationError::UnknownFormat(f),
        })?;

        Ok(format!("{}/{}", base, quote))
    }

    /// Convert a standard `BASE/QUOTE` symbol back to exchange-specific format.
    ///
    /// # Examples
    /// - `BTC/USDT` → `BTCUSDT` (binance)
    /// - `BTC/USDT` → `BTC-USDT` (okx)
    pub fn denormalize(standard: &str, exchange: &str) -> Result<String, NormalizationError> {
        if standard.is_empty() {
            return Err(NormalizationError::UnknownFormat(
                "empty string".to_string(),
            ));
        }

        let parts: Vec<_> = standard.split('/').collect();
        if parts.len() != 2 {
            return Err(NormalizationError::UnknownFormat(standard.to_string()));
        }

        let base = parts[0];
        let quote = parts[1];

        Self::validate(base, quote).map_err(|e| match e {
            ValidationError::InvalidBase(b) => NormalizationError::InvalidBase(b),
            ValidationError::InvalidQuote(q) => NormalizationError::InvalidQuote(q),
            ValidationError::InvalidFormat(f) => NormalizationError::UnknownFormat(f),
        })?;

        let format = exchange_format_from_name(exchange)?;
        Ok(format.format(base, quote))
    }

    /// Validate that base and quote tokens are known.
    pub fn validate(base: &str, quote: &str) -> Result<(), ValidationError> {
        let base_upper = base.to_ascii_uppercase();
        let quote_upper = quote.to_ascii_uppercase();

        let base_set = known_base_tokens_set();
        let quote_set = known_quote_tokens_set();

        if !base_set.contains(base_upper.as_str()) {
            return Err(ValidationError::InvalidBase(base_upper));
        }

        if !quote_set.contains(quote_upper.as_str()) {
            return Err(ValidationError::InvalidQuote(quote_upper));
        }

        Ok(())
    }

    /// Validate a complete standard format symbol `BASE/QUOTE`.
    pub fn validate_full(symbol: &str) -> Result<(), ValidationError> {
        if symbol.is_empty() {
            return Err(ValidationError::InvalidFormat("empty string".to_string()));
        }

        let parts: Vec<_> = symbol.split('/').collect();
        if parts.len() != 2 {
            return Err(ValidationError::InvalidFormat(symbol.to_string()));
        }

        if parts[0].is_empty() || parts[1].is_empty() {
            return Err(ValidationError::InvalidFormat(symbol.to_string()));
        }

        Self::validate(parts[0], parts[1])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Normalization tests
    // ========================================================================

    #[test]
    fn normalize_binance_format() {
        assert_eq!(
            SymbolNormalizer::normalize("BTCUSDT", "binance").unwrap(),
            "BTC/USDT"
        );
    }

    #[test]
    fn normalize_okx_format() {
        assert_eq!(
            SymbolNormalizer::normalize("BTC-USDT", "okx").unwrap(),
            "BTC/USDT"
        );
    }

    #[test]
    fn normalize_lowercase_input() {
        assert_eq!(
            SymbolNormalizer::normalize("btcusdt", "binance").unwrap(),
            "BTC/USDT"
        );
    }

    #[test]
    fn normalize_lowercase_with_dash() {
        assert_eq!(
            SymbolNormalizer::normalize("btc-usdt", "okx").unwrap(),
            "BTC/USDT"
        );
    }

    #[test]
    fn normalize_already_standard_format() {
        assert_eq!(
            SymbolNormalizer::normalize("BTC/USDT", "binance").unwrap(),
            "BTC/USDT"
        );
    }

    #[test]
    fn normalize_lowercase_standard_format() {
        assert_eq!(
            SymbolNormalizer::normalize("btc/usdt", "binance").unwrap(),
            "BTC/USDT"
        );
    }

    #[test]
    fn normalize_eth_btc_pair() {
        assert_eq!(
            SymbolNormalizer::normalize("ETHBTC", "binance").unwrap(),
            "ETH/BTC"
        );
    }

    #[test]
    fn normalize_usdc_quote() {
        assert_eq!(
            SymbolNormalizer::normalize("BTCUSDC", "binance").unwrap(),
            "BTC/USDC"
        );
    }

    #[test]
    fn normalize_bybit_format() {
        assert_eq!(
            SymbolNormalizer::normalize("BTCUSDT", "bybit").unwrap(),
            "BTC/USDT"
        );
    }

    #[test]
    fn normalize_bybit_slash_format() {
        assert_eq!(
            SymbolNormalizer::normalize("BTC/USDT", "bybit").unwrap(),
            "BTC/USDT"
        );
    }

    // ========================================================================
    // Error cases - normalization
    // ========================================================================

    #[test]
    fn normalize_unknown_base_token() {
        let result = SymbolNormalizer::normalize("UNKNOWNUSDT", "binance");
        assert!(matches!(result, Err(NormalizationError::InvalidBase(ref s)) if s == "UNKNOWN"));
    }

    #[test]
    fn normalize_unknown_quote_token() {
        let result = SymbolNormalizer::normalize("BTCXXX", "binance");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(_))));
    }

    #[test]
    fn normalize_empty_string() {
        let result = SymbolNormalizer::normalize("", "binance");
        assert!(
            matches!(result, Err(NormalizationError::UnknownFormat(ref s)) if s == "empty string")
        );
    }

    #[test]
    fn normalize_unknown_exchange() {
        let result = SymbolNormalizer::normalize("BTCUSDT", "unknown_exchange");
        assert!(
            matches!(result, Err(NormalizationError::UnknownExchange(ref s)) if s == "unknown_exchange")
        );
    }

    #[test]
    fn normalize_garbage_input() {
        let result = SymbolNormalizer::normalize("!!!@@@###", "binance");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(_))));
    }

    #[test]
    fn normalize_only_separator() {
        let result = SymbolNormalizer::normalize("/", "binance");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(_))));
    }

    #[test]
    fn normalize_missing_quote() {
        let result = SymbolNormalizer::normalize("BTC/", "binance");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(ref s)) if s == "BTC/"));
    }

    #[test]
    fn normalize_missing_base() {
        let result = SymbolNormalizer::normalize("/USDT", "binance");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(ref s)) if s == "/USDT"));
    }

    // ========================================================================
    // Ambiguous parsing tests
    // ========================================================================

    #[test]
    fn normalize_prefers_longer_quote_match() {
        // USDC is longer than USDT, so BTCUSDC should parse as BTC/USDC not BTCU/SDT
        assert_eq!(
            SymbolNormalizer::normalize("BTCUSDC", "binance").unwrap(),
            "BTC/USDC"
        );
    }

    #[test]
    fn normalize_btc_usdc_not_btcu_sdt() {
        // This tests that BTCUSDC is not parsed as BTCU/SDT (which would be invalid anyway)
        assert_eq!(
            SymbolNormalizer::normalize("BTCUSDC", "binance").unwrap(),
            "BTC/USDC"
        );
    }

    // ========================================================================
    // Denormalization tests
    // ========================================================================

    #[test]
    fn denormalize_to_binance_format() {
        assert_eq!(
            SymbolNormalizer::denormalize("BTC/USDT", "binance").unwrap(),
            "BTCUSDT"
        );
    }

    #[test]
    fn denormalize_to_okx_format() {
        assert_eq!(
            SymbolNormalizer::denormalize("BTC/USDT", "okx").unwrap(),
            "BTC-USDT"
        );
    }

    #[test]
    fn denormalize_eth_btc_to_binance() {
        assert_eq!(
            SymbolNormalizer::denormalize("ETH/BTC", "binance").unwrap(),
            "ETHBTC"
        );
    }

    #[test]
    fn denormalize_unknown_exchange() {
        let result = SymbolNormalizer::denormalize("BTC/USDT", "unknown");
        assert!(
            matches!(result, Err(NormalizationError::UnknownExchange(ref s)) if s == "unknown")
        );
    }

    #[test]
    fn denormalize_invalid_format() {
        let result = SymbolNormalizer::denormalize("BTCUSDT", "binance");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(_))));
    }

    #[test]
    fn denormalize_empty_string() {
        let result = SymbolNormalizer::denormalize("", "binance");
        assert!(
            matches!(result, Err(NormalizationError::UnknownFormat(ref s)) if s == "empty string")
        );
    }

    // ========================================================================
    // Validation tests
    // ========================================================================

    #[test]
    fn validate_valid_tokens() {
        assert!(SymbolNormalizer::validate("BTC", "USDT").is_ok());
    }

    #[test]
    fn validate_btc_as_quote() {
        assert!(SymbolNormalizer::validate("ETH", "BTC").is_ok());
    }

    #[test]
    fn validate_invalid_base() {
        let result = SymbolNormalizer::validate("UNKNOWN", "USDT");
        assert!(matches!(result, Err(ValidationError::InvalidBase(ref s)) if s == "UNKNOWN"));
    }

    #[test]
    fn validate_invalid_quote() {
        let result = SymbolNormalizer::validate("BTC", "UNKNOWN");
        assert!(matches!(result, Err(ValidationError::InvalidQuote(ref s)) if s == "UNKNOWN"));
    }

    #[test]
    fn validate_lowercase_tokens() {
        assert!(SymbolNormalizer::validate("btc", "usdt").is_ok());
    }

    #[test]
    fn validate_full_valid_symbol() {
        assert!(SymbolNormalizer::validate_full("BTC/USDT").is_ok());
    }

    #[test]
    fn validate_full_invalid_format_no_slash() {
        let result = SymbolNormalizer::validate_full("BTCUSDT");
        assert!(matches!(result, Err(ValidationError::InvalidFormat(ref s)) if s == "BTCUSDT"));
    }

    #[test]
    fn validate_full_empty_string() {
        let result = SymbolNormalizer::validate_full("");
        assert!(
            matches!(result, Err(ValidationError::InvalidFormat(ref s)) if s == "empty string")
        );
    }

    #[test]
    fn validate_full_missing_base() {
        let result = SymbolNormalizer::validate_full("/USDT");
        assert!(matches!(result, Err(ValidationError::InvalidFormat(ref s)) if s == "/USDT"));
    }

    #[test]
    fn validate_full_missing_quote() {
        let result = SymbolNormalizer::validate_full("BTC/");
        assert!(matches!(result, Err(ValidationError::InvalidFormat(ref s)) if s == "BTC/"));
    }

    #[test]
    fn validate_full_multiple_slashes() {
        let result = SymbolNormalizer::validate_full("BTC/USDT/EXTRA");
        assert!(
            matches!(result, Err(ValidationError::InvalidFormat(ref s)) if s == "BTC/USDT/EXTRA")
        );
    }

    // ========================================================================
    // ExchangeFormat tests
    // ========================================================================

    #[test]
    fn exchange_format_binance_parse() {
        let fmt = ExchangeFormat::Binance;
        assert_eq!(
            fmt.parse("BTCUSDT"),
            Some(("BTC".to_string(), "USDT".to_string()))
        );
    }

    #[test]
    fn exchange_format_okx_parse() {
        let fmt = ExchangeFormat::OKX;
        assert_eq!(
            fmt.parse("BTC-USDT"),
            Some(("BTC".to_string(), "USDT".to_string()))
        );
    }

    #[test]
    fn exchange_format_okx_parse_lowercase() {
        let fmt = ExchangeFormat::OKX;
        assert_eq!(
            fmt.parse("btc-usdt"),
            Some(("BTC".to_string(), "USDT".to_string()))
        );
    }

    #[test]
    fn exchange_format_custom_with_separator() {
        let fmt = ExchangeFormat::Custom {
            separator: Some('_'),
        };
        assert_eq!(
            fmt.parse("BTC_USDT"),
            Some(("BTC".to_string(), "USDT".to_string()))
        );
    }

    #[test]
    fn exchange_format_custom_without_separator() {
        let fmt = ExchangeFormat::Custom { separator: None };
        assert_eq!(
            fmt.parse("BTCUSDT"),
            Some(("BTC".to_string(), "USDT".to_string()))
        );
    }

    #[test]
    fn exchange_format_binance_format() {
        let fmt = ExchangeFormat::Binance;
        assert_eq!(fmt.format("BTC", "USDT"), "BTCUSDT");
    }

    #[test]
    fn exchange_format_okx_format() {
        let fmt = ExchangeFormat::OKX;
        assert_eq!(fmt.format("BTC", "USDT"), "BTC-USDT");
    }

    #[test]
    fn exchange_format_bybit_format() {
        let fmt = ExchangeFormat::Bybit;
        assert_eq!(fmt.format("BTC", "USDT"), "BTCUSDT");
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn normalize_whitespace_only() {
        let result = SymbolNormalizer::normalize("   ", "binance");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(_))));
    }

    #[test]
    fn normalize_with_whitespace_in_middle() {
        let result = SymbolNormalizer::normalize("BTC USDT", "binance");
        assert!(matches!(result, Err(NormalizationError::InvalidBase(ref s)) if s == "BTC "));
    }

    #[test]
    fn normalize_multiple_dashes() {
        let result = SymbolNormalizer::normalize("BTC-USDT-EXTRA", "okx");
        assert!(matches!(result, Err(NormalizationError::UnknownFormat(_))));
    }

    #[test]
    fn normalize_single_char_base() {
        // X is not a known base token
        let result = SymbolNormalizer::normalize("XUSDT", "binance");
        assert!(matches!(result, Err(NormalizationError::InvalidBase(ref s)) if s == "X"));
    }

    #[test]
    fn normalize_known_tokens_case_insensitive() {
        assert_eq!(
            SymbolNormalizer::normalize("bTcUsDt", "binance").unwrap(),
            "BTC/USDT"
        );
    }
}
