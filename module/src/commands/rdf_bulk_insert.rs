//! RDF.BULK_INSERT Command Implementation
//!
//! Bulk inserts RDF data from files with streaming, batch processing,
//! progress reporting, and partial failure recovery.

use redis_module::{Context, RedisError, RedisResult, RedisString, RedisValue};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use falkorsemantic_mapper::Mapper;
use falkorsemantic_parser::formats::NTriplesReader;
use falkorsemantic_parser::rdf::Triple;
use falkorsemantic_parser::TurtleParser;

use super::rdf_insert::RdfFormat;

/// Default batch size for processing
const DEFAULT_BATCH_SIZE: usize = 1000;

/// Progress reporting interval (number of triples)
const PROGRESS_INTERVAL: usize = 10000;

/// Bulk insert statistics
#[derive(Debug, Default)]
pub struct BulkInsertStats {
    /// Total triples parsed
    pub triples_parsed: usize,
    /// Total statements executed
    pub statements_executed: usize,
    /// Total errors encountered
    pub errors: usize,
    /// Number of batches processed
    pub batches_processed: usize,
    /// Triples that failed (for recovery)
    pub failed_triples: Vec<usize>,
    /// Error messages
    pub error_messages: Vec<String>,
    /// Last successfully processed line (for recovery)
    pub last_successful_line: usize,
}

/// Progress callback type (for future use with async progress reporting)
#[allow(dead_code)]
pub type ProgressCallback = Box<dyn Fn(usize, usize) + Send>;

/// Bulk insert arguments
struct BulkInsertArgs<'a> {
    graph_key: &'a str,
    file_path: &'a str,
    format: Option<RdfFormat>,
    batch_size: usize,
    skip_lines: usize,
    max_errors: usize,
    continue_on_error: bool,
}

impl<'a> BulkInsertArgs<'a> {
    fn parse(args: &'a [RedisString]) -> Result<Self, RedisError> {
        // args[0] is the command name
        if args.len() < 3 {
            return Err(RedisError::WrongArity);
        }

        let graph_key = args[1]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid graph key".into()))?;
        let file_path = args[2]
            .try_as_str()
            .map_err(|_| RedisError::String("Invalid file path".into()))?;

        let mut format = None;
        let mut batch_size = DEFAULT_BATCH_SIZE;
        let mut skip_lines = 0;
        let mut max_errors = usize::MAX;
        let mut continue_on_error = true;

        // Parse optional arguments
        let mut i = 3;
        while i < args.len() {
            let arg = args[i]
                .try_as_str()
                .map_err(|_| RedisError::String("Invalid argument".into()))?;

            match arg.to_uppercase().as_str() {
                "FORMAT" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String(
                            "FORMAT requires a value (turtle, ntriples, nquads, jsonld)".into(),
                        ));
                    }
                    let fmt_str = args[i]
                        .try_as_str()
                        .map_err(|_| RedisError::String("Invalid format value".into()))?;
                    format = Some(RdfFormat::from_str(fmt_str).ok_or_else(|| {
                        RedisError::String(format!(
                            "Unknown format '{}'. Use: turtle, ntriples, nquads, jsonld",
                            fmt_str
                        ))
                    })?);
                }
                "BATCH" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String("BATCH requires a size value".into()));
                    }
                    let size_str = args[i]
                        .try_as_str()
                        .map_err(|_| RedisError::String("Invalid batch size".into()))?;
                    batch_size = size_str.parse().map_err(|_| {
                        RedisError::String(format!("Invalid batch size: {}", size_str))
                    })?;
                    if batch_size == 0 {
                        return Err(RedisError::String("Batch size must be > 0".into()));
                    }
                }
                "SKIP" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String("SKIP requires a line count".into()));
                    }
                    let skip_str = args[i]
                        .try_as_str()
                        .map_err(|_| RedisError::String("Invalid skip count".into()))?;
                    skip_lines = skip_str.parse().map_err(|_| {
                        RedisError::String(format!("Invalid skip count: {}", skip_str))
                    })?;
                }
                "MAXERRORS" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(RedisError::String("MAXERRORS requires a count".into()));
                    }
                    let max_str = args[i]
                        .try_as_str()
                        .map_err(|_| RedisError::String("Invalid max errors".into()))?;
                    max_errors = max_str.parse().map_err(|_| {
                        RedisError::String(format!("Invalid max errors: {}", max_str))
                    })?;
                }
                "STOPONERROR" => {
                    continue_on_error = false;
                }
                _ => {
                    return Err(RedisError::String(format!("Unknown argument: {}", arg)));
                }
            }
            i += 1;
        }

        Ok(BulkInsertArgs {
            graph_key,
            file_path,
            format,
            batch_size,
            skip_lines,
            max_errors,
            continue_on_error,
        })
    }
}

/// Detect format from file extension
fn detect_format_from_path(path: &Path) -> Option<RdfFormat> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| match ext.to_lowercase().as_str() {
            "ttl" | "turtle" => Some(RdfFormat::Turtle),
            "nt" | "ntriples" => Some(RdfFormat::NTriples),
            "nq" | "nquads" => Some(RdfFormat::NQuads),
            "jsonld" | "json" => Some(RdfFormat::JsonLd),
            _ => None,
        })
}

/// Stream and parse N-Triples file in batches
fn stream_ntriples<F>(
    reader: BufReader<File>,
    skip_lines: usize,
    batch_size: usize,
    mut process_batch: F,
) -> Result<BulkInsertStats, String>
where
    F: FnMut(&[Triple], usize) -> Result<(usize, usize), String>,
{
    let mut stats = BulkInsertStats::default();
    let ntriples_reader = NTriplesReader::new();
    let mut batch: Vec<Triple> = Vec::with_capacity(batch_size);
    let mut line_number = 0;
    let mut batch_start_line = skip_lines;

    for line_result in reader.lines() {
        line_number += 1;

        // Skip initial lines if requested (for recovery)
        if line_number <= skip_lines {
            continue;
        }

        let line = line_result.map_err(|e| format!("IO error at line {}: {}", line_number, e))?;
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Parse the line
        match ntriples_reader.parse_all_str(trimmed) {
            Ok(triples) => {
                for triple in triples {
                    batch.push(triple);
                    stats.triples_parsed += 1;

                    // Process batch when full
                    if batch.len() >= batch_size {
                        match process_batch(&batch, batch_start_line) {
                            Ok((executed, errors)) => {
                                stats.statements_executed += executed;
                                stats.errors += errors;
                                stats.batches_processed += 1;
                                stats.last_successful_line = line_number;
                            }
                            Err(e) => {
                                stats.error_messages.push(e);
                                stats.errors += batch.len();
                            }
                        }
                        batch.clear();
                        batch_start_line = line_number + 1;

                        // Report progress
                        if stats.triples_parsed % PROGRESS_INTERVAL == 0 {
                            log::info!(
                                "Progress: {} triples parsed, {} executed, {} errors",
                                stats.triples_parsed,
                                stats.statements_executed,
                                stats.errors
                            );
                        }
                    }
                }
            }
            Err(e) => {
                stats.errors += 1;
                stats.failed_triples.push(line_number);
                stats
                    .error_messages
                    .push(format!("Parse error at line {}: {}", line_number, e));
            }
        }
    }

    // Process remaining batch
    if !batch.is_empty() {
        match process_batch(&batch, batch_start_line) {
            Ok((executed, errors)) => {
                stats.statements_executed += executed;
                stats.errors += errors;
                stats.batches_processed += 1;
                stats.last_successful_line = line_number;
            }
            Err(e) => {
                stats.error_messages.push(e);
                stats.errors += batch.len();
            }
        }
    }

    Ok(stats)
}

/// Process a batch of triples - execute against FalkorDB
fn process_batch(
    ctx: &Context,
    graph_key: &str,
    triples: &[Triple],
    _batch_start_line: usize,
) -> Result<(usize, usize), String> {
    if triples.is_empty() {
        return Ok((0, 0));
    }

    let mapper = Mapper::new();
    let statements = mapper
        .map_triples(triples)
        .map_err(|e| format!("Mapping error: {}", e))?;

    let mut executed = 0;
    let mut errors = 0;

    // Combine statements for batch execution
    let combined = statements.join("\n");
    let result = ctx.call("GRAPH.QUERY", &[graph_key, combined.as_str()]);

    match result {
        Ok(_) => {
            executed = statements.len();
        }
        Err(e) => {
            errors = statements.len();
            log::error!("Batch execution error: {:?}", e);
        }
    }

    Ok((executed, errors))
}

/// Load entire file for formats that require complete parsing (Turtle, JSON-LD)
fn load_complete_file(
    ctx: &Context,
    graph_key: &str,
    file_path: &str,
    format: RdfFormat,
    batch_size: usize,
) -> Result<BulkInsertStats, String> {
    let content =
        std::fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    let triples = match format {
        RdfFormat::Turtle => {
            let mut parser = TurtleParser::new();
            parser.parse(&content).map_err(|e| e.to_string())?
        }
        RdfFormat::JsonLd => {
            return Err("JSON-LD parsing not yet implemented".into());
        }
        _ => {
            return Err(format!("Format {:?} should use streaming", format));
        }
    };

    let mut stats = BulkInsertStats {
        triples_parsed: triples.len(),
        ..Default::default()
    };

    // Process in batches
    let mapper = Mapper::new();
    for chunk in triples.chunks(batch_size) {
        let statements = mapper
            .map_triples(chunk)
            .map_err(|e| format!("Mapping error: {}", e))?;

        let combined = statements.join("\n");
        let result = ctx.call("GRAPH.QUERY", &[graph_key, combined.as_str()]);

        match result {
            Ok(_) => {
                stats.statements_executed += statements.len();
                stats.batches_processed += 1;
            }
            Err(e) => {
                stats.errors += statements.len();
                stats.error_messages.push(format!("Batch error: {:?}", e));
            }
        }

        // Progress reporting
        if stats.statements_executed % PROGRESS_INTERVAL == 0 {
            log::info!(
                "Progress: {} statements executed, {} errors",
                stats.statements_executed,
                stats.errors
            );
        }
    }

    Ok(stats)
}

/// RDF.BULK_INSERT command handler
///
/// Syntax: RDF.BULK_INSERT <graph_key> <file_path> [FORMAT turtle|ntriples|nquads|jsonld]
///         [BATCH size] [SKIP lines] [MAXERRORS count] [STOPONERROR]
///
/// Arguments:
/// - graph_key: The FalkorDB graph name to insert into
/// - file_path: Path to the RDF file to load
/// - FORMAT: Optional format specifier (auto-detected from extension if not provided)
/// - BATCH: Batch size for processing (default: 1000)
/// - SKIP: Number of lines to skip (for recovery from partial failures)
/// - MAXERRORS: Maximum errors before stopping (default: unlimited)
/// - STOPONERROR: Stop on first error instead of continuing
///
/// Returns: Array with [triples_parsed, statements_executed, errors, batches_processed, last_line]
pub fn rdf_bulk_insert(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    let parsed_args = BulkInsertArgs::parse(&args)?;

    log::info!(
        "RDF.BULK_INSERT graph={} file={} batch_size={} skip={}",
        parsed_args.graph_key,
        parsed_args.file_path,
        parsed_args.batch_size,
        parsed_args.skip_lines
    );

    // Validate file exists
    let path = Path::new(parsed_args.file_path);
    if !path.exists() {
        return Err(RedisError::String(format!(
            "File not found: {}",
            parsed_args.file_path
        )));
    }

    // Detect format
    let format = parsed_args
        .format
        .or_else(|| detect_format_from_path(path))
        .unwrap_or(RdfFormat::NTriples);

    log::info!("Using format: {:?}", format);

    // Process based on format
    let stats = match format {
        RdfFormat::NTriples | RdfFormat::NQuads => {
            // Stream-based processing for line-based formats
            let file = File::open(path)
                .map_err(|e| RedisError::String(format!("Failed to open file: {}", e)))?;
            let reader = BufReader::with_capacity(64 * 1024, file);

            let graph_key = parsed_args.graph_key.to_string();
            let max_errors = parsed_args.max_errors;
            let continue_on_error = parsed_args.continue_on_error;
            let mut total_errors = 0;

            stream_ntriples(
                reader,
                parsed_args.skip_lines,
                parsed_args.batch_size,
                |triples, batch_start| {
                    let result = process_batch(ctx, &graph_key, triples, batch_start);
                    if let Ok((_, errors)) = &result {
                        total_errors += errors;
                        if !continue_on_error && *errors > 0 {
                            return Err("Stopped on error".into());
                        }
                        if total_errors >= max_errors {
                            return Err(format!("Max errors ({}) reached", max_errors));
                        }
                    }
                    result
                },
            )
            .map_err(RedisError::String)?
        }
        RdfFormat::Turtle | RdfFormat::JsonLd => {
            // Load complete file for formats requiring full context
            load_complete_file(
                ctx,
                parsed_args.graph_key,
                parsed_args.file_path,
                format,
                parsed_args.batch_size,
            )
            .map_err(RedisError::String)?
        }
    };

    log::info!(
        "Bulk insert complete: parsed={}, executed={}, errors={}, batches={}",
        stats.triples_parsed,
        stats.statements_executed,
        stats.errors,
        stats.batches_processed
    );

    // Return detailed statistics
    Ok(RedisValue::Array(vec![
        RedisValue::Integer(stats.triples_parsed as i64),
        RedisValue::Integer(stats.statements_executed as i64),
        RedisValue::Integer(stats.errors as i64),
        RedisValue::Integer(stats.batches_processed as i64),
        RedisValue::Integer(stats.last_successful_line as i64),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_detect_format_from_path() {
        assert_eq!(
            detect_format_from_path(Path::new("data.ttl")),
            Some(RdfFormat::Turtle)
        );
        assert_eq!(
            detect_format_from_path(Path::new("data.nt")),
            Some(RdfFormat::NTriples)
        );
        assert_eq!(
            detect_format_from_path(Path::new("data.nq")),
            Some(RdfFormat::NQuads)
        );
        assert_eq!(
            detect_format_from_path(Path::new("data.jsonld")),
            Some(RdfFormat::JsonLd)
        );
        assert_eq!(detect_format_from_path(Path::new("data.txt")), None);
    }

    #[test]
    fn test_bulk_insert_stats_default() {
        let stats = BulkInsertStats::default();
        assert_eq!(stats.triples_parsed, 0);
        assert_eq!(stats.statements_executed, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.batches_processed, 0);
    }

    #[test]
    fn test_stream_ntriples_parsing() {
        // Create a temp file with N-Triples
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "<http://example.org/s1> <http://example.org/p> <http://example.org/o1> ."
        )
        .unwrap();
        writeln!(
            file,
            "<http://example.org/s2> <http://example.org/p> <http://example.org/o2> ."
        )
        .unwrap();
        writeln!(file, "# This is a comment").unwrap();
        writeln!(
            file,
            "<http://example.org/s3> <http://example.org/p> <http://example.org/o3> ."
        )
        .unwrap();
        file.flush().unwrap();

        let reader = BufReader::new(File::open(file.path()).unwrap());
        let mut batches_received = 0;
        let mut triples_in_batches = 0;

        let stats = stream_ntriples(reader, 0, 2, |triples, _| {
            batches_received += 1;
            triples_in_batches += triples.len();
            Ok((triples.len(), 0))
        })
        .unwrap();

        assert_eq!(stats.triples_parsed, 3);
        assert_eq!(batches_received, 2); // 2 + 1
        assert_eq!(triples_in_batches, 3);
    }

    #[test]
    fn test_stream_with_skip_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "<http://example.org/s1> <http://example.org/p> <http://example.org/o1> ."
        )
        .unwrap();
        writeln!(
            file,
            "<http://example.org/s2> <http://example.org/p> <http://example.org/o2> ."
        )
        .unwrap();
        writeln!(
            file,
            "<http://example.org/s3> <http://example.org/p> <http://example.org/o3> ."
        )
        .unwrap();
        file.flush().unwrap();

        let reader = BufReader::new(File::open(file.path()).unwrap());

        let stats = stream_ntriples(reader, 1, 10, |triples, _| Ok((triples.len(), 0))).unwrap();

        // Should skip first line
        assert_eq!(stats.triples_parsed, 2);
    }
}
