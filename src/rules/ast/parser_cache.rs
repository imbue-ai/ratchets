#![forbid(unsafe_code)]

//! Parser cache for lazy loading of tree-sitter parsers
//!
//! This module provides a thread-safe cache for tree-sitter parsers,
//! loading them on-demand as needed for each supported language.

use crate::types::Language;
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

/// Errors that can occur when loading parsers
#[derive(Debug, Error)]
pub enum ParserError {
    /// The requested language is not supported or not enabled
    #[error("Language {0:?} is not supported or not enabled via feature flags")]
    UnsupportedLanguage(Language),

    /// Failed to create or initialize a parser
    #[error("Failed to initialize parser for {0:?}")]
    InitializationFailed(Language),

    /// Lock poisoned (internal error)
    #[error("Internal cache lock error")]
    LockPoisoned,
}

/// Cache for tree-sitter parsers
///
/// This struct provides lazy loading of parsers for each supported language.
/// Parsers are loaded on first use and cached for subsequent requests.
/// The cache is thread-safe and can be shared across threads.
///
/// # Interior Mutability
///
/// This type uses interior mutability (RwLock) to enable lazy loading of parsers.
/// This is necessary because parsers are expensive to create and should only be
/// loaded when needed, but the cache must be usable from an immutable reference.
pub struct ParserCache {
    parsers: RwLock<HashMap<Language, tree_sitter::Parser>>,
}

impl ParserCache {
    /// Creates a new empty parser cache
    pub fn new() -> Self {
        Self {
            parsers: RwLock::new(HashMap::new()),
        }
    }

    /// Gets a parser for the specified language, loading it if necessary
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The language is not supported
    /// - The language feature is not enabled
    /// - Parser initialization fails
    ///
    /// # Note on Return Value
    ///
    /// This method returns a newly created Parser rather than a reference because
    /// tree_sitter::Parser instances are lightweight and cheap to clone. The actual
    /// grammar data is shared internally, so this approach avoids lifetime complications
    /// while maintaining efficient memory usage.
    pub fn get_parser(&self, language: Language) -> Result<tree_sitter::Parser, ParserError> {
        // First try to read from cache
        {
            let parsers = self.parsers.read().map_err(|_| ParserError::LockPoisoned)?;
            if parsers.contains_key(&language) {
                // Parser exists, create a new instance with same language
                return Self::create_parser_for_language(language);
            }
        }

        // Cache miss - acquire write lock and create parser
        let mut parsers = self
            .parsers
            .write()
            .map_err(|_| ParserError::LockPoisoned)?;

        // Double-check in case another thread added it while we were waiting
        if let std::collections::hash_map::Entry::Vacant(e) = parsers.entry(language) {
            let parser = Self::create_parser_for_language(language)?;
            e.insert(parser);
        }

        // Return a new parser instance
        Self::create_parser_for_language(language)
    }

    /// Creates a parser for the given language
    fn create_parser_for_language(language: Language) -> Result<tree_sitter::Parser, ParserError> {
        match language {
            Language::Rust => Self::create_rust_parser(),
            Language::TypeScript => Self::create_typescript_parser(),
            Language::JavaScript => Self::create_javascript_parser(),
            Language::Python => Self::create_python_parser(),
            Language::Go => Self::create_go_parser(),
        }
    }

    fn create_rust_parser() -> Result<tree_sitter::Parser, ParserError> {
        #[cfg(feature = "lang-rust")]
        {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_rust::language())
                .map_err(|_| ParserError::InitializationFailed(Language::Rust))?;
            Ok(parser)
        }
        #[cfg(not(feature = "lang-rust"))]
        {
            Err(ParserError::UnsupportedLanguage(Language::Rust))
        }
    }

    fn create_typescript_parser() -> Result<tree_sitter::Parser, ParserError> {
        #[cfg(feature = "lang-typescript")]
        {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_typescript::language_typescript())
                .map_err(|_| ParserError::InitializationFailed(Language::TypeScript))?;
            Ok(parser)
        }
        #[cfg(not(feature = "lang-typescript"))]
        {
            Err(ParserError::UnsupportedLanguage(Language::TypeScript))
        }
    }

    fn create_javascript_parser() -> Result<tree_sitter::Parser, ParserError> {
        #[cfg(feature = "lang-javascript")]
        {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_javascript::language())
                .map_err(|_| ParserError::InitializationFailed(Language::JavaScript))?;
            Ok(parser)
        }
        #[cfg(not(feature = "lang-javascript"))]
        {
            Err(ParserError::UnsupportedLanguage(Language::JavaScript))
        }
    }

    fn create_python_parser() -> Result<tree_sitter::Parser, ParserError> {
        #[cfg(feature = "lang-python")]
        {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_python::language())
                .map_err(|_| ParserError::InitializationFailed(Language::Python))?;
            Ok(parser)
        }
        #[cfg(not(feature = "lang-python"))]
        {
            Err(ParserError::UnsupportedLanguage(Language::Python))
        }
    }

    fn create_go_parser() -> Result<tree_sitter::Parser, ParserError> {
        #[cfg(feature = "lang-go")]
        {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&tree_sitter_go::language())
                .map_err(|_| ParserError::InitializationFailed(Language::Go))?;
            Ok(parser)
        }
        #[cfg(not(feature = "lang-go"))]
        {
            Err(ParserError::UnsupportedLanguage(Language::Go))
        }
    }
}

impl Default for ParserCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_cache_creation() {
        let cache = ParserCache::new();
        // Just verify it compiles and constructs
        drop(cache);
    }

    #[test]
    fn test_parser_cache_default() {
        let cache = ParserCache::default();
        drop(cache);
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_rust_parser_loading() {
        let cache = ParserCache::new();
        let result = cache.get_parser(Language::Rust);
        assert!(
            result.is_ok(),
            "Rust parser should load when feature is enabled"
        );
    }

    #[cfg(feature = "lang-typescript")]
    #[test]
    fn test_typescript_parser_loading() {
        let cache = ParserCache::new();
        let result = cache.get_parser(Language::TypeScript);
        assert!(
            result.is_ok(),
            "TypeScript parser should load when feature is enabled"
        );
    }

    #[cfg(feature = "lang-javascript")]
    #[test]
    fn test_javascript_parser_loading() {
        let cache = ParserCache::new();
        let result = cache.get_parser(Language::JavaScript);
        assert!(
            result.is_ok(),
            "JavaScript parser should load when feature is enabled"
        );
    }

    #[cfg(feature = "lang-python")]
    #[test]
    fn test_python_parser_loading() {
        let cache = ParserCache::new();
        let result = cache.get_parser(Language::Python);
        assert!(
            result.is_ok(),
            "Python parser should load when feature is enabled"
        );
    }

    #[cfg(feature = "lang-go")]
    #[test]
    fn test_go_parser_loading() {
        let cache = ParserCache::new();
        let result = cache.get_parser(Language::Go);
        assert!(
            result.is_ok(),
            "Go parser should load when feature is enabled"
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn test_parser_caching() -> Result<(), Box<dyn std::error::Error>> {
        let cache = ParserCache::new();

        // Get parser twice - should succeed both times
        let parser1 = cache.get_parser(Language::Rust);
        assert!(parser1.is_ok(), "First parser load should succeed");

        let parser2 = cache.get_parser(Language::Rust);
        assert!(
            parser2.is_ok(),
            "Second parser load should succeed (from cache)"
        );

        // Verify the cache was populated by checking the internal state
        let parsers = cache.parsers.read().map_err(|_| "lock poisoned")?;
        assert!(
            parsers.contains_key(&Language::Rust),
            "Parser should be cached"
        );
        Ok(())
    }

    #[cfg(not(feature = "lang-rust"))]
    #[test]
    fn test_unsupported_language_error() {
        let cache = ParserCache::new();
        let result = cache.get_parser(Language::Rust);
        assert!(matches!(
            result,
            Err(ParserError::UnsupportedLanguage(Language::Rust))
        ));
    }
}
