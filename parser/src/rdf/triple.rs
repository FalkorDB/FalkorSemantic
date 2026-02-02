//! RDF Triple and Quad implementation
//!
//! Core data structures for RDF statements.

use std::fmt;

use super::{BlankNode, Iri, Literal};

/// A subject in an RDF triple
///
/// Can be either an IRI or a blank node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    /// An IRI reference
    Iri(Iri),
    /// A blank node
    BlankNode(BlankNode),
}

impl Subject {
    /// Check if this subject is an IRI
    pub fn is_iri(&self) -> bool {
        matches!(self, Subject::Iri(_))
    }

    /// Check if this subject is a blank node
    pub fn is_blank_node(&self) -> bool {
        matches!(self, Subject::BlankNode(_))
    }

    /// Get the IRI if this is an IRI subject
    pub fn as_iri(&self) -> Option<&Iri> {
        match self {
            Subject::Iri(iri) => Some(iri),
            _ => None,
        }
    }

    /// Get the blank node if this is a blank node subject
    pub fn as_blank_node(&self) -> Option<&BlankNode> {
        match self {
            Subject::BlankNode(bn) => Some(bn),
            _ => None,
        }
    }
}

impl From<Iri> for Subject {
    fn from(iri: Iri) -> Self {
        Subject::Iri(iri)
    }
}

impl From<BlankNode> for Subject {
    fn from(bn: BlankNode) -> Self {
        Subject::BlankNode(bn)
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Subject::Iri(iri) => write!(f, "{}", iri),
            Subject::BlankNode(bn) => write!(f, "{}", bn),
        }
    }
}

/// A predicate in an RDF triple
///
/// Predicates are always IRIs.
pub type Predicate = Iri;

/// An object in an RDF triple
///
/// Can be an IRI, a blank node, or a literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Object {
    /// An IRI reference
    Iri(Iri),
    /// A blank node
    BlankNode(BlankNode),
    /// A literal value
    Literal(Literal),
}

impl Object {
    /// Check if this object is an IRI
    pub fn is_iri(&self) -> bool {
        matches!(self, Object::Iri(_))
    }

    /// Check if this object is a blank node
    pub fn is_blank_node(&self) -> bool {
        matches!(self, Object::BlankNode(_))
    }

    /// Check if this object is a literal
    pub fn is_literal(&self) -> bool {
        matches!(self, Object::Literal(_))
    }

    /// Get the IRI if this is an IRI object
    pub fn as_iri(&self) -> Option<&Iri> {
        match self {
            Object::Iri(iri) => Some(iri),
            _ => None,
        }
    }

    /// Get the blank node if this is a blank node object
    pub fn as_blank_node(&self) -> Option<&BlankNode> {
        match self {
            Object::BlankNode(bn) => Some(bn),
            _ => None,
        }
    }

    /// Get the literal if this is a literal object
    pub fn as_literal(&self) -> Option<&Literal> {
        match self {
            Object::Literal(lit) => Some(lit),
            _ => None,
        }
    }
}

impl From<Iri> for Object {
    fn from(iri: Iri) -> Self {
        Object::Iri(iri)
    }
}

impl From<BlankNode> for Object {
    fn from(bn: BlankNode) -> Self {
        Object::BlankNode(bn)
    }
}

impl From<Literal> for Object {
    fn from(lit: Literal) -> Self {
        Object::Literal(lit)
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Object::Iri(iri) => write!(f, "{}", iri),
            Object::BlankNode(bn) => write!(f, "{}", bn),
            Object::Literal(lit) => write!(f, "{}", lit),
        }
    }
}

/// An RDF Triple (subject, predicate, object)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Triple {
    /// The subject of the triple
    pub subject: Subject,
    /// The predicate of the triple
    pub predicate: Predicate,
    /// The object of the triple
    pub object: Object,
}

impl Triple {
    /// Create a new triple
    pub fn new(subject: impl Into<Subject>, predicate: Predicate, object: impl Into<Object>) -> Self {
        Self {
            subject: subject.into(),
            predicate,
            object: object.into(),
        }
    }

    /// Get the subject
    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    /// Get the predicate
    pub fn predicate(&self) -> &Predicate {
        &self.predicate
    }

    /// Get the object
    pub fn object(&self) -> &Object {
        &self.object
    }
}

impl fmt::Display for Triple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {} .", self.subject, self.predicate, self.object)
    }
}

/// A graph name for named graphs
///
/// Can be an IRI or a blank node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GraphName {
    /// An IRI reference
    Iri(Iri),
    /// A blank node
    BlankNode(BlankNode),
}

impl From<Iri> for GraphName {
    fn from(iri: Iri) -> Self {
        GraphName::Iri(iri)
    }
}

impl From<BlankNode> for GraphName {
    fn from(bn: BlankNode) -> Self {
        GraphName::BlankNode(bn)
    }
}

impl fmt::Display for GraphName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphName::Iri(iri) => write!(f, "{}", iri),
            GraphName::BlankNode(bn) => write!(f, "{}", bn),
        }
    }
}

/// An RDF Quad (triple + graph name)
///
/// Quads are triples that belong to a named graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Quad {
    /// The triple
    pub triple: Triple,
    /// The graph name (None for the default graph)
    pub graph: Option<GraphName>,
}

impl Quad {
    /// Create a new quad
    pub fn new(triple: Triple, graph: Option<GraphName>) -> Self {
        Self { triple, graph }
    }

    /// Create a quad in the default graph
    pub fn in_default_graph(triple: Triple) -> Self {
        Self {
            triple,
            graph: None,
        }
    }

    /// Create a quad in a named graph
    pub fn in_named_graph(triple: Triple, graph: impl Into<GraphName>) -> Self {
        Self {
            triple,
            graph: Some(graph.into()),
        }
    }

    /// Get the subject
    pub fn subject(&self) -> &Subject {
        &self.triple.subject
    }

    /// Get the predicate
    pub fn predicate(&self) -> &Predicate {
        &self.triple.predicate
    }

    /// Get the object
    pub fn object(&self) -> &Object {
        &self.triple.object
    }

    /// Get the graph name
    pub fn graph(&self) -> Option<&GraphName> {
        self.graph.as_ref()
    }

    /// Check if this quad is in the default graph
    pub fn is_default_graph(&self) -> bool {
        self.graph.is_none()
    }
}

impl From<Triple> for Quad {
    fn from(triple: Triple) -> Self {
        Quad::in_default_graph(triple)
    }
}

impl fmt::Display for Quad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref graph) = self.graph {
            write!(
                f,
                "{} {} {} {} .",
                self.triple.subject, self.triple.predicate, self.triple.object, graph
            )
        } else {
            write!(f, "{}", self.triple)
        }
    }
}

/// A triple pattern for matching/deletion operations
///
/// Each component is optional - `None` represents a wildcard that matches any value.
/// Used for pattern-based queries like DELETE operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TriplePattern {
    /// The subject pattern (None = wildcard)
    pub subject: Option<Subject>,
    /// The predicate pattern (None = wildcard)
    pub predicate: Option<Predicate>,
    /// The object pattern (None = wildcard)
    pub object: Option<Object>,
}

impl TriplePattern {
    /// Create a new triple pattern with all wildcards
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a pattern matching a specific triple exactly
    pub fn from_triple(triple: &Triple) -> Self {
        Self {
            subject: Some(triple.subject.clone()),
            predicate: Some(triple.predicate.clone()),
            object: Some(triple.object.clone()),
        }
    }

    /// Create a pattern with a specific subject (wildcard predicate/object)
    pub fn with_subject(subject: impl Into<Subject>) -> Self {
        Self {
            subject: Some(subject.into()),
            predicate: None,
            object: None,
        }
    }

    /// Create a pattern with a specific predicate (wildcard subject/object)
    pub fn with_predicate(predicate: Predicate) -> Self {
        Self {
            subject: None,
            predicate: Some(predicate),
            object: None,
        }
    }

    /// Create a pattern with a specific object (wildcard subject/predicate)
    pub fn with_object(object: impl Into<Object>) -> Self {
        Self {
            subject: None,
            predicate: None,
            object: Some(object.into()),
        }
    }

    /// Set the subject pattern
    pub fn subject(mut self, subject: impl Into<Subject>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Set the predicate pattern
    pub fn predicate(mut self, predicate: Predicate) -> Self {
        self.predicate = Some(predicate);
        self
    }

    /// Set the object pattern
    pub fn object(mut self, object: impl Into<Object>) -> Self {
        self.object = Some(object.into());
        self
    }

    /// Check if this pattern matches a given triple
    pub fn matches(&self, triple: &Triple) -> bool {
        let subject_matches = self
            .subject
            .as_ref()
            .map_or(true, |s| s == &triple.subject);
        let predicate_matches = self
            .predicate
            .as_ref()
            .map_or(true, |p| p == &triple.predicate);
        let object_matches = self.object.as_ref().map_or(true, |o| o == &triple.object);

        subject_matches && predicate_matches && object_matches
    }

    /// Check if all components are wildcards
    pub fn is_all_wildcard(&self) -> bool {
        self.subject.is_none() && self.predicate.is_none() && self.object.is_none()
    }

    /// Check if this pattern has any wildcards
    pub fn has_wildcard(&self) -> bool {
        self.subject.is_none() || self.predicate.is_none() || self.object.is_none()
    }

    /// Check if this is an exact pattern (no wildcards)
    pub fn is_exact(&self) -> bool {
        self.subject.is_some() && self.predicate.is_some() && self.object.is_some()
    }
}

impl fmt::Display for TriplePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let subject = self
            .subject
            .as_ref()
            .map_or("*".to_string(), |s| s.to_string());
        let predicate = self
            .predicate
            .as_ref()
            .map_or("*".to_string(), |p| p.to_string());
        let object = self
            .object
            .as_ref()
            .map_or("*".to_string(), |o| o.to_string());
        write!(f, "{} {} {} .", subject, predicate, object)
    }
}

/// A quad pattern for matching/deletion operations with graph scope
///
/// Extends `TriplePattern` with an optional graph component.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct QuadPattern {
    /// The triple pattern
    pub pattern: TriplePattern,
    /// The graph pattern (None = default graph, Some(None) = any graph, Some(Some(_)) = specific graph)
    pub graph: Option<Option<GraphName>>,
}

/// Specifies which graphs a pattern should match
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GraphScope {
    /// Match only the default (unnamed) graph
    #[default]
    Default,
    /// Match a specific named graph
    Named(GraphName),
    /// Match all graphs (default and named)
    All,
}

impl QuadPattern {
    /// Create a new quad pattern with all wildcards in the default graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a quad pattern from a triple pattern (default graph)
    pub fn from_triple_pattern(pattern: TriplePattern) -> Self {
        Self {
            pattern,
            graph: None,
        }
    }

    /// Create a quad pattern matching a specific quad exactly
    pub fn from_quad(quad: &Quad) -> Self {
        Self {
            pattern: TriplePattern::from_triple(&quad.triple),
            graph: Some(quad.graph.clone()),
        }
    }

    /// Create a pattern scoped to a specific named graph
    pub fn in_graph(pattern: TriplePattern, graph: impl Into<GraphName>) -> Self {
        Self {
            pattern,
            graph: Some(Some(graph.into())),
        }
    }

    /// Create a pattern that matches across all graphs
    pub fn in_all_graphs(pattern: TriplePattern) -> Self {
        Self {
            pattern,
            graph: Some(None), // Some(None) means "any graph"
        }
    }

    /// Get the graph scope for this pattern
    pub fn graph_scope(&self) -> GraphScope {
        match &self.graph {
            None => GraphScope::Default,
            Some(None) => GraphScope::All,
            Some(Some(g)) => GraphScope::Named(g.clone()),
        }
    }

    /// Check if this pattern matches a given quad
    pub fn matches(&self, quad: &Quad) -> bool {
        if !self.pattern.matches(&quad.triple) {
            return false;
        }

        match &self.graph {
            None => quad.graph.is_none(), // Default graph only
            Some(None) => true,           // Any graph
            Some(Some(g)) => quad.graph.as_ref() == Some(g),
        }
    }

    /// Set the subject pattern
    pub fn subject(mut self, subject: impl Into<Subject>) -> Self {
        self.pattern.subject = Some(subject.into());
        self
    }

    /// Set the predicate pattern
    pub fn predicate(mut self, predicate: Predicate) -> Self {
        self.pattern.predicate = Some(predicate);
        self
    }

    /// Set the object pattern
    pub fn object(mut self, object: impl Into<Object>) -> Self {
        self.pattern.object = Some(object.into());
        self
    }
}

impl fmt::Display for QuadPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.graph {
            None => write!(f, "{}", self.pattern),
            Some(None) => {
                let s = self
                    .pattern
                    .subject
                    .as_ref()
                    .map_or("*".to_string(), |s| s.to_string());
                let p = self
                    .pattern
                    .predicate
                    .as_ref()
                    .map_or("*".to_string(), |p| p.to_string());
                let o = self
                    .pattern
                    .object
                    .as_ref()
                    .map_or("*".to_string(), |o| o.to_string());
                write!(f, "{} {} {} * .", s, p, o)
            }
            Some(Some(g)) => {
                let s = self
                    .pattern
                    .subject
                    .as_ref()
                    .map_or("*".to_string(), |s| s.to_string());
                let p = self
                    .pattern
                    .predicate
                    .as_ref()
                    .map_or("*".to_string(), |p| p.to_string());
                let o = self
                    .pattern
                    .object
                    .as_ref()
                    .map_or("*".to_string(), |o| o.to_string());
                write!(f, "{} {} {} {} .", s, p, o, g)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_triple_creation() {
        let subject = test_iri("http://example.org/subject");
        let predicate = test_iri("http://example.org/predicate");
        let object = Literal::new("value");

        let triple = Triple::new(subject, predicate, object);
        assert!(triple.subject().is_iri());
        assert!(triple.object().is_literal());
    }

    #[test]
    fn test_triple_display() {
        let subject = test_iri("http://example.org/s");
        let predicate = test_iri("http://example.org/p");
        let object = Literal::new("hello");

        let triple = Triple::new(subject, predicate, object);
        assert_eq!(
            format!("{}", triple),
            "<http://example.org/s> <http://example.org/p> \"hello\" ."
        );
    }

    #[test]
    fn test_quad_default_graph() {
        let subject = test_iri("http://example.org/s");
        let predicate = test_iri("http://example.org/p");
        let object = test_iri("http://example.org/o");

        let triple = Triple::new(subject, predicate, object);
        let quad = Quad::in_default_graph(triple);

        assert!(quad.is_default_graph());
    }

    #[test]
    fn test_quad_named_graph() {
        let subject = test_iri("http://example.org/s");
        let predicate = test_iri("http://example.org/p");
        let object = test_iri("http://example.org/o");
        let graph = test_iri("http://example.org/graph");

        let triple = Triple::new(subject, predicate, object);
        let quad = Quad::in_named_graph(triple, graph);

        assert!(!quad.is_default_graph());
        assert!(quad.graph().is_some());
    }

    #[test]
    fn test_subject_from_blank_node() {
        let bn = BlankNode::new("node1");
        let subject: Subject = bn.into();
        assert!(subject.is_blank_node());
        assert_eq!(format!("{}", subject), "_:node1");
    }

    #[test]
    fn test_object_variants() {
        let iri_obj: Object = test_iri("http://example.org/o").into();
        assert!(iri_obj.is_iri());

        let bn_obj: Object = BlankNode::new("bn").into();
        assert!(bn_obj.is_blank_node());

        let lit_obj: Object = Literal::new("text").into();
        assert!(lit_obj.is_literal());
    }

    // TriplePattern tests
    #[test]
    fn test_triple_pattern_wildcard() {
        let pattern = TriplePattern::new();
        assert!(pattern.is_all_wildcard());
        assert!(pattern.has_wildcard());
        assert!(!pattern.is_exact());
    }

    #[test]
    fn test_triple_pattern_from_triple() {
        let triple = Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            test_iri("http://example.org/o"),
        );
        let pattern = TriplePattern::from_triple(&triple);

        assert!(pattern.is_exact());
        assert!(!pattern.has_wildcard());
        assert!(pattern.matches(&triple));
    }

    #[test]
    fn test_triple_pattern_matches_subject_wildcard() {
        let pattern = TriplePattern::new()
            .predicate(test_iri("http://example.org/knows"))
            .object(test_iri("http://example.org/Bob"));

        let matching = Triple::new(
            test_iri("http://example.org/Alice"),
            test_iri("http://example.org/knows"),
            test_iri("http://example.org/Bob"),
        );
        let non_matching = Triple::new(
            test_iri("http://example.org/Alice"),
            test_iri("http://example.org/likes"),
            test_iri("http://example.org/Bob"),
        );

        assert!(pattern.matches(&matching));
        assert!(!pattern.matches(&non_matching));
    }

    #[test]
    fn test_triple_pattern_display() {
        let pattern = TriplePattern::with_subject(test_iri("http://example.org/s"));
        assert!(format!("{}", pattern).contains("<http://example.org/s>"));
        assert!(format!("{}", pattern).contains("*"));
    }

    // QuadPattern tests
    #[test]
    fn test_quad_pattern_default_graph() {
        let pattern = QuadPattern::from_triple_pattern(TriplePattern::new());

        let quad_in_default = Quad::in_default_graph(Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            test_iri("http://example.org/o"),
        ));
        let quad_in_named = Quad::in_named_graph(
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p"),
                test_iri("http://example.org/o"),
            ),
            test_iri("http://example.org/graph"),
        );

        assert!(pattern.matches(&quad_in_default));
        assert!(!pattern.matches(&quad_in_named));
    }

    #[test]
    fn test_quad_pattern_all_graphs() {
        let pattern = QuadPattern::in_all_graphs(TriplePattern::new());

        let quad_in_default = Quad::in_default_graph(Triple::new(
            test_iri("http://example.org/s"),
            test_iri("http://example.org/p"),
            test_iri("http://example.org/o"),
        ));
        let quad_in_named = Quad::in_named_graph(
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p"),
                test_iri("http://example.org/o"),
            ),
            test_iri("http://example.org/graph"),
        );

        assert!(pattern.matches(&quad_in_default));
        assert!(pattern.matches(&quad_in_named));
    }

    #[test]
    fn test_quad_pattern_named_graph() {
        let graph = test_iri("http://example.org/graph1");
        let pattern = QuadPattern::in_graph(TriplePattern::new(), graph.clone());

        let quad_graph1 = Quad::in_named_graph(
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p"),
                test_iri("http://example.org/o"),
            ),
            graph,
        );
        let quad_graph2 = Quad::in_named_graph(
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p"),
                test_iri("http://example.org/o"),
            ),
            test_iri("http://example.org/graph2"),
        );

        assert!(pattern.matches(&quad_graph1));
        assert!(!pattern.matches(&quad_graph2));
    }

    #[test]
    fn test_graph_scope() {
        let default_pattern = QuadPattern::new();
        assert!(matches!(default_pattern.graph_scope(), GraphScope::Default));

        let all_pattern = QuadPattern::in_all_graphs(TriplePattern::new());
        assert!(matches!(all_pattern.graph_scope(), GraphScope::All));

        let named_pattern =
            QuadPattern::in_graph(TriplePattern::new(), test_iri("http://example.org/g"));
        assert!(matches!(named_pattern.graph_scope(), GraphScope::Named(_)));
    }
}
