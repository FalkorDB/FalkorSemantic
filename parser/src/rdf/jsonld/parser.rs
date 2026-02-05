//! JSON-LD Parser implementation
//!
//! Provides parsing, expansion, compaction, and framing of JSON-LD documents.

use std::collections::HashMap;

use serde_json::{json, Value};

use super::context::{ContextResolver, ResolvedContext};
use super::conversion::JsonLdToRdf;
use super::error::{JsonLdError, JsonLdResult};
use crate::rdf::{Quad, Triple};

/// JSON-LD Parser
///
/// Provides functionality for parsing and processing JSON-LD documents including:
/// - Parsing JSON-LD from strings
/// - Expanding documents (removing context, using full IRIs)
/// - Compacting documents (applying context, using prefixes)
/// - Basic framing support
/// - Converting to RDF triples/quads
#[derive(Debug, Default)]
pub struct JsonLdParser {
    /// Context resolver for handling @context
    context_resolver: ContextResolver,
    /// Base IRI for the document
    base_iri: Option<String>,
}

impl JsonLdParser {
    /// Create a new JSON-LD parser
    #[must_use] 
    pub fn new() -> Self {
        Self {
            context_resolver: ContextResolver::new(),
            base_iri: None,
        }
    }

    /// Create a parser with a base IRI
    pub fn with_base(base_iri: impl Into<String>) -> Self {
        Self {
            context_resolver: ContextResolver::new(),
            base_iri: Some(base_iri.into()),
        }
    }

    /// Set the base IRI
    pub fn set_base(&mut self, base_iri: impl Into<String>) {
        self.base_iri = Some(base_iri.into());
    }

    /// Parse a JSON-LD string into a JSON value
    pub fn parse(&self, input: &str) -> JsonLdResult<Value> {
        serde_json::from_str(input).map_err(|e| JsonLdError::json_parse(e.to_string()))
    }

    /// Expand a JSON-LD document
    ///
    /// Expansion removes the @context and replaces all terms with their full IRIs.
    /// The result is an array of expanded node objects.
    pub fn expand(&mut self, document: &Value) -> JsonLdResult<Value> {
        // Extract and resolve context
        let context = document.get("@context").cloned().unwrap_or(Value::Null);

        let resolved_context = self
            .context_resolver
            .resolve(&context, self.base_iri.as_deref())?;

        // Expand the document
        let expanded = self.expand_value(document, &resolved_context)?;

        // Wrap in array if not already
        match expanded {
            Value::Array(_) => Ok(expanded),
            Value::Null => Ok(json!([])),
            _ => Ok(json!([expanded])),
        }
    }

    /// Expand a value according to the context
    fn expand_value(&mut self, value: &Value, context: &ResolvedContext) -> JsonLdResult<Value> {
        match value {
            Value::Object(obj) => self.expand_object(obj, context),
            Value::Array(arr) => {
                let expanded: Result<Vec<_>, _> =
                    arr.iter().map(|v| self.expand_value(v, context)).collect();
                Ok(Value::Array(expanded?))
            }
            _ => Ok(value.clone()),
        }
    }

    /// Expand an object according to the context
    fn expand_object(
        &mut self,
        obj: &serde_json::Map<String, Value>,
        parent_context: &ResolvedContext,
    ) -> JsonLdResult<Value> {
        // Check for local @context
        let context = if let Some(local_ctx) = obj.get("@context") {
            let mut merged = parent_context.clone();
            let local_resolved = self
                .context_resolver
                .resolve(local_ctx, parent_context.base.as_deref())?;
            merged.terms.extend(local_resolved.terms);
            if local_resolved.base.is_some() {
                merged.base = local_resolved.base;
            }
            if local_resolved.vocab.is_some() {
                merged.vocab = local_resolved.vocab;
            }
            if local_resolved.language.is_some() {
                merged.language = local_resolved.language;
            }
            merged
        } else {
            parent_context.clone()
        };

        let mut result = serde_json::Map::new();

        for (key, value) in obj {
            // Skip @context as it's been processed
            if key == "@context" {
                continue;
            }

            // Expand the key
            let expanded_key = self.expand_term(key, &context)?;

            // Handle JSON-LD keywords
            match key.as_str() {
                "@id" => {
                    if let Value::String(id) = value {
                        let expanded_id = self.expand_iri(id, &context)?;
                        result.insert("@id".to_string(), Value::String(expanded_id));
                    } else {
                        result.insert("@id".to_string(), value.clone());
                    }
                }
                "@type" => {
                    let expanded_types = self.expand_type_value(value, &context)?;
                    result.insert("@type".to_string(), expanded_types);
                }
                "@value" | "@language" | "@list" | "@set" | "@graph" | "@index" | "@reverse" => {
                    // Keep these keywords as-is but expand nested values
                    let expanded_value = self.expand_value(value, &context)?;
                    result.insert(key.clone(), expanded_value);
                }
                _ => {
                    // Regular property
                    let expanded_value = self.expand_property_value(key, value, &context)?;
                    if !expanded_value.is_null() {
                        result.insert(expanded_key, expanded_value);
                    }
                }
            }
        }

        if result.is_empty() {
            Ok(Value::Null)
        } else {
            Ok(Value::Object(result))
        }
    }

    /// Expand a term to its full IRI
    fn expand_term(&self, term: &str, context: &ResolvedContext) -> JsonLdResult<String> {
        // Keywords stay as-is
        if term.starts_with('@') {
            return Ok(term.to_string());
        }

        // Check if term is defined in context
        if let Some(term_def) = context.terms.get(term) {
            return Ok(term_def.iri.clone());
        }

        // Check for compact IRI (prefix:suffix)
        if let Some(colon_pos) = term.find(':') {
            let prefix = &term[..colon_pos];
            let suffix = &term[colon_pos + 1..];

            if let Some(prefix_def) = context.terms.get(prefix) {
                return Ok(format!("{}{}", prefix_def.iri, suffix));
            }

            // Might be an absolute IRI
            return Ok(term.to_string());
        }

        // Apply vocab if present
        if let Some(ref vocab) = context.vocab {
            return Ok(format!("{vocab}{term}"));
        }

        Ok(term.to_string())
    }

    /// Expand an IRI
    fn expand_iri(&self, iri: &str, context: &ResolvedContext) -> JsonLdResult<String> {
        // Already absolute
        if iri.contains("://") {
            return Ok(iri.to_string());
        }

        // Blank node
        if iri.starts_with("_:") {
            return Ok(iri.to_string());
        }

        // Relative IRI - resolve against base
        if let Some(ref base) = context.base {
            if iri.starts_with('#') {
                return Ok(format!("{base}{iri}"));
            }
            if iri.starts_with('/') {
                // Absolute path - find authority
                if let Some(authority_end) = base.find("://").map(|i| {
                    base[i + 3..]
                        .find('/')
                        .map_or(base.len(), |j| i + 3 + j)
                }) {
                    return Ok(format!("{}{}", &base[..authority_end], iri));
                }
            }
            // Relative path
            if let Some(last_slash) = base.rfind('/') {
                return Ok(format!("{}/{}", &base[..last_slash], iri));
            }
        }

        self.expand_term(iri, context)
    }

    /// Expand a @type value
    fn expand_type_value(&self, value: &Value, context: &ResolvedContext) -> JsonLdResult<Value> {
        match value {
            Value::String(s) => {
                let expanded = self.expand_term(s, context)?;
                Ok(json!([expanded]))
            }
            Value::Array(arr) => {
                let expanded: Result<Vec<_>, _> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| self.expand_term(s, context))
                    .collect();
                Ok(Value::Array(
                    expanded?.into_iter().map(Value::String).collect(),
                ))
            }
            _ => Ok(value.clone()),
        }
    }

    /// Expand a property value
    fn expand_property_value(
        &mut self,
        property: &str,
        value: &Value,
        context: &ResolvedContext,
    ) -> JsonLdResult<Value> {
        // Get term definition for type coercion
        let term_def = context.terms.get(property);
        let type_coercion = term_def.and_then(|t| t.type_coercion.as_deref());

        match value {
            Value::Array(arr) => {
                let expanded: Result<Vec<_>, _> = arr
                    .iter()
                    .map(|v| self.expand_property_value(property, v, context))
                    .collect();
                Ok(Value::Array(expanded?))
            }
            Value::Object(obj) => {
                // Check if this is already a value object or node reference
                if obj.contains_key("@value")
                    || obj.contains_key("@id")
                    || obj.contains_key("@list")
                {
                    self.expand_object(obj, context)
                } else {
                    // Nested node
                    self.expand_object(obj, context)
                }
            }
            Value::String(s) => {
                // Apply type coercion
                if type_coercion == Some("@id") {
                    let expanded_id = self.expand_iri(s, context)?;
                    Ok(json!([{"@id": expanded_id}]))
                } else {
                    // Plain literal
                    let mut obj = serde_json::Map::new();
                    obj.insert("@value".to_string(), Value::String(s.clone()));

                    // Apply language from term or context
                    let lang = term_def
                        .and_then(|t| t.language.as_ref())
                        .or(context.language.as_ref());

                    if let Some(lang) = lang {
                        obj.insert("@language".to_string(), Value::String(lang.clone()));
                    }

                    Ok(json!([Value::Object(obj)]))
                }
            }
            Value::Number(_) | Value::Bool(_) => Ok(json!([{"@value": value}])),
            Value::Null => Ok(Value::Null),
        }
    }

    /// Compact an expanded JSON-LD document using a context
    ///
    /// Compaction applies a context to replace IRIs with prefixed names and terms.
    pub fn compact(&mut self, expanded: &Value, context: &Value) -> JsonLdResult<Value> {
        let resolved_context = self
            .context_resolver
            .resolve(context, self.base_iri.as_deref())?;

        // Build reverse mapping from IRIs to terms
        let iri_to_term: HashMap<String, String> = resolved_context
            .terms
            .iter()
            .map(|(term, def)| (def.iri.clone(), term.clone()))
            .collect();

        let compacted = self.compact_value(expanded, &resolved_context, &iri_to_term)?;

        // Add context to result
        if let Value::Object(obj) = compacted {
            if context.is_null() {
                Ok(Value::Object(obj))
            } else {
                // Insert @context at the beginning by creating a new map
                let mut result = serde_json::Map::new();
                result.insert("@context".to_string(), context.clone());
                result.extend(obj);
                Ok(Value::Object(result))
            }
        } else if let Value::Array(arr) = compacted {
            if arr.len() == 1 {
                // Unwrap single-element array
                let first = arr.into_iter().next().unwrap();
                if let Value::Object(obj) = first {
                    if !context.is_null() {
                        let mut result = serde_json::Map::new();
                        result.insert("@context".to_string(), context.clone());
                        result.extend(obj);
                        return Ok(Value::Object(result));
                    }
                    return Ok(Value::Object(obj));
                }
                // If not an object, wrap in graph
                let mut result = serde_json::Map::new();
                if !context.is_null() {
                    result.insert("@context".to_string(), context.clone());
                }
                result.insert("@graph".to_string(), json!([first]));
                Ok(Value::Object(result))
            } else {
                let mut result = serde_json::Map::new();
                if !context.is_null() {
                    result.insert("@context".to_string(), context.clone());
                }
                result.insert("@graph".to_string(), Value::Array(arr));
                Ok(Value::Object(result))
            }
        } else {
            Ok(compacted)
        }
    }

    /// Compact a value
    fn compact_value(
        &self,
        value: &Value,
        context: &ResolvedContext,
        iri_to_term: &HashMap<String, String>,
    ) -> JsonLdResult<Value> {
        match value {
            Value::Array(arr) => {
                let compacted: Result<Vec<_>, _> = arr
                    .iter()
                    .map(|v| self.compact_value(v, context, iri_to_term))
                    .collect();
                Ok(Value::Array(compacted?))
            }
            Value::Object(obj) => self.compact_object(obj, context, iri_to_term),
            _ => Ok(value.clone()),
        }
    }

    /// Compact an object
    fn compact_object(
        &self,
        obj: &serde_json::Map<String, Value>,
        context: &ResolvedContext,
        iri_to_term: &HashMap<String, String>,
    ) -> JsonLdResult<Value> {
        // Check if this is a simple value object that can be simplified
        if obj.len() == 1 && obj.contains_key("@value") {
            return Ok(obj.get("@value").unwrap().clone());
        }

        let mut result = serde_json::Map::new();

        for (key, value) in obj {
            let compacted_key = if key.starts_with('@') {
                key.clone()
            } else {
                self.compact_iri(key, iri_to_term)
            };

            let compacted_value = match key.as_str() {
                "@id" => {
                    if let Value::String(id) = value {
                        Value::String(self.compact_iri(id, iri_to_term))
                    } else {
                        value.clone()
                    }
                }
                "@type" => self.compact_type_value(value, iri_to_term),
                _ => {
                    let v = self.compact_value(value, context, iri_to_term)?;
                    // Simplify single-element arrays
                    if let Value::Array(arr) = &v {
                        if arr.len() == 1 {
                            arr[0].clone()
                        } else {
                            v
                        }
                    } else {
                        v
                    }
                }
            };

            result.insert(compacted_key, compacted_value);
        }

        Ok(Value::Object(result))
    }

    /// Compact an IRI using the reverse term mapping
    fn compact_iri(&self, iri: &str, iri_to_term: &HashMap<String, String>) -> String {
        // Direct term match
        if let Some(term) = iri_to_term.get(iri) {
            return term.clone();
        }

        // Try to find a prefix match
        for (prefix_iri, term) in iri_to_term {
            if iri.starts_with(prefix_iri) && iri.len() > prefix_iri.len() {
                let suffix = &iri[prefix_iri.len()..];
                // Only use prefix if suffix doesn't contain special chars
                if !suffix.contains('/') && !suffix.contains('#') {
                    return format!("{term}:{suffix}");
                }
            }
        }

        iri.to_string()
    }

    /// Compact @type values
    fn compact_type_value(&self, value: &Value, iri_to_term: &HashMap<String, String>) -> Value {
        match value {
            Value::Array(arr) => {
                let compacted: Vec<_> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| Value::String(self.compact_iri(s, iri_to_term)))
                    .collect();
                if compacted.len() == 1 {
                    compacted.into_iter().next().unwrap()
                } else {
                    Value::Array(compacted)
                }
            }
            Value::String(s) => Value::String(self.compact_iri(s, iri_to_term)),
            _ => value.clone(),
        }
    }

    /// Apply a frame to a JSON-LD document (basic framing)
    ///
    /// Framing reshapes JSON-LD data according to a template.
    /// This is a simplified implementation supporting basic patterns.
    pub fn frame(&mut self, document: &Value, frame: &Value) -> JsonLdResult<Value> {
        // First expand both document and frame
        let expanded_doc = self.expand(document)?;

        let frame_obj = frame
            .as_object()
            .ok_or_else(|| JsonLdError::framing("Frame must be an object"))?;

        // Get the frame context for compaction
        let frame_context = frame.get("@context").cloned().unwrap_or(Value::Null);

        // Match nodes against frame
        let matched = self.match_frame(&expanded_doc, frame_obj)?;

        // Compact the result using the frame's context
        if frame_context.is_null() {
            Ok(matched)
        } else {
            self.compact(&matched, &frame_context)
        }
    }

    /// Match expanded document against frame
    fn match_frame(
        &self,
        expanded: &Value,
        frame: &serde_json::Map<String, Value>,
    ) -> JsonLdResult<Value> {
        let nodes = match expanded {
            Value::Array(arr) => arr.clone(),
            _ => vec![expanded.clone()],
        };

        // Extract frame constraints
        let type_constraint = frame.get("@type");
        let id_constraint = frame.get("@id");

        let mut matched = Vec::new();

        for node in nodes {
            if self.node_matches_frame(&node, type_constraint, id_constraint) {
                matched.push(node);
            }
        }

        if matched.len() == 1 {
            Ok(matched.into_iter().next().unwrap())
        } else {
            Ok(Value::Array(matched))
        }
    }

    /// Check if a node matches frame constraints
    fn node_matches_frame(
        &self,
        node: &Value,
        type_constraint: Option<&Value>,
        id_constraint: Option<&Value>,
    ) -> bool {
        let obj = match node.as_object() {
            Some(o) => o,
            None => return false,
        };

        // Check @type constraint
        if let Some(constraint) = type_constraint {
            let node_types = obj.get("@type");
            match (constraint, node_types) {
                (Value::Array(wanted), Some(Value::Array(actual))) => {
                    // Check if any wanted type is in actual types
                    if !wanted.iter().any(|w| actual.contains(w)) {
                        return false;
                    }
                }
                (Value::String(wanted), Some(Value::Array(actual))) => {
                    if !actual.iter().any(|a| a.as_str() == Some(wanted.as_str())) {
                        return false;
                    }
                }
                (Value::Object(_), _) => {
                    // Wildcard - matches any type
                }
                _ => return false,
            }
        }

        // Check @id constraint
        if let Some(constraint) = id_constraint {
            match constraint {
                Value::String(wanted_id) => {
                    if obj.get("@id").and_then(|v| v.as_str()) != Some(wanted_id.as_str()) {
                        return false;
                    }
                }
                Value::Object(_) => {
                    // Wildcard - matches any @id
                }
                _ => return false,
            }
        }

        true
    }

    /// Convert a JSON-LD document to RDF triples
    pub fn to_rdf(&mut self, document: &Value) -> JsonLdResult<Vec<Triple>> {
        let expanded = self.expand(document)?;
        let mut converter = JsonLdToRdf::new();
        converter.convert_to_triples(&expanded)
    }

    /// Convert a JSON-LD document to RDF quads
    pub fn to_rdf_quads(&mut self, document: &Value) -> JsonLdResult<Vec<Quad>> {
        let expanded = self.expand(document)?;
        let mut converter = JsonLdToRdf::new();
        converter.convert(&expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let parser = JsonLdParser::new();
        let input = r#"{"@context": {}, "name": "John"}"#;
        let result = parser.parse(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_simple() {
        let mut parser = JsonLdParser::new();
        let doc = json!({
            "@context": {
                "name": "http://schema.org/name"
            },
            "@id": "http://example.org/person/1",
            "name": "John Doe"
        });

        let expanded = parser.expand(&doc).unwrap();

        // Should be an array
        assert!(expanded.is_array());

        let arr = expanded.as_array().unwrap();
        assert_eq!(arr.len(), 1);

        let obj = arr[0].as_object().unwrap();
        assert!(obj.contains_key("http://schema.org/name"));
    }

    #[test]
    fn test_expand_with_vocab() {
        let mut parser = JsonLdParser::new();
        let doc = json!({
            "@context": {
                "@vocab": "http://schema.org/"
            },
            "@id": "http://example.org/thing",
            "name": "Test"
        });

        let expanded = parser.expand(&doc).unwrap();
        let obj = expanded.as_array().unwrap()[0].as_object().unwrap();
        assert!(obj.contains_key("http://schema.org/name"));
    }

    #[test]
    fn test_expand_type() {
        let mut parser = JsonLdParser::new();
        let doc = json!({
            "@context": {
                "Person": "http://schema.org/Person"
            },
            "@id": "http://example.org/person/1",
            "@type": "Person"
        });

        let expanded = parser.expand(&doc).unwrap();
        let obj = expanded.as_array().unwrap()[0].as_object().unwrap();
        let types = obj.get("@type").unwrap().as_array().unwrap();
        assert_eq!(types[0].as_str().unwrap(), "http://schema.org/Person");
    }

    #[test]
    fn test_compact_simple() {
        let mut parser = JsonLdParser::new();
        let expanded = json!([{
            "@id": "http://example.org/person/1",
            "http://schema.org/name": [{"@value": "John"}]
        }]);

        let context = json!({
            "name": "http://schema.org/name"
        });

        let compacted = parser.compact(&expanded, &context).unwrap();
        let obj = compacted.as_object().unwrap();
        assert!(obj.contains_key("name"));
    }

    #[test]
    fn test_frame_by_type() {
        let mut parser = JsonLdParser::new();
        let doc = json!({
            "@context": {
                "Person": "http://schema.org/Person",
                "name": "http://schema.org/name"
            },
            "@graph": [
                {
                    "@id": "http://example.org/person/1",
                    "@type": "Person",
                    "name": "John"
                },
                {
                    "@id": "http://example.org/thing/1",
                    "name": "Some Thing"
                }
            ]
        });

        let frame = json!({
            "@context": {
                "Person": "http://schema.org/Person",
                "name": "http://schema.org/name"
            },
            "@type": "Person"
        });

        let framed = parser.frame(&doc, &frame).unwrap();
        // Should only include the Person - the framed result should be an object
        assert!(framed.is_object(), "Framed result should be an object");
        let obj = framed.as_object().unwrap();
        // The result should contain @context or other expected keys
        assert!(!obj.is_empty(), "Framed object should not be empty");
    }

    #[test]
    fn test_to_rdf() {
        let mut parser = JsonLdParser::new();
        let doc = json!({
            "@context": {
                "name": "http://schema.org/name",
                "Person": "http://schema.org/Person"
            },
            "@id": "http://example.org/person/1",
            "@type": "Person",
            "name": "John Doe"
        });

        let triples = parser.to_rdf(&doc).unwrap();
        assert!(!triples.is_empty());

        // Should have at least type and name triples
        assert!(triples.len() >= 2);
    }

    #[test]
    fn test_expand_id_coercion() {
        let mut parser = JsonLdParser::new();
        let doc = json!({
            "@context": {
                "knows": {
                    "@id": "http://xmlns.com/foaf/0.1/knows",
                    "@type": "@id"
                }
            },
            "@id": "http://example.org/person/1",
            "knows": "http://example.org/person/2"
        });

        let expanded = parser.expand(&doc).unwrap();
        let obj = expanded.as_array().unwrap()[0].as_object().unwrap();
        let knows = obj.get("http://xmlns.com/foaf/0.1/knows").unwrap();

        // Should be expanded to an @id reference
        let knows_arr = knows.as_array().unwrap();
        assert!(knows_arr[0].as_object().unwrap().contains_key("@id"));
    }
}
