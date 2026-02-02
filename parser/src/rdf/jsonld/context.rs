//! Context resolution for JSON-LD
//!
//! Handles @context processing including embedded contexts and base IRI resolution.

use std::collections::HashMap;
use serde_json::Value;

use super::error::{JsonLdError, JsonLdResult};

/// A resolved JSON-LD context containing term definitions
#[derive(Debug, Clone, Default)]
pub struct ResolvedContext {
    /// Term definitions mapping terms to IRIs
    pub terms: HashMap<String, TermDefinition>,
    /// The base IRI for relative IRI resolution
    pub base: Option<String>,
    /// The vocabulary mapping (@vocab)
    pub vocab: Option<String>,
    /// Language setting (@language)
    pub language: Option<String>,
}

/// A term definition in a JSON-LD context
#[derive(Debug, Clone)]
pub struct TermDefinition {
    /// The IRI that this term expands to
    pub iri: String,
    /// The type coercion (@type)
    pub type_coercion: Option<String>,
    /// The container mapping (@container)
    pub container: Option<ContainerType>,
    /// Language for this term
    pub language: Option<String>,
    /// Whether this term is a reverse property
    pub reverse: bool,
}

/// Container types for JSON-LD
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerType {
    /// @list container
    List,
    /// @set container
    Set,
    /// @index container
    Index,
    /// @language container
    Language,
    /// @graph container
    Graph,
    /// @id container
    Id,
    /// @type container
    Type,
}

impl ContainerType {
    /// Parse a container type from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "@list" => Some(ContainerType::List),
            "@set" => Some(ContainerType::Set),
            "@index" => Some(ContainerType::Index),
            "@language" => Some(ContainerType::Language),
            "@graph" => Some(ContainerType::Graph),
            "@id" => Some(ContainerType::Id),
            "@type" => Some(ContainerType::Type),
            _ => None,
        }
    }
}

/// Context resolver for processing @context values
#[derive(Debug, Default)]
pub struct ContextResolver {
    /// Cache of resolved contexts by URL (for future remote context support)
    #[allow(dead_code)]
    cache: HashMap<String, ResolvedContext>,
}

impl ContextResolver {
    /// Create a new context resolver
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Resolve a @context value
    pub fn resolve(&mut self, context: &Value, base_iri: Option<&str>) -> JsonLdResult<ResolvedContext> {
        match context {
            Value::Null => Ok(ResolvedContext::default()),
            Value::String(url) => {
                // Remote context reference - not supported for now
                Err(JsonLdError::RemoteContextNotSupported(url.clone()))
            }
            Value::Object(obj) => self.resolve_object(obj, base_iri),
            Value::Array(arr) => self.resolve_array(arr, base_iri),
            _ => Err(JsonLdError::context("Invalid @context value type")),
        }
    }

    /// Resolve an array of contexts (merge them)
    fn resolve_array(
        &mut self,
        contexts: &[Value],
        base_iri: Option<&str>,
    ) -> JsonLdResult<ResolvedContext> {
        let mut result = ResolvedContext::default();
        
        for ctx in contexts {
            let resolved = self.resolve(ctx, base_iri)?;
            // Merge contexts - later contexts override earlier ones
            result.terms.extend(resolved.terms);
            if resolved.base.is_some() {
                result.base = resolved.base;
            }
            if resolved.vocab.is_some() {
                result.vocab = resolved.vocab;
            }
            if resolved.language.is_some() {
                result.language = resolved.language;
            }
        }
        
        Ok(result)
    }

    /// Resolve an embedded context object
    fn resolve_object(
        &mut self,
        obj: &serde_json::Map<String, Value>,
        base_iri: Option<&str>,
    ) -> JsonLdResult<ResolvedContext> {
        let mut result = ResolvedContext {
            base: base_iri.map(String::from),
            ..Default::default()
        };

        for (key, value) in obj {
            match key.as_str() {
                "@base" => {
                    if let Value::String(base) = value {
                        result.base = Some(base.clone());
                    } else if !value.is_null() {
                        return Err(JsonLdError::context("@base must be a string or null"));
                    }
                }
                "@vocab" => {
                    if let Value::String(vocab) = value {
                        result.vocab = Some(vocab.clone());
                    } else if !value.is_null() {
                        return Err(JsonLdError::context("@vocab must be a string or null"));
                    }
                }
                "@language" => {
                    if let Value::String(lang) = value {
                        result.language = Some(lang.clone());
                    } else if !value.is_null() {
                        return Err(JsonLdError::context("@language must be a string or null"));
                    }
                }
                _ if !key.starts_with('@') => {
                    // Term definition
                    let term_def = self.parse_term_definition(key, value, &result)?;
                    result.terms.insert(key.clone(), term_def);
                }
                _ => {
                    // Ignore unknown @ keywords for forward compatibility
                }
            }
        }

        Ok(result)
    }

    /// Parse a term definition
    fn parse_term_definition(
        &self,
        term: &str,
        value: &Value,
        context: &ResolvedContext,
    ) -> JsonLdResult<TermDefinition> {
        match value {
            Value::String(iri) => {
                // Simple term definition: "term": "iri"
                let expanded = self.expand_iri(iri, context)?;
                Ok(TermDefinition {
                    iri: expanded,
                    type_coercion: None,
                    container: None,
                    language: None,
                    reverse: false,
                })
            }
            Value::Object(obj) => {
                // Expanded term definition
                let id = obj
                    .get("@id")
                    .and_then(|v| v.as_str())
                    .map(|s| self.expand_iri(s, context))
                    .transpose()?
                    .unwrap_or_else(|| {
                        // If no @id, the term itself is the IRI (with vocab prefix)
                        if let Some(ref vocab) = context.vocab {
                            format!("{}{}", vocab, term)
                        } else {
                            term.to_string()
                        }
                    });

                let type_coercion = obj
                    .get("@type")
                    .and_then(|v| v.as_str())
                    .map(|s| self.expand_iri(s, context))
                    .transpose()?;

                let container = obj
                    .get("@container")
                    .and_then(|v| v.as_str())
                    .and_then(ContainerType::from_str);

                let language = obj.get("@language").and_then(|v| v.as_str()).map(String::from);

                let reverse = obj
                    .get("@reverse")
                    .and_then(|v| v.as_str())
                    .is_some();

                Ok(TermDefinition {
                    iri: id,
                    type_coercion,
                    container,
                    language,
                    reverse,
                })
            }
            Value::Null => {
                // Null removes the term definition
                Ok(TermDefinition {
                    iri: String::new(),
                    type_coercion: None,
                    container: None,
                    language: None,
                    reverse: false,
                })
            }
            _ => Err(JsonLdError::context(format!(
                "Invalid term definition for '{}'",
                term
            ))),
        }
    }

    /// Expand a compact IRI or term to a full IRI
    fn expand_iri(&self, value: &str, context: &ResolvedContext) -> JsonLdResult<String> {
        // Check if it's already an absolute IRI
        if value.contains("://") {
            return Ok(value.to_string());
        }

        // Check for JSON-LD keywords
        if value.starts_with('@') {
            return Ok(value.to_string());
        }

        // Check for prefix:suffix (compact IRI)
        if let Some(colon_pos) = value.find(':') {
            let prefix = &value[..colon_pos];
            let suffix = &value[colon_pos + 1..];

            // Look up prefix in context
            if let Some(term_def) = context.terms.get(prefix) {
                return Ok(format!("{}{}", term_def.iri, suffix));
            }

            // If prefix not found, it might be a scheme (like "http:")
            // In that case, return as-is
            return Ok(value.to_string());
        }

        // Check if term exists in context
        if let Some(term_def) = context.terms.get(value) {
            return Ok(term_def.iri.clone());
        }

        // Apply @vocab if present
        if let Some(ref vocab) = context.vocab {
            return Ok(format!("{}{}", vocab, value));
        }

        // Return as-is
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_simple_context() {
        let mut resolver = ContextResolver::new();
        let context = json!({
            "name": "http://schema.org/name",
            "homepage": "http://schema.org/url"
        });

        let resolved = resolver.resolve(&context, None).unwrap();
        assert_eq!(resolved.terms.get("name").unwrap().iri, "http://schema.org/name");
        assert_eq!(resolved.terms.get("homepage").unwrap().iri, "http://schema.org/url");
    }

    #[test]
    fn test_resolve_vocab() {
        let mut resolver = ContextResolver::new();
        let context = json!({
            "@vocab": "http://schema.org/",
            "name": "http://xmlns.com/foaf/0.1/name"
        });

        let resolved = resolver.resolve(&context, None).unwrap();
        assert_eq!(resolved.vocab, Some("http://schema.org/".to_string()));
        assert_eq!(resolved.terms.get("name").unwrap().iri, "http://xmlns.com/foaf/0.1/name");
    }

    #[test]
    fn test_resolve_expanded_term() {
        let mut resolver = ContextResolver::new();
        let context = json!({
            "knows": {
                "@id": "http://xmlns.com/foaf/0.1/knows",
                "@type": "@id"
            }
        });

        let resolved = resolver.resolve(&context, None).unwrap();
        let term = resolved.terms.get("knows").unwrap();
        assert_eq!(term.iri, "http://xmlns.com/foaf/0.1/knows");
        assert_eq!(term.type_coercion, Some("@id".to_string()));
    }

    #[test]
    fn test_resolve_prefix() {
        let mut resolver = ContextResolver::new();
        let context = json!({
            "foaf": "http://xmlns.com/foaf/0.1/",
            "name": "foaf:name"
        });

        let resolved = resolver.resolve(&context, None).unwrap();
        assert_eq!(resolved.terms.get("name").unwrap().iri, "http://xmlns.com/foaf/0.1/name");
    }

    #[test]
    fn test_resolve_array_context() {
        let mut resolver = ContextResolver::new();
        let context = json!([
            { "name": "http://schema.org/name" },
            { "title": "http://purl.org/dc/terms/title" }
        ]);

        let resolved = resolver.resolve(&context, None).unwrap();
        assert!(resolved.terms.contains_key("name"));
        assert!(resolved.terms.contains_key("title"));
    }

    #[test]
    fn test_container_types() {
        let mut resolver = ContextResolver::new();
        let context = json!({
            "tags": {
                "@id": "http://example.org/tags",
                "@container": "@set"
            }
        });

        let resolved = resolver.resolve(&context, None).unwrap();
        let term = resolved.terms.get("tags").unwrap();
        assert_eq!(term.container, Some(ContainerType::Set));
    }
}
