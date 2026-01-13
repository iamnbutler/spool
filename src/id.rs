use chrono::Utc;
use rand::Rng;

/// Generate a unique task ID in the format `{timestamp}-{random}`
/// - timestamp: Unix epoch milliseconds, base36 encoded
/// - random: 4 random alphanumeric characters
pub fn generate_id() -> String {
    let timestamp = Utc::now().timestamp_millis() as u64;
    let timestamp_b36 = base36_encode(timestamp);
    let random = random_alphanumeric(4);
    format!("{}-{}", timestamp_b36, random)
}

fn base36_encode(mut n: u64) -> String {
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut result = Vec::new();
    while n > 0 {
        result.push(CHARS[(n % 36) as usize]);
        n /= 36;
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}

fn random_alphanumeric(len: usize) -> String {
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id = generate_id();
        assert!(id.contains('-'));
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(!parts[0].is_empty());
        assert_eq!(parts[1].len(), 4);
    }

    #[test]
    fn test_base36_encode() {
        assert_eq!(base36_encode(0), "0");
        assert_eq!(base36_encode(35), "z");
        assert_eq!(base36_encode(36), "10");
    }
}
