//! Turtle Lexer
//!
//! Tokenizes Turtle input into a stream of tokens.

use std::iter::Peekable;
use std::str::Chars;

use crate::ParserError;

/// Token kinds for Turtle syntax
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// IRI reference in angle brackets: <http://example.org>
    IriRef(String),
    /// Prefixed name: prefix:localName
    PrefixedName { prefix: String, local: String },
    /// Blank node label: _:label
    BlankNodeLabel(String),
    /// String literal (without quotes)
    StringLiteral(String),
    /// Language tag (without @): en, en-US
    LangTag(String),
    /// Integer literal
    Integer(i64),
    /// Decimal literal
    Decimal(f64),
    /// Double literal (scientific notation)
    Double(f64),
    /// Boolean literal
    Boolean(bool),
    /// @prefix keyword
    PrefixKeyword,
    /// @base keyword
    BaseKeyword,
    /// PREFIX (SPARQL-style)
    SparqlPrefix,
    /// BASE (SPARQL-style)
    SparqlBase,
    /// 'a' keyword (shorthand for rdf:type)
    A,
    /// Dot: .
    Dot,
    /// Semicolon: ;
    Semicolon,
    /// Comma: ,
    Comma,
    /// Open bracket: [
    OpenBracket,
    /// Close bracket: ]
    CloseBracket,
    /// Open paren: (
    OpenParen,
    /// Close paren: )
    CloseParen,
    /// Datatype marker: ^^
    DoubleCaret,
    /// End of input
    Eof,
}

/// A token with its position in the input
#[derive(Debug, Clone)]
pub struct Token {
    /// The token kind
    pub kind: TokenKind,
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
}

impl Token {
    const fn new(kind: TokenKind, line: usize, column: usize) -> Self {
        Self { kind, line, column }
    }
}

/// Turtle lexer
pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
    line: usize,
    column: usize,
    /// Track if we've consumed all input
    eof: bool,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.chars().peekable(),
            line: 1,
            column: 1,
            eof: false,
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Result<Token, ParserError> {
        self.skip_whitespace_and_comments();

        let line = self.line;
        let column = self.column;

        let Some(ch) = self.peek() else {
            self.eof = true;
            return Ok(Token::new(TokenKind::Eof, line, column));
        };

        let kind = match ch {
            '<' => self.read_iri_ref()?,
            '"' | '\'' => self.read_string_literal()?,
            '@' => self.read_at_keyword()?,
            '_' => self.read_blank_node_label()?,
            '.' => {
                self.advance();
                TokenKind::Dot
            }
            ';' => {
                self.advance();
                TokenKind::Semicolon
            }
            ',' => {
                self.advance();
                TokenKind::Comma
            }
            '[' => {
                self.advance();
                TokenKind::OpenBracket
            }
            ']' => {
                self.advance();
                TokenKind::CloseBracket
            }
            '(' => {
                self.advance();
                TokenKind::OpenParen
            }
            ')' => {
                self.advance();
                TokenKind::CloseParen
            }
            '^' => self.read_double_caret()?,
            '+' | '-' | '0'..='9' => self.read_number()?,
            'a' => {
                // Check if it's 'a' keyword or a prefixed name
                self.advance();
                let next = self.peek();
                if self.is_pn_char_or_colon(next) {
                    self.read_prefixed_name_from('a')?
                } else {
                    TokenKind::A
                }
            }
            'P' | 'B' => {
                // Check for SPARQL-style PREFIX/BASE
                let word = self.read_word();
                match word.as_str() {
                    "PREFIX" => TokenKind::SparqlPrefix,
                    "BASE" => TokenKind::SparqlBase,
                    _ => self.read_prefixed_name_from_word(&word)?,
                }
            }
            'T' | 't' | 'F' | 'f' => {
                // Check for true/false
                let word = self.read_word();
                match word.to_lowercase().as_str() {
                    "true" => TokenKind::Boolean(true),
                    "false" => TokenKind::Boolean(false),
                    _ => self.read_prefixed_name_from_word(&word)?,
                }
            }
            c if self.is_pn_chars_base(c) => {
                let word = self.read_word();
                self.read_prefixed_name_from_word(&word)?
            }
            ':' => {
                // Empty prefix
                self.advance();
                let local = self.read_pn_local();
                TokenKind::PrefixedName {
                    prefix: String::new(),
                    local,
                }
            }
            _ => {
                return Err(ParserError::ParseError(format!(
                    "Unexpected character '{ch}' at line {line}, column {column}"
                )));
            }
        };

        Ok(Token::new(kind, line, column))
    }

    /// Tokenize all input into a vector of tokens
    pub fn tokenize(&mut self) -> Result<Vec<Token>, ParserError> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn peek(&mut self) -> Option<char> {
        self.input.peek().copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.next();
        if let Some(c) = ch {
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        ch
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                Some('#') => {
                    // Skip comment until end of line
                    while let Some(c) = self.advance() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn read_iri_ref(&mut self) -> Result<TokenKind, ParserError> {
        self.advance(); // consume '<'
        let mut iri = String::new();

        loop {
            match self.advance() {
                Some('>') => break,
                Some('\\') => {
                    // Handle escape sequences
                    if let Some(escaped) = self.advance() {
                        iri.push(self.unescape_char(escaped)?);
                    }
                }
                Some(c) if c == '<' || c.is_whitespace() && c != ' ' => {
                    return Err(ParserError::ParseError(format!(
                        "Invalid character in IRI: {c:?}"
                    )));
                }
                Some(c) => iri.push(c),
                None => {
                    return Err(ParserError::ParseError("Unterminated IRI reference".into()));
                }
            }
        }

        Ok(TokenKind::IriRef(iri))
    }

    fn read_string_literal(&mut self) -> Result<TokenKind, ParserError> {
        let quote = self.advance().unwrap(); // consume opening quote
        let mut value = String::new();

        // Check for long string (triple quotes)
        let is_long = if self.peek() == Some(quote) {
            self.advance();
            if self.peek() == Some(quote) {
                self.advance();
                true
            } else {
                // Empty string with two quotes - put back by not consuming
                return Ok(TokenKind::StringLiteral(value));
            }
        } else {
            false
        };

        loop {
            match self.advance() {
                Some(c) if c == quote => {
                    if is_long {
                        // Need two more quotes
                        if self.peek() == Some(quote) {
                            self.advance();
                            if self.peek() == Some(quote) {
                                self.advance();
                                break;
                            }
                            value.push(quote);
                            value.push(quote);
                        } else {
                            value.push(quote);
                        }
                    } else {
                        break;
                    }
                }
                Some('\\') => {
                    if let Some(escaped) = self.advance() {
                        value.push(self.unescape_char(escaped)?);
                    }
                }
                Some('\n' | '\r') if !is_long => {
                    return Err(ParserError::ParseError(
                        "Newline in short string literal".into(),
                    ));
                }
                Some(c) => value.push(c),
                None => {
                    return Err(ParserError::ParseError(
                        "Unterminated string literal".into(),
                    ));
                }
            }
        }

        Ok(TokenKind::StringLiteral(value))
    }

    const fn unescape_char(&self, ch: char) -> Result<char, ParserError> {
        match ch {
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            '\\' => Ok('\\'),
            '"' => Ok('"'),
            '\'' => Ok('\''),
            'u' | 'U' => {
                // Unicode escape - for simplicity, return the escaped form
                // A full implementation would read the hex digits
                Ok(ch)
            }
            _ => Ok(ch),
        }
    }

    fn read_at_keyword(&mut self) -> Result<TokenKind, ParserError> {
        self.advance(); // consume '@'

        let word = self.read_word();

        match word.to_lowercase().as_str() {
            "prefix" => Ok(TokenKind::PrefixKeyword),
            "base" => Ok(TokenKind::BaseKeyword),
            _ => {
                // It's a language tag
                Ok(TokenKind::LangTag(word))
            }
        }
    }

    fn read_blank_node_label(&mut self) -> Result<TokenKind, ParserError> {
        self.advance(); // consume '_'

        if self.peek() != Some(':') {
            return Err(ParserError::ParseError(
                "Expected ':' after '_' in blank node".into(),
            ));
        }
        self.advance(); // consume ':'

        let mut label = String::new();

        // First char must be PN_CHARS_U or digit
        if let Some(c) = self.peek() {
            if self.is_pn_chars_u(c) || c.is_ascii_digit() {
                label.push(self.advance().unwrap());
            } else {
                return Err(ParserError::ParseError(format!(
                    "Invalid first character in blank node label: {c:?}"
                )));
            }
        }

        // Subsequent chars
        while let Some(c) = self.peek() {
            if self.is_pn_chars(c) || c == '.' {
                // Dot allowed inside but not at end
                if c == '.' {
                    // Look ahead to ensure not at end
                    label.push(self.advance().unwrap());
                    if self.peek().map_or(true, |c| !self.is_pn_chars(c)) {
                        // Dot was at end, remove it
                        label.pop();
                        break;
                    }
                } else {
                    label.push(self.advance().unwrap());
                }
            } else {
                break;
            }
        }

        Ok(TokenKind::BlankNodeLabel(label))
    }

    fn read_double_caret(&mut self) -> Result<TokenKind, ParserError> {
        self.advance(); // consume first '^'
        if self.peek() != Some('^') {
            return Err(ParserError::ParseError("Expected '^^'".into()));
        }
        self.advance(); // consume second '^'
        Ok(TokenKind::DoubleCaret)
    }

    fn read_number(&mut self) -> Result<TokenKind, ParserError> {
        let mut num_str = String::new();
        let mut has_dot = false;
        let mut has_exponent = false;

        // Handle sign
        if self.peek() == Some('+') || self.peek() == Some('-') {
            num_str.push(self.advance().unwrap());
        }

        // Read digits before decimal point
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                num_str.push(self.advance().unwrap());
            } else {
                break;
            }
        }

        // Check for decimal point
        if self.peek() == Some('.') {
            // Look ahead to ensure it's a decimal, not a statement terminator
            let saved_col = self.column;
            self.advance();
            if let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    has_dot = true;
                    num_str.push('.');
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            num_str.push(self.advance().unwrap());
                        } else {
                            break;
                        }
                    }
                } else {
                    // It was a statement terminator, need to unread
                    // Since we can't unread, we'll treat '.' as next token
                    // This is a limitation - we'll return the number and the dot
                    // will be picked up next
                    self.column = saved_col;
                    // Actually, we already consumed '.', so return what we have
                    // and let the parser handle it
                    if num_str.is_empty() || num_str == "+" || num_str == "-" {
                        return Ok(TokenKind::Dot);
                    }
                    let val: i64 = num_str.parse().map_err(|_| {
                        ParserError::ParseError(format!("Invalid integer: {num_str}"))
                    })?;
                    return Ok(TokenKind::Integer(val));
                }
            }
        }

        // Check for exponent
        if self.peek() == Some('e') || self.peek() == Some('E') {
            has_exponent = true;
            num_str.push(self.advance().unwrap());

            // Optional sign
            if self.peek() == Some('+') || self.peek() == Some('-') {
                num_str.push(self.advance().unwrap());
            }

            // Exponent digits
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    num_str.push(self.advance().unwrap());
                } else {
                    break;
                }
            }
        }

        if has_exponent {
            let val: f64 = num_str
                .parse()
                .map_err(|_| ParserError::ParseError(format!("Invalid double: {num_str}")))?;
            Ok(TokenKind::Double(val))
        } else if has_dot {
            let val: f64 = num_str
                .parse()
                .map_err(|_| ParserError::ParseError(format!("Invalid decimal: {num_str}")))?;
            Ok(TokenKind::Decimal(val))
        } else {
            let val: i64 = num_str
                .parse()
                .map_err(|_| ParserError::ParseError(format!("Invalid integer: {num_str}")))?;
            Ok(TokenKind::Integer(val))
        }
    }

    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                word.push(self.advance().unwrap());
            } else {
                break;
            }
        }
        word
    }

    fn read_prefixed_name_from(&mut self, first: char) -> Result<TokenKind, ParserError> {
        let mut prefix = String::from(first);

        // Read rest of prefix
        while let Some(c) = self.peek() {
            if c == ':' {
                self.advance();
                let local = self.read_pn_local();
                return Ok(TokenKind::PrefixedName { prefix, local });
            } else if self.is_pn_chars(c) {
                prefix.push(self.advance().unwrap());
            } else {
                break;
            }
        }

        Err(ParserError::ParseError(format!(
            "Expected ':' in prefixed name after '{prefix}'"
        )))
    }

    fn read_prefixed_name_from_word(&mut self, word: &str) -> Result<TokenKind, ParserError> {
        if self.peek() == Some(':') {
            self.advance();
            let local = self.read_pn_local();
            Ok(TokenKind::PrefixedName {
                prefix: word.to_string(),
                local,
            })
        } else {
            Err(ParserError::ParseError(format!(
                "Expected ':' in prefixed name after '{word}'"
            )))
        }
    }

    fn read_pn_local(&mut self) -> String {
        let mut local = String::new();

        // First char
        if let Some(c) = self.peek() {
            if self.is_pn_chars_u(c) || c == ':' || c.is_ascii_digit() || c == '%' || c == '\\' {
                local.push(self.advance().unwrap());
            } else {
                return local;
            }
        }

        // Subsequent chars
        while let Some(c) = self.peek() {
            if self.is_pn_chars(c) || c == '.' || c == ':' || c == '%' || c == '\\' {
                if c == '.' {
                    // Dot allowed inside but not at end
                    local.push(self.advance().unwrap());
                    if self
                        .peek()
                        .map_or(true, |c| !self.is_pn_chars(c) && c != ':')
                    {
                        local.pop();
                        break;
                    }
                } else {
                    local.push(self.advance().unwrap());
                }
            } else {
                break;
            }
        }

        local
    }

    fn is_pn_chars_base(&self, c: char) -> bool {
        c.is_alphabetic()
    }

    fn is_pn_chars_u(&self, c: char) -> bool {
        self.is_pn_chars_base(c) || c == '_'
    }

    fn is_pn_chars(&self, c: char) -> bool {
        self.is_pn_chars_u(c) || c == '-' || c.is_ascii_digit() || c == '\u{00B7}'
    }

    fn is_pn_char_or_colon(&self, c: Option<char>) -> bool {
        c.is_some_and(|c| self.is_pn_chars(c) || c == ':')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iri_ref() {
        let mut lexer = Lexer::new("<http://example.org/resource>");
        let token = lexer.next_token().unwrap();
        assert_eq!(
            token.kind,
            TokenKind::IriRef("http://example.org/resource".into())
        );
    }

    #[test]
    fn test_prefixed_name() {
        let mut lexer = Lexer::new("rdf:type");
        let token = lexer.next_token().unwrap();
        assert_eq!(
            token.kind,
            TokenKind::PrefixedName {
                prefix: "rdf".into(),
                local: "type".into()
            }
        );
    }

    #[test]
    fn test_blank_node() {
        let mut lexer = Lexer::new("_:node1");
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, TokenKind::BlankNodeLabel("node1".into()));
    }

    #[test]
    fn test_string_literal() {
        let mut lexer = Lexer::new("\"hello world\"");
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, TokenKind::StringLiteral("hello world".into()));
    }

    #[test]
    fn test_lang_tag() {
        let mut lexer = Lexer::new("\"hello\"@en");
        let t1 = lexer.next_token().unwrap();
        let t2 = lexer.next_token().unwrap();
        assert_eq!(t1.kind, TokenKind::StringLiteral("hello".into()));
        assert_eq!(t2.kind, TokenKind::LangTag("en".into()));
    }

    #[test]
    fn test_prefix_keyword() {
        let mut lexer = Lexer::new("@prefix ex: <http://example.org/> .");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::PrefixKeyword);
        assert_eq!(
            tokens[1].kind,
            TokenKind::PrefixedName {
                prefix: "ex".into(),
                local: "".into()
            }
        );
        assert_eq!(
            tokens[2].kind,
            TokenKind::IriRef("http://example.org/".into())
        );
        assert_eq!(tokens[3].kind, TokenKind::Dot);
    }

    #[test]
    fn test_numbers() {
        let mut lexer = Lexer::new("42 3.25 1.5e10");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Integer(42));
        assert_eq!(tokens[1].kind, TokenKind::Decimal(3.25));
        assert_eq!(tokens[2].kind, TokenKind::Double(1.5e10));
    }

    #[test]
    fn test_boolean() {
        let mut lexer = Lexer::new("true false");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Boolean(true));
        assert_eq!(tokens[1].kind, TokenKind::Boolean(false));
    }

    #[test]
    fn test_a_keyword() {
        let mut lexer = Lexer::new("a");
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, TokenKind::A);
    }

    #[test]
    fn test_punctuation() {
        let mut lexer = Lexer::new(". ; , [ ] ( ) ^^");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Dot);
        assert_eq!(tokens[1].kind, TokenKind::Semicolon);
        assert_eq!(tokens[2].kind, TokenKind::Comma);
        assert_eq!(tokens[3].kind, TokenKind::OpenBracket);
        assert_eq!(tokens[4].kind, TokenKind::CloseBracket);
        assert_eq!(tokens[5].kind, TokenKind::OpenParen);
        assert_eq!(tokens[6].kind, TokenKind::CloseParen);
        assert_eq!(tokens[7].kind, TokenKind::DoubleCaret);
    }

    #[test]
    fn test_comments() {
        let mut lexer = Lexer::new("# this is a comment\n<http://example.org>");
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, TokenKind::IriRef("http://example.org".into()));
    }

    #[test]
    fn test_typed_literal() {
        let mut lexer = Lexer::new("\"42\"^^xsd:integer");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral("42".into()));
        assert_eq!(tokens[1].kind, TokenKind::DoubleCaret);
        assert_eq!(
            tokens[2].kind,
            TokenKind::PrefixedName {
                prefix: "xsd".into(),
                local: "integer".into()
            }
        );
    }
}
