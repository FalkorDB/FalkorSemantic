//! Turtle Parser
//!
//! Parses Turtle syntax into RDF triples.

use std::collections::HashMap;

use super::lexer::{Lexer, Token, TokenKind};
use crate::rdf::{
    BlankNode, BlankNodeScope, Iri, Literal, NamespaceRegistry, Object, Predicate, Subject, Triple,
};
use crate::{ParserError, Result};

/// RDF type predicate IRI
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Maximum nesting depth for collections and blank nodes.
/// This prevents stack overflow from maliciously crafted deeply nested input.
const MAX_NESTING_DEPTH: usize = 128;
/// RDF first predicate IRI
const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
/// RDF rest predicate IRI
const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
/// RDF nil IRI
const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// Turtle parser
///
/// Parses Turtle documents into RDF triples with support for:
/// - Prefix declarations (@prefix, PREFIX)
/// - Base URI declarations (@base, BASE)
/// - Collections (RDF lists)
/// - Nested blank node syntax
pub struct TurtleParser {
    /// Namespace registry for prefix resolution
    namespaces: NamespaceRegistry,
    /// Base IRI for resolving relative IRIs
    base_iri: Option<Iri>,
    /// Blank node scope for generating unique blank node IDs
    blank_node_scope: BlankNodeScope,
    /// Mapping from parsed blank node labels to scoped blank nodes
    blank_node_map: HashMap<String, BlankNode>,
}

impl TurtleParser {
    /// Create a new Turtle parser
    pub fn new() -> Self {
        Self {
            namespaces: NamespaceRegistry::with_defaults(),
            base_iri: None,
            blank_node_scope: BlankNodeScope::generate(),
            blank_node_map: HashMap::new(),
        }
    }

    /// Create a parser with a base IRI
    pub fn with_base(base_iri: Iri) -> Self {
        Self {
            namespaces: NamespaceRegistry::with_defaults(),
            base_iri: Some(base_iri),
            blank_node_scope: BlankNodeScope::generate(),
            blank_node_map: HashMap::new(),
        }
    }

    /// Parse a Turtle document and return triples
    pub fn parse(&mut self, input: &str) -> Result<Vec<Triple>> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize()?;
        self.parse_tokens(&tokens)
    }

    /// Parse from pre-tokenized input
    fn parse_tokens(&mut self, tokens: &[Token]) -> Result<Vec<Triple>> {
        let mut triples = Vec::new();
        let mut pos = 0;

        while pos < tokens.len() {
            if tokens[pos].kind == TokenKind::Eof {
                break;
            }

            pos = self.parse_statement(tokens, pos, &mut triples)?;
        }

        Ok(triples)
    }

    /// Parse a single statement (directive or triples)
    fn parse_statement(
        &mut self,
        tokens: &[Token],
        pos: usize,
        triples: &mut Vec<Triple>,
    ) -> Result<usize> {
        match &tokens[pos].kind {
            TokenKind::PrefixKeyword | TokenKind::SparqlPrefix => {
                self.parse_prefix_directive(tokens, pos)
            }
            TokenKind::BaseKeyword | TokenKind::SparqlBase => {
                self.parse_base_directive(tokens, pos)
            }
            _ => self.parse_triples(tokens, pos, triples),
        }
    }

    /// Parse @prefix directive
    fn parse_prefix_directive(&mut self, tokens: &[Token], pos: usize) -> Result<usize> {
        let mut pos = pos + 1; // skip @prefix or PREFIX

        // Expect prefixed name (prefix:)
        let (prefix, new_pos) = self.expect_prefix_decl(tokens, pos)?;
        pos = new_pos;

        // Expect IRI
        let (namespace, new_pos) = self.expect_iri_ref(tokens, pos)?;
        pos = new_pos;

        // Register the prefix
        self.namespaces.add(prefix, namespace.as_str());

        // Expect dot (optional for SPARQL-style)
        if pos < tokens.len() && tokens[pos].kind == TokenKind::Dot {
            pos += 1;
        }

        Ok(pos)
    }

    /// Parse @base directive
    fn parse_base_directive(&mut self, tokens: &[Token], pos: usize) -> Result<usize> {
        let mut pos = pos + 1; // skip @base or BASE

        // Expect IRI
        let (base_iri, new_pos) = self.expect_iri_ref(tokens, pos)?;
        pos = new_pos;

        self.base_iri = Some(base_iri);

        // Expect dot (optional for SPARQL-style)
        if pos < tokens.len() && tokens[pos].kind == TokenKind::Dot {
            pos += 1;
        }

        Ok(pos)
    }

    /// Parse triples statement
    fn parse_triples(
        &mut self,
        tokens: &[Token],
        pos: usize,
        triples: &mut Vec<Triple>,
    ) -> Result<usize> {
        // Parse subject (start at depth 0)
        let (subject, mut pos) = self.parse_subject(tokens, pos, triples, 0)?;

        // Parse predicate-object list
        pos = self.parse_predicate_object_list(tokens, pos, &subject, triples, 0)?;

        // Expect dot
        if pos < tokens.len() && tokens[pos].kind == TokenKind::Dot {
            pos += 1;
        }

        Ok(pos)
    }

    /// Parse subject (IRI, blank node, or collection)
    ///
    /// The `depth` parameter tracks nesting level to prevent stack overflow.
    fn parse_subject(
        &mut self,
        tokens: &[Token],
        pos: usize,
        triples: &mut Vec<Triple>,
        depth: usize,
    ) -> Result<(Subject, usize)> {
        if depth > MAX_NESTING_DEPTH {
            return Err(ParserError::ParseError(format!(
                "Maximum nesting depth ({}) exceeded at line {}, column {}",
                MAX_NESTING_DEPTH, tokens[pos].line, tokens[pos].column
            )));
        }

        match &tokens[pos].kind {
            TokenKind::IriRef(iri) => {
                let iri = self.resolve_iri(iri)?;
                Ok((Subject::Iri(iri), pos + 1))
            }
            TokenKind::PrefixedName { prefix, local } => {
                let iri = self.expand_prefixed_name(prefix, local)?;
                Ok((Subject::Iri(iri), pos + 1))
            }
            TokenKind::BlankNodeLabel(label) => {
                let bn = self.get_or_create_blank_node(label);
                Ok((Subject::BlankNode(bn), pos + 1))
            }
            TokenKind::OpenBracket => {
                // Blank node with properties
                let (bn, new_pos) =
                    self.parse_blank_node_property_list(tokens, pos + 1, triples, depth + 1)?;
                Ok((Subject::BlankNode(bn), new_pos))
            }
            TokenKind::OpenParen => {
                // Collection
                let (subject, new_pos) =
                    self.parse_collection(tokens, pos + 1, triples, depth + 1)?;
                Ok((subject, new_pos))
            }
            _ => Err(ParserError::ParseError(format!(
                "Expected subject at line {}, column {}",
                tokens[pos].line, tokens[pos].column
            ))),
        }
    }

    /// Parse predicate (IRI or 'a')
    fn parse_predicate(&mut self, tokens: &[Token], pos: usize) -> Result<(Predicate, usize)> {
        match &tokens[pos].kind {
            TokenKind::IriRef(iri) => {
                let iri = self.resolve_iri(iri)?;
                Ok((iri, pos + 1))
            }
            TokenKind::PrefixedName { prefix, local } => {
                let iri = self.expand_prefixed_name(prefix, local)?;
                Ok((iri, pos + 1))
            }
            TokenKind::A => {
                let iri = Iri::new_unchecked(RDF_TYPE);
                Ok((iri, pos + 1))
            }
            _ => Err(ParserError::ParseError(format!(
                "Expected predicate at line {}, column {}",
                tokens[pos].line, tokens[pos].column
            ))),
        }
    }

    /// Parse object (IRI, blank node, literal, or collection)
    ///
    /// The `depth` parameter tracks nesting level to prevent stack overflow.
    fn parse_object(
        &mut self,
        tokens: &[Token],
        pos: usize,
        triples: &mut Vec<Triple>,
        depth: usize,
    ) -> Result<(Object, usize)> {
        if depth > MAX_NESTING_DEPTH {
            return Err(ParserError::ParseError(format!(
                "Maximum nesting depth ({}) exceeded at line {}, column {}",
                MAX_NESTING_DEPTH, tokens[pos].line, tokens[pos].column
            )));
        }

        match &tokens[pos].kind {
            TokenKind::IriRef(iri) => {
                let iri = self.resolve_iri(iri)?;
                Ok((Object::Iri(iri), pos + 1))
            }
            TokenKind::PrefixedName { prefix, local } => {
                let iri = self.expand_prefixed_name(prefix, local)?;
                Ok((Object::Iri(iri), pos + 1))
            }
            TokenKind::BlankNodeLabel(label) => {
                let bn = self.get_or_create_blank_node(label);
                Ok((Object::BlankNode(bn), pos + 1))
            }
            TokenKind::OpenBracket => {
                let (bn, new_pos) =
                    self.parse_blank_node_property_list(tokens, pos + 1, triples, depth + 1)?;
                Ok((Object::BlankNode(bn), new_pos))
            }
            TokenKind::OpenParen => {
                let (subject, new_pos) =
                    self.parse_collection(tokens, pos + 1, triples, depth + 1)?;
                let object = match subject {
                    Subject::Iri(iri) => Object::Iri(iri),
                    Subject::BlankNode(bn) => Object::BlankNode(bn),
                };
                Ok((object, new_pos))
            }
            TokenKind::StringLiteral(value) => self.parse_literal(tokens, pos, value.clone()),
            TokenKind::Integer(n) => {
                let lit = Literal::integer(*n);
                Ok((Object::Literal(lit), pos + 1))
            }
            TokenKind::Decimal(n) => {
                let lit = Literal::decimal(*n);
                Ok((Object::Literal(lit), pos + 1))
            }
            TokenKind::Double(n) => {
                let lit = Literal::double(*n);
                Ok((Object::Literal(lit), pos + 1))
            }
            TokenKind::Boolean(b) => {
                let lit = Literal::boolean(*b);
                Ok((Object::Literal(lit), pos + 1))
            }
            _ => Err(ParserError::ParseError(format!(
                "Expected object at line {}, column {}",
                tokens[pos].line, tokens[pos].column
            ))),
        }
    }

    /// Parse a literal with optional language tag or datatype
    fn parse_literal(
        &mut self,
        tokens: &[Token],
        pos: usize,
        value: String,
    ) -> Result<(Object, usize)> {
        let mut pos = pos + 1;

        if pos < tokens.len() {
            match &tokens[pos].kind {
                TokenKind::LangTag(lang) => {
                    let lit = Literal::with_language(value, lang.as_str())?;
                    return Ok((Object::Literal(lit), pos + 1));
                }
                TokenKind::DoubleCaret => {
                    pos += 1;
                    let (datatype, new_pos) = self.parse_datatype_iri(tokens, pos)?;
                    let lit = Literal::with_datatype(value, datatype);
                    return Ok((Object::Literal(lit), new_pos));
                }
                _ => {}
            }
        }

        let lit = Literal::new(value);
        Ok((Object::Literal(lit), pos))
    }

    /// Parse datatype IRI after ^^
    fn parse_datatype_iri(&mut self, tokens: &[Token], pos: usize) -> Result<(Iri, usize)> {
        match &tokens[pos].kind {
            TokenKind::IriRef(iri) => {
                let iri = self.resolve_iri(iri)?;
                Ok((iri, pos + 1))
            }
            TokenKind::PrefixedName { prefix, local } => {
                let iri = self.expand_prefixed_name(prefix, local)?;
                Ok((iri, pos + 1))
            }
            _ => Err(ParserError::ParseError(format!(
                "Expected datatype IRI at line {}, column {}",
                tokens[pos].line, tokens[pos].column
            ))),
        }
    }

    /// Parse predicate-object list (handles ; and ,)
    ///
    /// The `depth` parameter tracks nesting level to prevent stack overflow.
    fn parse_predicate_object_list(
        &mut self,
        tokens: &[Token],
        mut pos: usize,
        subject: &Subject,
        triples: &mut Vec<Triple>,
        depth: usize,
    ) -> Result<usize> {
        loop {
            // Parse predicate
            let (predicate, new_pos) = self.parse_predicate(tokens, pos)?;
            pos = new_pos;

            // Parse object list (handles ,)
            pos = self.parse_object_list(tokens, pos, subject, &predicate, triples, depth)?;

            // Check for semicolon (more predicate-object pairs)
            if pos < tokens.len() && tokens[pos].kind == TokenKind::Semicolon {
                pos += 1;
                // Skip any trailing semicolons and check if we have more predicates
                while pos < tokens.len() && tokens[pos].kind == TokenKind::Semicolon {
                    pos += 1;
                }
                // Check if next token can be a predicate
                if pos >= tokens.len() {
                    break;
                }
                match &tokens[pos].kind {
                    TokenKind::Dot | TokenKind::CloseBracket | TokenKind::Eof => break,
                    _ => continue,
                }
            } else {
                break;
            }
        }

        Ok(pos)
    }

    /// Parse object list (handles ,)
    ///
    /// The `depth` parameter tracks nesting level to prevent stack overflow.
    fn parse_object_list(
        &mut self,
        tokens: &[Token],
        mut pos: usize,
        subject: &Subject,
        predicate: &Predicate,
        triples: &mut Vec<Triple>,
        depth: usize,
    ) -> Result<usize> {
        loop {
            // Parse object
            let (object, new_pos) = self.parse_object(tokens, pos, triples, depth)?;
            pos = new_pos;

            // Create triple
            triples.push(Triple::new(subject.clone(), predicate.clone(), object));

            // Check for comma (more objects)
            if pos < tokens.len() && tokens[pos].kind == TokenKind::Comma {
                pos += 1;
            } else {
                break;
            }
        }

        Ok(pos)
    }

    /// Parse blank node property list: [ predicate object ; ... ]
    ///
    /// The `depth` parameter tracks nesting level to prevent stack overflow.
    fn parse_blank_node_property_list(
        &mut self,
        tokens: &[Token],
        mut pos: usize,
        triples: &mut Vec<Triple>,
        depth: usize,
    ) -> Result<(BlankNode, usize)> {
        if depth > MAX_NESTING_DEPTH {
            return Err(ParserError::ParseError(format!(
                "Maximum nesting depth ({}) exceeded at line {}, column {}",
                MAX_NESTING_DEPTH,
                tokens.get(pos).map(|t| t.line).unwrap_or(0),
                tokens.get(pos).map(|t| t.column).unwrap_or(0)
            )));
        }

        let bn = self.blank_node_scope.next();

        // Check for empty blank node []
        if pos < tokens.len() && tokens[pos].kind == TokenKind::CloseBracket {
            return Ok((bn, pos + 1));
        }

        // Parse predicate-object list
        let subject = Subject::BlankNode(bn.clone());
        pos = self.parse_predicate_object_list(tokens, pos, &subject, triples, depth)?;

        // Expect closing bracket
        if pos >= tokens.len() || tokens[pos].kind != TokenKind::CloseBracket {
            return Err(ParserError::ParseError(format!(
                "Expected ']' at line {}, column {}",
                tokens.get(pos).map(|t| t.line).unwrap_or(0),
                tokens.get(pos).map(|t| t.column).unwrap_or(0)
            )));
        }
        pos += 1;

        Ok((bn, pos))
    }

    /// Parse collection: ( item1 item2 ... )
    ///
    /// The `depth` parameter tracks nesting level to prevent stack overflow.
    fn parse_collection(
        &mut self,
        tokens: &[Token],
        mut pos: usize,
        triples: &mut Vec<Triple>,
        depth: usize,
    ) -> Result<(Subject, usize)> {
        if depth > MAX_NESTING_DEPTH {
            return Err(ParserError::ParseError(format!(
                "Maximum nesting depth ({}) exceeded at line {}, column {}",
                MAX_NESTING_DEPTH,
                tokens.get(pos).map(|t| t.line).unwrap_or(0),
                tokens.get(pos).map(|t| t.column).unwrap_or(0)
            )));
        }

        // Check for empty collection
        if pos < tokens.len() && tokens[pos].kind == TokenKind::CloseParen {
            let nil = Iri::new_unchecked(RDF_NIL);
            return Ok((Subject::Iri(nil), pos + 1));
        }

        let first_pred = Iri::new_unchecked(RDF_FIRST);
        let rest_pred = Iri::new_unchecked(RDF_REST);
        let nil = Iri::new_unchecked(RDF_NIL);

        let head = self.blank_node_scope.next();
        let mut current = head.clone();

        loop {
            // Parse item as object (increment depth for nested items)
            let (item, new_pos) = self.parse_object(tokens, pos, triples, depth + 1)?;
            pos = new_pos;

            // Add rdf:first triple
            triples.push(Triple::new(
                Subject::BlankNode(current.clone()),
                first_pred.clone(),
                item,
            ));

            // Check for closing paren
            if pos < tokens.len() && tokens[pos].kind == TokenKind::CloseParen {
                // Add rdf:rest rdf:nil
                triples.push(Triple::new(
                    Subject::BlankNode(current),
                    rest_pred.clone(),
                    Object::Iri(nil),
                ));
                pos += 1;
                break;
            }

            // Create next node and link with rdf:rest
            let next = self.blank_node_scope.next();
            triples.push(Triple::new(
                Subject::BlankNode(current),
                rest_pred.clone(),
                Object::BlankNode(next.clone()),
            ));
            current = next;
        }

        Ok((Subject::BlankNode(head), pos))
    }

    /// Get or create a blank node for a label
    fn get_or_create_blank_node(&mut self, label: &str) -> BlankNode {
        if let Some(bn) = self.blank_node_map.get(label) {
            bn.clone()
        } else {
            let bn = self.blank_node_scope.map(label);
            self.blank_node_map.insert(label.to_string(), bn.clone());
            bn
        }
    }

    /// Resolve a relative IRI against the base
    fn resolve_iri(&self, iri: &str) -> Result<Iri> {
        if let Some(ref base) = self.base_iri {
            base.resolve(iri)
        } else {
            Iri::new(iri)
        }
    }

    /// Expand a prefixed name to a full IRI
    fn expand_prefixed_name(&self, prefix: &str, local: &str) -> Result<Iri> {
        let namespace = self
            .namespaces
            .get_namespace(prefix)
            .ok_or_else(|| ParserError::InvalidInput(format!("Unknown prefix: {}", prefix)))?;
        Iri::new(format!("{}{}", namespace, local))
    }

    /// Expect a prefix declaration (prefix:)
    fn expect_prefix_decl(&self, tokens: &[Token], pos: usize) -> Result<(String, usize)> {
        match &tokens[pos].kind {
            TokenKind::PrefixedName { prefix, local } if local.is_empty() => {
                Ok((prefix.clone(), pos + 1))
            }
            _ => Err(ParserError::ParseError(format!(
                "Expected prefix declaration at line {}, column {}",
                tokens[pos].line, tokens[pos].column
            ))),
        }
    }

    /// Expect an IRI reference
    fn expect_iri_ref(&self, tokens: &[Token], pos: usize) -> Result<(Iri, usize)> {
        match &tokens[pos].kind {
            TokenKind::IriRef(iri) => {
                let iri = Iri::new(iri.as_str())?;
                Ok((iri, pos + 1))
            }
            _ => Err(ParserError::ParseError(format!(
                "Expected IRI at line {}, column {}",
                tokens[pos].line, tokens[pos].column
            ))),
        }
    }

    /// Get the namespace registry
    pub fn namespaces(&self) -> &NamespaceRegistry {
        &self.namespaces
    }

    /// Get mutable reference to namespace registry
    pub fn namespaces_mut(&mut self) -> &mut NamespaceRegistry {
        &mut self.namespaces
    }

    /// Get the base IRI
    pub fn base_iri(&self) -> Option<&Iri> {
        self.base_iri.as_ref()
    }
}

impl Default for TurtleParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_triple() {
        let mut parser = TurtleParser::new();
        let triples = parser
            .parse("<http://example.org/subject> <http://example.org/predicate> <http://example.org/object> .")
            .unwrap();

        assert_eq!(triples.len(), 1);
        assert_eq!(
            triples[0].subject().as_iri().unwrap().as_str(),
            "http://example.org/subject"
        );
        assert_eq!(
            triples[0].predicate().as_str(),
            "http://example.org/predicate"
        );
        assert!(triples[0].object().is_iri());
    }

    #[test]
    fn test_prefix_declaration() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:predicate ex:object .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert_eq!(
            triples[0].subject().as_iri().unwrap().as_str(),
            "http://example.org/subject"
        );
    }

    #[test]
    fn test_base_directive() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @base <http://example.org/> .
            <subject> <predicate> <object> .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert_eq!(
            triples[0].subject().as_iri().unwrap().as_str(),
            "http://example.org/subject"
        );
    }

    #[test]
    fn test_a_keyword() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject a ex:Type .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0].predicate().as_str(), RDF_TYPE);
    }

    #[test]
    fn test_literal_object() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:name "John Doe" .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        let lit = triples[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "John Doe");
    }

    #[test]
    fn test_language_tagged_literal() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:label "Hello"@en .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        let lit = triples[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "Hello");
        assert_eq!(lit.language(), Some("en"));
    }

    #[test]
    fn test_typed_literal() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
            ex:subject ex:age "42"^^xsd:integer .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        let lit = triples[0].object().as_literal().unwrap();
        assert_eq!(lit.value(), "42");
        assert_eq!(
            lit.explicit_datatype().unwrap().as_str(),
            "http://www.w3.org/2001/XMLSchema#integer"
        );
    }

    #[test]
    fn test_numeric_literals() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:count 42 ;
                       ex:ratio 3.14 ;
                       ex:large 1.5e10 .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 3);
        assert_eq!(
            triples[0].object().as_literal().unwrap().as_integer(),
            Some(42)
        );
    }

    #[test]
    fn test_boolean_literal() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:active true .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert_eq!(
            triples[0].object().as_literal().unwrap().as_bool(),
            Some(true)
        );
    }

    #[test]
    fn test_blank_node_label() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            _:node1 ex:predicate ex:object .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert!(triples[0].subject().is_blank_node());
    }

    #[test]
    fn test_semicolon_syntax() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:prop1 "value1" ;
                       ex:prop2 "value2" ;
                       ex:prop3 "value3" .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 3);
    }

    #[test]
    fn test_comma_syntax() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:values "a", "b", "c" .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 3);
        // All triples should have the same subject and predicate
        for triple in &triples {
            assert_eq!(
                triple.subject().as_iri().unwrap().as_str(),
                "http://example.org/subject"
            );
            assert_eq!(triple.predicate().as_str(), "http://example.org/values");
        }
    }

    #[test]
    fn test_nested_blank_node() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:address [
                ex:street "123 Main St" ;
                ex:city "Springfield"
            ] .
        "#;
        let triples = parser.parse(input).unwrap();

        // Should have 3 triples:
        // 1. ex:subject ex:address _:b1
        // 2. _:b1 ex:street "123 Main St"
        // 3. _:b1 ex:city "Springfield"
        assert_eq!(triples.len(), 3);
    }

    #[test]
    fn test_empty_blank_node() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:related [] .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert!(triples[0].object().is_blank_node());
    }

    #[test]
    fn test_collection() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:list ("a" "b" "c") .
        "#;
        let triples = parser.parse(input).unwrap();

        // Collection ("a" "b" "c") generates:
        // ex:subject ex:list _:b1
        // _:b1 rdf:first "a"
        // _:b1 rdf:rest _:b2
        // _:b2 rdf:first "b"
        // _:b2 rdf:rest _:b3
        // _:b3 rdf:first "c"
        // _:b3 rdf:rest rdf:nil
        assert_eq!(triples.len(), 7);
    }

    #[test]
    fn test_empty_collection() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:list () .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert_eq!(triples[0].object().as_iri().unwrap().as_str(), RDF_NIL);
    }

    #[test]
    fn test_sparql_style_prefix() {
        let mut parser = TurtleParser::new();
        let input = r#"
            PREFIX ex: <http://example.org/>
            ex:subject ex:predicate ex:object .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
    }

    #[test]
    fn test_sparql_style_base() {
        let mut parser = TurtleParser::new();
        let input = r#"
            BASE <http://example.org/>
            <subject> <predicate> <object> .
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
        assert_eq!(
            triples[0].subject().as_iri().unwrap().as_str(),
            "http://example.org/subject"
        );
    }

    #[test]
    fn test_comments() {
        let mut parser = TurtleParser::new();
        let input = r#"
            # This is a comment
            @prefix ex: <http://example.org/> .
            # Another comment
            ex:subject ex:predicate ex:object . # inline comment
        "#;
        let triples = parser.parse(input).unwrap();

        assert_eq!(triples.len(), 1);
    }

    #[test]
    fn test_blank_node_as_subject() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            [
                ex:name "Anonymous" ;
                ex:type ex:Person
            ] ex:knows ex:someone .
        "#;
        let triples = parser.parse(input).unwrap();

        // Should have:
        // _:b1 ex:name "Anonymous"
        // _:b1 ex:type ex:Person
        // _:b1 ex:knows ex:someone
        assert_eq!(triples.len(), 3);
    }

    #[test]
    fn test_deeply_nested_blank_nodes() {
        let mut parser = TurtleParser::new();
        let input = r#"
            @prefix ex: <http://example.org/> .
            ex:subject ex:outer [
                ex:inner [
                    ex:value "deep"
                ]
            ] .
        "#;
        let triples = parser.parse(input).unwrap();

        // ex:subject ex:outer _:b1
        // _:b1 ex:inner _:b2
        // _:b2 ex:value "deep"
        assert_eq!(triples.len(), 3);
    }

    #[test]
    fn test_max_nesting_depth_exceeded() {
        let mut parser = TurtleParser::new();

        // Generate deeply nested blank nodes that exceed MAX_NESTING_DEPTH
        let mut input = String::from("@prefix ex: <http://example.org/> .\nex:subject ex:prop ");
        for _ in 0..150 {
            input.push_str("[ ex:nested ");
        }
        input.push_str("\"value\"");
        for _ in 0..150 {
            input.push_str(" ]");
        }
        input.push_str(" .");

        let result = parser.parse(&input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Maximum nesting depth"));
    }

    #[test]
    fn test_moderate_nesting_allowed() {
        let mut parser = TurtleParser::new();

        // Generate nested blank nodes within limits (10 levels is fine)
        let mut input = String::from("@prefix ex: <http://example.org/> .\nex:subject ex:prop ");
        for i in 0..10 {
            input.push_str(&format!("[ ex:level{} ", i));
        }
        input.push_str("\"deep\"");
        for _ in 0..10 {
            input.push_str(" ]");
        }
        input.push_str(" .");

        let result = parser.parse(&input);
        assert!(result.is_ok());
    }
}
