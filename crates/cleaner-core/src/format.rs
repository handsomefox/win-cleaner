/// Formats a byte count using 1024-based units with two decimals, matching
/// the application UI.
#[must_use]
pub fn human_bytes(bytes: u64) -> String {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    const T: u64 = G * 1024;

    #[expect(clippy::cast_precision_loss, reason = "display-only approximation")]
    match bytes {
        b if b >= T => format!("{:.2} TB", b as f64 / T as f64),
        b if b >= G => format!("{:.2} GB", b as f64 / G as f64),
        b if b >= M => format!("{:.2} MB", b as f64 / M as f64),
        b if b >= K => format!("{:.2} KB", b as f64 / K as f64),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::human_bytes;

    #[test]
    fn formats_each_magnitude() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.00 KB");
        assert_eq!(human_bytes(1536), "1.50 KB");
        assert_eq!(human_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(human_bytes(5 * 1024 * 1024 * 1024), "5.00 GB");
        assert_eq!(human_bytes(2 * 1024 * 1024 * 1024 * 1024), "2.00 TB");
    }
}
