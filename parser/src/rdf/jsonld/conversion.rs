//! Conversion from JSON-LD expanded form to RDF triples

use std::collections::HashMap;

use serde_json::Value;

use super::error::{JsonLdError, JsonLdResult};
use crate::rdf::{BlankNode, BlankNodeScope, Iri, Literal, Object, Quad, Subject, Triple};

/// Converts JSON-LD expanded form to RDF quads/triples
pub struct JsonLdToRdf {
    /// Blank node scope for generating unique blank node IDs
    blank_node_scope: BlankNodeScope,
    /// Mapping from JSON-LD blank node identifiers to our blank nodes
    blank_node_map: HashMap<String, BlankNode>,
}

impl JsonLdToRdf {
    /// Create a new converter
    #[must_use]
    pub fn new() -> Self {
        Self {
            blank_node_scope: BlankNodeScope::generate(),
            blank_node_map: HashMap::new(),
        }
    }

    /// Convert expanded JSON-LD to RDF quads
    pub fn convert(&mut self, expanded: &Value) -> JsonLdResult<Vec<Quad>> {
        let mut quads = Vec::new();

        match expanded {
            Value::Array(arr) => {
                for item in arr {
                    self.process_node(item, &mut quads)?;
                }
            }
            Value::Object(_) => {
                self.process_node(expanded, &mut quads)?;
            }
            _ => {
                return Err(JsonLdError::rdf_conversion(
                    "Expanded JSON-LD must be an array or object",
                ));
            }
        }

        Ok(quads)
    }

    /// Convert expanded JSON-LD to RDF triples (ignoring graph names)
    pub fn convert_to_triples(&mut self, expanded: &Value) -> JsonLdResult<Vec<Triple>> {
        let quads = self.convert(expanded)?;
        Ok(quads.into_iter().map(|q| q.triple).collect())
    }

    /// Process a node object and generate quads
    fn process_node(
        &mut self,
        node: &Value,
        quads: &mut Vec<Quad>,
    ) -> JsonLdResult<Option<Subject>> {
        let obj = match node.as_object() {
            Some(o) => o,
            None => return Ok(None),
        };

        // Check if this is a value object
        if obj.contains_key("@value") {
            return Ok(None);
        }

        // Check if this is a list object
        if obj.contains_key("@list") {
            return Ok(None);
        }

        // Get or generate subject
        let subject = self.get_subject(obj)?;

        // Process @type
        if let Some(types) = obj.get("@type") {
            let rdf_type = Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
                .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;

            match types {
                Value::Array(arr) => {
                    for type_val in arr {
                        if let Some(type_iri) = type_val.as_str() {
                            let type_iri = Iri::new(type_iri)
                                .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
                            let triple = Triple::new(
                                subject.clone(),
                                rdf_type.clone(),
                                Object::Iri(type_iri),
                            );
                            quads.push(Quad::in_default_graph(triple));
                        }
                    }
                }
                Value::String(type_iri) => {
                    let type_iri = Iri::new(type_iri)
                        .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
                    let triple = Triple::new(subject.clone(), rdf_type, Object::Iri(type_iri));
                    quads.push(Quad::in_default_graph(triple));
                }
                _ => {}
            }
        }

        // Process other properties
        for (key, value) in obj {
            // Skip JSON-LD keywords
            if key.starts_with('@') {
                continue;
            }

            let predicate = Iri::new(key).map_err(|e| {
                JsonLdError::rdf_conversion(format!("Invalid predicate IRI '{key}': {e}"))
            })?;

            self.process_property(&subject, &predicate, value, quads)?;
        }

        Ok(Some(subject))
    }

    /// Get or create a subject from a node object
    fn get_subject(&mut self, obj: &serde_json::Map<String, Value>) -> JsonLdResult<Subject> {
        if let Some(id) = obj.get("@id") {
            if let Some(id_str) = id.as_str() {
                if let Some(stripped) = id_str.strip_prefix("_:") {
                    // Blank node
                    let bn = self.get_or_create_blank_node(stripped);
                    Ok(Subject::BlankNode(bn))
                } else {
                    // IRI
                    let iri =
                        Iri::new(id_str).map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
                    Ok(Subject::Iri(iri))
                }
            } else {
                Err(JsonLdError::rdf_conversion("@id must be a string"))
            }
        } else {
            // Generate a blank node
            let bn = self.blank_node_scope.next();
            Ok(Subject::BlankNode(bn))
        }
    }

    /// Process a property and its values
    fn process_property(
        &mut self,
        subject: &Subject,
        predicate: &Iri,
        value: &Value,
        quads: &mut Vec<Quad>,
    ) -> JsonLdResult<()> {
        match value {
            Value::Array(arr) => {
                for item in arr {
                    self.process_value(subject, predicate, item, quads)?;
                }
            }
            _ => {
                self.process_value(subject, predicate, value, quads)?;
            }
        }
        Ok(())
    }

    /// Process a single value and generate a quad
    fn process_value(
        &mut self,
        subject: &Subject,
        predicate: &Iri,
        value: &Value,
        quads: &mut Vec<Quad>,
    ) -> JsonLdResult<()> {
        let object = self.value_to_object(value, quads)?;

        if let Some(obj) = object {
            let triple = Triple::new(subject.clone(), predicate.clone(), obj);
            quads.push(Quad::in_default_graph(triple));
        }

        Ok(())
    }

    /// Convert a JSON-LD value to an RDF object
    fn value_to_object(
        &mut self,
        value: &Value,
        quads: &mut Vec<Quad>,
    ) -> JsonLdResult<Option<Object>> {
        match value {
            Value::Object(obj) => {
                if obj.contains_key("@value") {
                    // Value object
                    self.value_object_to_literal(obj)
                } else if let Some(id) = obj.get("@id") {
                    // Node reference
                    if let Some(id_str) = id.as_str() {
                        if let Some(stripped) = id_str.strip_prefix("_:") {
                            let bn = self.get_or_create_blank_node(stripped);
                            Ok(Some(Object::BlankNode(bn)))
                        } else {
                            let iri = Iri::new(id_str)
                                .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
                            Ok(Some(Object::Iri(iri)))
                        }
                    } else {
                        Err(JsonLdError::rdf_conversion("@id must be a string"))
                    }
                } else if let Some(list) = obj.get("@list") {
                    // List object - convert to RDF list
                    self.list_to_rdf(list, quads)
                } else {
                    // Nested node object
                    if let Some(nested_subject) = self.process_node(value, quads)? {
                        match nested_subject {
                            Subject::Iri(iri) => Ok(Some(Object::Iri(iri))),
                            Subject::BlankNode(bn) => Ok(Some(Object::BlankNode(bn))),
                        }
                    } else {
                        Ok(None)
                    }
                }
            }
            Value::String(s) => {
                // Plain string literal
                Ok(Some(Object::Literal(Literal::new(s))))
            }
            Value::Number(n) => {
                // Numeric literal
                if n.is_f64() {
                    let lit = Literal::double(n.as_f64().unwrap());
                    Ok(Some(Object::Literal(lit)))
                } else if n.is_i64() {
                    let lit = Literal::integer(n.as_i64().unwrap());
                    Ok(Some(Object::Literal(lit)))
                } else {
                    let lit = Literal::integer(n.as_u64().unwrap() as i64);
                    Ok(Some(Object::Literal(lit)))
                }
            }
            Value::Bool(b) => {
                let lit = Literal::boolean(*b);
                Ok(Some(Object::Literal(lit)))
            }
            Value::Null => Ok(None),
            Value::Array(_) => {
                // Arrays should be handled at the property level
                Err(JsonLdError::rdf_conversion(
                    "Unexpected array in value position",
                ))
            }
        }
    }

    /// Convert a value object to an RDF literal
    fn value_object_to_literal(
        &self,
        obj: &serde_json::Map<String, Value>,
    ) -> JsonLdResult<Option<Object>> {
        let value = obj
            .get("@value")
            .ok_or_else(|| JsonLdError::rdf_conversion("Value object missing @value"))?;

        let value_str = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => {
                return Err(JsonLdError::rdf_conversion(
                    "@value must be a string, number, or boolean",
                ))
            }
        };

        // Check for language tag
        if let Some(lang) = obj.get("@language") {
            if let Some(lang_str) = lang.as_str() {
                let lit = Literal::with_language(&value_str, lang_str)
                    .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
                return Ok(Some(Object::Literal(lit)));
            }
        }

        // Check for datatype
        if let Some(datatype) = obj.get("@type") {
            if let Some(dt_str) = datatype.as_str() {
                let dt_iri =
                    Iri::new(dt_str).map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
                let lit = Literal::with_datatype(&value_str, dt_iri);
                return Ok(Some(Object::Literal(lit)));
            }
        }

        // Plain literal with inferred datatype
        let lit = match value {
            Value::Number(n) if n.is_f64() => Literal::double(n.as_f64().unwrap()),
            Value::Number(n) if n.is_i64() => Literal::integer(n.as_i64().unwrap()),
            Value::Number(n) => Literal::integer(n.as_u64().unwrap() as i64),
            Value::Bool(b) => Literal::boolean(*b),
            _ => Literal::new(&value_str),
        };

        Ok(Some(Object::Literal(lit)))
    }

    /// Convert a @list to RDF list structure
    fn list_to_rdf(&mut self, list: &Value, quads: &mut Vec<Quad>) -> JsonLdResult<Option<Object>> {
        let items = match list {
            Value::Array(arr) => arr,
            _ => return Err(JsonLdError::rdf_conversion("@list must be an array")),
        };

        if items.is_empty() {
            // Empty list is rdf:nil
            let nil = Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil")
                .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
            return Ok(Some(Object::Iri(nil)));
        }

        let rdf_first = Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#first")
            .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
        let rdf_rest = Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest")
            .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;
        let rdf_nil = Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil")
            .map_err(|e| JsonLdError::rdf_conversion(e.to_string()))?;

        let mut list_head: Option<BlankNode> = None;
        let mut prev_node: Option<BlankNode> = None;

        for item in items {
            let current_node = self.blank_node_scope.next();

            if list_head.is_none() {
                list_head = Some(current_node.clone());
            }

            // Link previous node to current
            if let Some(prev) = prev_node {
                let triple = Triple::new(
                    Subject::BlankNode(prev),
                    rdf_rest.clone(),
                    Object::BlankNode(current_node.clone()),
                );
                quads.push(Quad::in_default_graph(triple));
            }

            // Add first element
            if let Some(obj) = self.value_to_object(item, quads)? {
                let triple = Triple::new(
                    Subject::BlankNode(current_node.clone()),
                    rdf_first.clone(),
                    obj,
                );
                quads.push(Quad::in_default_graph(triple));
            }

            prev_node = Some(current_node);
        }

        // Terminate list with rdf:nil
        if let Some(last) = prev_node {
            let triple = Triple::new(Subject::BlankNode(last), rdf_rest, Object::Iri(rdf_nil));
            quads.push(Quad::in_default_graph(triple));
        }

        Ok(list_head.map(Object::BlankNode))
    }

    /// Get or create a blank node for a given identifier
    fn get_or_create_blank_node(&mut self, id: &str) -> BlankNode {
        if let Some(bn) = self.blank_node_map.get(id) {
            bn.clone()
        } else {
            let bn = self.blank_node_scope.map(id);
            self.blank_node_map.insert(id.to_string(), bn.clone());
            bn
        }
    }
}

impl Default for JsonLdToRdf {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_node() {
        let mut converter = JsonLdToRdf::new();
        let expanded = json!([{
            "@id": "http://example.org/person/1",
            "http://schema.org/name": [{"@value": "John Doe"}],
            "@type": ["http://schema.org/Person"]
        }]);

        let quads = converter.convert(&expanded).unwrap();
        assert_eq!(quads.len(), 2); // type + name

        // Check that we have the expected triples
        let subjects: Vec<_> = quads.iter().map(|q| q.subject().to_string()).collect();
        assert!(subjects.iter().all(|s| s.contains("example.org/person/1")));
    }

    #[test]
    fn test_blank_node() {
        let mut converter = JsonLdToRdf::new();
        let expanded = json!([{
            "@id": "_:b0",
            "http://schema.org/name": [{"@value": "Anonymous"}]
        }]);

        let quads = converter.convert(&expanded).unwrap();
        assert_eq!(quads.len(), 1);
        assert!(quads[0].subject().is_blank_node());
    }

    #[test]
    fn test_typed_literal() {
        let mut converter = JsonLdToRdf::new();
        let expanded = json!([{
            "@id": "http://example.org/thing",
            "http://example.org/count": [{
                "@value": "42",
                "@type": "http://www.w3.org/2001/XMLSchema#integer"
            }]
        }]);

        let quads = converter.convert(&expanded).unwrap();
        assert_eq!(quads.len(), 1);

        if let Object::Literal(lit) = quads[0].object() {
            assert_eq!(lit.value(), "42");
            assert!(lit.datatype().as_str().contains("integer"));
        } else {
            panic!("Expected literal object");
        }
    }

    #[test]
    fn test_language_tagged_literal() {
        let mut converter = JsonLdToRdf::new();
        let expanded = json!([{
            "@id": "http://example.org/thing",
            "http://example.org/label": [{
                "@value": "Bonjour",
                "@language": "fr"
            }]
        }]);

        let quads = converter.convert(&expanded).unwrap();
        assert_eq!(quads.len(), 1);

        if let Object::Literal(lit) = quads[0].object() {
            assert_eq!(lit.value(), "Bonjour");
            assert_eq!(lit.language(), Some("fr"));
        } else {
            panic!("Expected literal object");
        }
    }

    #[test]
    fn test_nested_node() {
        let mut converter = JsonLdToRdf::new();
        let expanded = json!([{
            "@id": "http://example.org/person/1",
            "http://schema.org/knows": [{
                "@id": "http://example.org/person/2",
                "http://schema.org/name": [{"@value": "Jane"}]
            }]
        }]);

        let quads = converter.convert(&expanded).unwrap();
        // Should have at least: knows relation
        // The nested node (person/2) and its name property should also be processed
        assert!(!quads.is_empty());

        // Check that we have a knows relation
        let has_knows = quads
            .iter()
            .any(|q| q.predicate().as_str().contains("knows"));
        assert!(has_knows, "Expected a 'knows' relation");
    }

    #[test]
    fn test_list() {
        let mut converter = JsonLdToRdf::new();
        let expanded = json!([{
            "@id": "http://example.org/thing",
            "http://example.org/items": [{
                "@list": [
                    {"@value": "a"},
                    {"@value": "b"},
                    {"@value": "c"}
                ]
            }]
        }]);

        let quads = converter.convert(&expanded).unwrap();
        // Should have: items relation + list structure (3 first + 3 rest + nil)
        assert!(quads.len() >= 7);
    }
}
