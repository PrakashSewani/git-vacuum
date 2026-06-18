use std::time::Duration;

pub fn exponential_backoff(
    attempt: u32,
    base: Duration,
    multiplier: f64,
    cap: Duration,
) -> Duration {
    let exp = multiplier.powi(attempt as i32);
    let delay = base.mul_f64(exp);
    if delay > cap {
        cap
    } else {
        delay
    }
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

pub fn human_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_then_caps() {
        let base = Duration::from_secs(5);
        let cap = Duration::from_secs(60);
        assert_eq!(
            exponential_backoff(0, base, 2.0, cap),
            Duration::from_secs(5)
        );
        assert_eq!(
            exponential_backoff(1, base, 2.0, cap),
            Duration::from_secs(10)
        );
        assert_eq!(
            exponential_backoff(2, base, 2.0, cap),
            Duration::from_secs(20)
        );
        assert_eq!(
            exponential_backoff(3, base, 2.0, cap),
            Duration::from_secs(40)
        );
        assert_eq!(
            exponential_backoff(4, base, 2.0, cap),
            Duration::from_secs(60)
        );
        assert_eq!(
            exponential_backoff(10, base, 2.0, cap),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn human_bytes_works() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(500), "500 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(1_572_864), "1.5 MB");
    }

    #[test]
    fn human_duration_works() {
        assert_eq!(human_duration(Duration::from_secs(5)), "5s");
        assert_eq!(human_duration(Duration::from_secs(75)), "1m 15s");
        assert_eq!(human_duration(Duration::from_secs(3725)), "1h 02m");
    }
}
