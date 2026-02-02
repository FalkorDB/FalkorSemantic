//! JSON-LD Serializer
//!
//! Serializes RDF data in JSON-LD format.
//! https://www.w3.org/TR/json-ld/

use std::collections::HashMap;
use std::io::Write;

use super::error::SerializerResult;
use super::traits::{escape_json_string, QuadSerializer, TripleSerializer};
use crate::rdf::{GraphName, Object, Quad, Subject, Triple};

/// JSON-LD serializer
///
/// Serializes RDF triples/quads in JSON-LD format. Supports:
/// - Compact form with @context
/// - Expanded form (full IRIs)
/// - Named graphs via @graph
#[derive(Debug)]
pub struct JsonLdSerializer {
    /// Namespace prefixes for context
    prefixes: HashMap<String, String>,
    /// Whether to use compact form with @context
    use_context: bool,
    /// Accumulated nodes by subject
    nodes: HashMap<String, JsonLdNode>,
    /// Named graphs (graph IRI -> nodes)
    named_graphs: HashMap<String, HashMap<String, JsonLdNode>>,
    /// Pretty print output
    pretty: bool,
}

/// Internal representation of a JSON-LD node
#[derive(Debug, Clone)]
struct JsonLdNode {
    /// Node @id
    id: Option<String>,
    /// Node @type(s)
    types: Vec<String>,
    /// Properties (predicate -> values)
    properties: HashMap<String, Vec<JsonLdValue>>,
}

/// Internal representation of a JSON-LD value
#[derive(Debug, Clone)]
enum JsonLdValue {
    /// IRI reference
    Id(String),
    /// Literal value
    Literal {
        value: String,
        datatype: Option<String>,
        language: Option<String>,
    },
}

impl Default for JsonLdSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLdSerializer {
    /// Create a new JSON-LD serializer
    pub fn new() -> Self {
        Self {
            prefixes: HashMap::new(),
            use_context: true,
            nodes: HashMap::new(),
            named_graphs: HashMap::new(),
            pretty: true,
        }
    }

    /// Create a JSON-LD serializer with expanded form (no @context)
    pub fn expanded() -> Self {
        Self {
            prefixes: HashMap::new(),
            use_context: false,
            nodes: HashMap::new(),
            named_graphs: HashMap::new(),
            pretty: true,
        }
    }

    /// Set whether to use pretty printing
    pub fn pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }

    /// Add a namespace prefix for the @context
    pub fn add_prefix(&mut self, prefix: &str, iri: &str) {
        self.prefixes.insert(prefix.to_string(), iri.to_string());
    }

    /// Get or create a node for a subject
    fn get_or_create_node(
        &mut self,
        subject: &Subject,
        graph: Option<&GraphName>,
    ) -> &mut JsonLdNode {
        let subject_id = Self::subject_to_id(subject);

        let nodes = if let Some(g) = graph {
            let graph_id = Self::graph_name_to_id(g);
            self.named_graphs.entry(graph_id).or_default()
        } else {
            &mut self.nodes
        };

        nodes
            .entry(subject_id.clone())
            .or_insert_with(|| JsonLdNode {
                id: Some(subject_id),
                types: Vec::new(),
                properties: HashMap::new(),
            })
    }

    /// Convert subject to string ID
    fn subject_to_id(subject: &Subject) -> String {
        match subject {
            Subject::Iri(iri) => iri.as_str().to_string(),
            Subject::BlankNode(bn) => format!("_:{}", bn.label()),
        }
    }

    /// Convert graph name to string ID
    fn graph_name_to_id(graph: &GraphName) -> String {
        match graph {
            GraphName::Iri(iri) => iri.as_str().to_string(),
            GraphName::BlankNode(bn) => format!("_:{}", bn.label()),
        }
    }

    /// Convert object to JSON-LD value
    fn object_to_value(object: &Object) -> JsonLdValue {
        match object {
            Object::Iri(iri) => JsonLdValue::Id(iri.as_str().to_string()),
            Object::BlankNode(bn) => JsonLdValue::Id(format!("_:{}", bn.label())),
            Object::Literal(lit) => JsonLdValue::Literal {
                value: lit.value().to_string(),
                datatype: lit.explicit_datatype().map(|d| d.as_str().to_string()),
                language: lit.language().map(|l| l.to_string()),
            },
        }
    }

    /// Try to compact an IRI using prefixes
    fn compact_iri(&self, iri: &str) -> String {
        if !self.use_context {
            return iri.to_string();
        }

        for (prefix, namespace) in &self.prefixes {
            if iri.starts_with(namespace) {
                let local = &iri[namespace.len()..];
                if super::traits::is_valid_local_name(local) {
                    return format!("{}:{}", prefix, local);
                }
            }
        }
        iri.to_string()
    }

    /// Write the JSON output
    pub fn write_output<W: Write>(&self, writer: &mut W) -> SerializerResult<()> {
        let indent = if self.pretty { "  " } else { "" };
        let newline = if self.pretty { "\n" } else { "" };

        write!(writer, "{{")?;

        // Write @context if using compact form
        if self.use_context && !self.prefixes.is_empty() {
            write!(writer, "{}{}\"@context\": {{", newline, indent)?;

            let mut first = true;
            let mut prefixes: Vec<_> = self.prefixes.iter().collect();
            prefixes.sort_by_key(|(k, _)| k.as_str());

            for (prefix, iri) in prefixes {
                if !first {
                    write!(writer, ",")?;
                }
                write!(writer, "{}{}{}\"{}\":", newline, indent, indent, prefix)?;
                write!(writer, " \"{}\"", iri)?;
                first = false;
            }

            write!(writer, "{}{}}}", newline, indent)?;
        }

        // Write nodes
        let has_named_graphs = !self.named_graphs.is_empty();

        if has_named_graphs {
            // Write as @graph array
            if self.use_context && !self.prefixes.is_empty() {
                write!(writer, ",")?;
            }
            write!(writer, "{}{}\"@graph\": [", newline, indent)?;

            // Write default graph nodes
            self.write_nodes_array(&self.nodes, writer, indent, newline, true)?;

            // Write named graphs
            for (graph_id, nodes) in &self.named_graphs {
                write!(writer, ",")?;
                write!(writer, "{}{}{{", newline, indent)?;
                write!(
                    writer,
                    "{}{}{}\"@id\": \"{}\",",
                    newline, indent, indent, graph_id
                )?;
                write!(writer, "{}{}{}\"@graph\": [", newline, indent, indent)?;
                self.write_nodes_array(
                    nodes,
                    writer,
                    &format!("{}{}", indent, indent),
                    newline,
                    true,
                )?;
                write!(writer, "{}{}{}]", newline, indent, indent)?;
                write!(writer, "{}{}}}", newline, indent)?;
            }

            write!(writer, "{}{}]", newline, indent)?;
        } else if !self.nodes.is_empty() {
            // Single graph - write nodes directly or as @graph
            if self.nodes.len() == 1 {
                // Single node - merge with root
                let node = self.nodes.values().next().unwrap();
                if self.use_context && !self.prefixes.is_empty() {
                    write!(writer, ",")?;
                }
                // Always include @id for single nodes
                self.write_node_properties(node, writer, indent, newline, true)?;
            } else {
                // Multiple nodes - use @graph
                if self.use_context && !self.prefixes.is_empty() {
                    write!(writer, ",")?;
                }
                write!(writer, "{}{}\"@graph\": [", newline, indent)?;
                self.write_nodes_array(&self.nodes, writer, indent, newline, true)?;
                write!(writer, "{}{}]", newline, indent)?;
            }
        }

        write!(writer, "{}}}{}", newline, newline)?;
        Ok(())
    }

    /// Write an array of nodes
    ///
    /// # Arguments
    /// * `skip_first_separator` - If true, skip the comma before the first node (the array opening bracket was just written)
    fn write_nodes_array<W: Write>(
        &self,
        nodes: &HashMap<String, JsonLdNode>,
        writer: &mut W,
        indent: &str,
        newline: &str,
        skip_first_separator: bool,
    ) -> SerializerResult<()> {
        let mut is_first = skip_first_separator;

        let mut sorted_nodes: Vec<_> = nodes.iter().collect();
        sorted_nodes.sort_by_key(|(k, _)| k.as_str());

        for (_, node) in sorted_nodes {
            if !is_first {
                write!(writer, ",")?;
            }
            write!(writer, "{}{}{{", newline, indent)?;
            self.write_node_properties(node, writer, indent, newline, true)?;
            write!(writer, "{}{}}}", newline, indent)?;
            is_first = false;
        }

        Ok(())
    }

    /// Write node properties
    fn write_node_properties<W: Write>(
        &self,
        node: &JsonLdNode,
        writer: &mut W,
        indent: &str,
        newline: &str,
        include_id: bool,
    ) -> SerializerResult<()> {
        let inner_indent = format!("{}{}", indent, if self.pretty { "  " } else { "" });
        let mut first = true;

        // Write @id
        if include_id {
            if let Some(ref id) = node.id {
                write!(
                    writer,
                    "{}{}\"@id\": \"{}\"",
                    newline,
                    inner_indent,
                    escape_json_string(id)
                )?;
                first = false;
            }
        }

        // Write @type
        if !node.types.is_empty() {
            if !first {
                write!(writer, ",")?;
            }
            write!(writer, "{}{}\"@type\": ", newline, inner_indent)?;
            if node.types.len() == 1 {
                write!(writer, "\"{}\"", self.compact_iri(&node.types[0]))?;
            } else {
                write!(writer, "[")?;
                for (i, t) in node.types.iter().enumerate() {
                    if i > 0 {
                        write!(writer, ", ")?;
                    }
                    write!(writer, "\"{}\"", self.compact_iri(t))?;
                }
                write!(writer, "]")?;
            }
            first = false;
        }

        // Write properties
        let mut sorted_props: Vec<_> = node.properties.iter().collect();
        sorted_props.sort_by_key(|(k, _)| k.as_str());

        for (predicate, values) in sorted_props {
            if !first {
                write!(writer, ",")?;
            }

            let key = self.compact_iri(predicate);
            write!(
                writer,
                "{}{}\"{}\":",
                newline,
                inner_indent,
                escape_json_string(&key)
            )?;

            if values.len() == 1 {
                write!(writer, " ")?;
                self.write_value(&values[0], writer)?;
            } else {
                write!(writer, " [")?;
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        write!(writer, ",")?;
                    }
                    write!(writer, " ")?;
                    self.write_value(value, writer)?;
                }
                write!(writer, " ]")?;
            }

            first = false;
        }

        Ok(())
    }

    /// Write a JSON-LD value
    fn write_value<W: Write>(&self, value: &JsonLdValue, writer: &mut W) -> SerializerResult<()> {
        match value {
            JsonLdValue::Id(id) => {
                let compacted = self.compact_iri(id);
                if compacted == *id {
                    write!(writer, "{{\"@id\": \"{}\"}}", escape_json_string(id))?;
                } else {
                    write!(
                        writer,
                        "{{\"@id\": \"{}\"}}",
                        escape_json_string(&compacted)
                    )?;
                }
            }
            JsonLdValue::Literal {
                value,
                datatype,
                language,
            } => {
                if language.is_some() || datatype.is_some() {
                    write!(writer, "{{\"@value\": \"{}\"", escape_json_string(value))?;
                    if let Some(lang) = language {
                        write!(writer, ", \"@language\": \"{}\"", lang)?;
                    } else if let Some(dt) = datatype {
                        if dt != "http://www.w3.org/2001/XMLSchema#string" {
                            write!(writer, ", \"@type\": \"{}\"", self.compact_iri(dt))?;
                        }
                    }
                    write!(writer, "}}")?;
                } else {
                    write!(writer, "\"{}\"", escape_json_string(value))?;
                }
            }
        }
        Ok(())
    }

    /// Clear accumulated data
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.named_graphs.clear();
    }
}

impl TripleSerializer for JsonLdSerializer {
    fn serialize_triple<W: Write>(
        &mut self,
        triple: &Triple,
        _writer: &mut W,
    ) -> SerializerResult<()> {
        let node = self.get_or_create_node(&triple.subject, None);

        // Handle rdf:type specially
        if triple.predicate.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" {
            if let Object::Iri(type_iri) = &triple.object {
                node.types.push(type_iri.as_str().to_string());
                return Ok(());
            }
        }

        let predicate = triple.predicate.as_str().to_string();
        let value = Self::object_to_value(&triple.object);

        node.properties.entry(predicate).or_default().push(value);

        Ok(())
    }

    fn finish<W: Write>(&mut self, writer: &mut W) -> SerializerResult<()> {
        self.write_output(writer)
    }
}

impl QuadSerializer for JsonLdSerializer {
    fn serialize_quad<W: Write>(&mut self, quad: &Quad, _writer: &mut W) -> SerializerResult<()> {
        let node = self.get_or_create_node(&quad.triple.subject, quad.graph.as_ref());

        // Handle rdf:type specially
        if quad.triple.predicate.as_str() == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" {
            if let Object::Iri(type_iri) = &quad.triple.object {
                node.types.push(type_iri.as_str().to_string());
                return Ok(());
            }
        }

        let predicate = quad.triple.predicate.as_str().to_string();
        let value = Self::object_to_value(&quad.triple.object);

        node.properties.entry(predicate).or_default().push(value);

        Ok(())
    }

    fn finish<W: Write>(&mut self, writer: &mut W) -> SerializerResult<()> {
        self.write_output(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdf::{Iri, Literal};

    fn test_iri(s: &str) -> Iri {
        Iri::new(s).unwrap()
    }

    #[test]
    fn test_simple_triple() {
        let mut serializer = JsonLdSerializer::new();
        serializer.use_context = false;

        let triple = Triple::new(
            test_iri("http://example.org/person/1"),
            test_iri("http://example.org/name"),
            Literal::new("John"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.write_output(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"@id\""));
        assert!(result.contains("http://example.org/person/1"));
        assert!(result.contains("\"John\""));
    }

    #[test]
    fn test_with_context() {
        let mut serializer = JsonLdSerializer::new();
        serializer.add_prefix("ex", "http://example.org/");

        let triple = Triple::new(
            test_iri("http://example.org/person/1"),
            test_iri("http://example.org/name"),
            Literal::new("John"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.write_output(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"@context\""));
        assert!(result.contains("\"ex\":"));
    }

    #[test]
    fn test_rdf_type() {
        let mut serializer = JsonLdSerializer::new();
        serializer.use_context = false;

        let triple = Triple::new(
            test_iri("http://example.org/person/1"),
            test_iri("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            test_iri("http://example.org/Person"),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.write_output(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"@type\""));
        assert!(result.contains("http://example.org/Person"));
    }

    #[test]
    fn test_named_graph() {
        let mut serializer = JsonLdSerializer::new();
        serializer.use_context = false;

        let quad = Quad::in_named_graph(
            Triple::new(
                test_iri("http://example.org/s"),
                test_iri("http://example.org/p"),
                Literal::new("value"),
            ),
            test_iri("http://example.org/graph1"),
        );

        let mut output = Vec::new();
        serializer.serialize_quad(&quad, &mut output).unwrap();
        serializer.write_output(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"@graph\""));
        assert!(result.contains("http://example.org/graph1"));
    }

    #[test]
    fn test_language_tag() {
        let mut serializer = JsonLdSerializer::new();
        serializer.use_context = false;

        let triple = Triple::new(
            test_iri("http://example.org/thing"),
            test_iri("http://example.org/label"),
            Literal::with_language("Bonjour", "fr").unwrap(),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.write_output(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"@value\""));
        assert!(result.contains("\"@language\""));
        assert!(result.contains("\"fr\""));
    }

    #[test]
    fn test_typed_literal() {
        let mut serializer = JsonLdSerializer::new();
        serializer.use_context = false;

        let triple = Triple::new(
            test_iri("http://example.org/thing"),
            test_iri("http://example.org/count"),
            Literal::integer(42),
        );

        let mut output = Vec::new();
        serializer.serialize_triple(&triple, &mut output).unwrap();
        serializer.write_output(&mut output).unwrap();

        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("\"@value\""));
        assert!(result.contains("\"@type\""));
        assert!(result.contains("integer"));
    }
}
