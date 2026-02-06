//! Utility functions for command handlers

use redis_module::{Context, RedisValue};

/// Scan for keys matching a pattern using SCAN (non-blocking)
///
/// SCAN is preferred over KEYS for production use because:
/// - KEYS blocks Redis while scanning all keys (O(n) blocking)
/// - SCAN is incremental and returns results in batches
///
/// Returns a vector of matching key names as strings.
pub fn scan_keys(ctx: &Context, pattern: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut cursor = "0".to_string();

    loop {
        // SCAN cursor MATCH pattern COUNT 100
        let result = ctx.call("SCAN", &[&cursor, "MATCH", pattern, "COUNT", "100"]);

        match result {
            Ok(RedisValue::Array(arr)) if arr.len() >= 2 => {
                // First element is the next cursor
                cursor = match &arr[0] {
                    RedisValue::SimpleString(s) | RedisValue::BulkString(s) => s.clone(),
                    RedisValue::Integer(i) => i.to_string(),
                    _ => break,
                };

                // Second element is the array of keys
                if let RedisValue::Array(key_arr) = &arr[1] {
                    for key in key_arr {
                        match key {
                            RedisValue::SimpleString(k) | RedisValue::BulkString(k) => {
                                keys.push(k.clone());
                            }
                            _ => {}
                        }
                    }
                }

                // If cursor is "0", we've completed the full iteration
                if cursor == "0" {
                    break;
                }
            }
            _ => break,
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    // Note: scan_keys requires a Redis context to test
    // Integration tests should be in a separate test module
}
