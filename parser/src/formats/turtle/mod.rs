//! Turtle Parser
//!
//! Parser for the Turtle (Terse RDF Triple Language) format.
//! Supports prefix declarations, base URI resolution, collections,
//! and nested blank node syntax.

mod lexer;
mod parser;

pub use lexer::{Lexer, Token, TokenKind};
pub use parser::TurtleParser;
