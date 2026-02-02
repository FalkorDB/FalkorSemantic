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
}
