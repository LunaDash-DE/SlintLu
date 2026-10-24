//! CPU utilisation from `/proc/stat`.

use std::fs;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Times {
    busy: u64,
    total: u64,
}

fn parse_line(line: &str) -> Option<Times> {
    let values: Vec<u64> = line.split_whitespace().skip(1).filter_map(|v| v.parse().ok()).collect();
    if values.len() < 4 {
        return None;
    }
    // user nice system idle iowait irq softirq steal (guest time is already in user/nice).
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    let total: u64 = values.iter().take(8).sum();
    Some(Times { busy: total.saturating_sub(idle), total })
}

/// Computes utilisation between successive calls to [`CpuSampler::sample`].
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous: Vec<Times>,
}

impl CpuSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `(overall, per_core)` utilisation in `0.0..=1.0` since the previous call
    /// (zero on the first call).
    pub fn sample(&mut self) -> (f32, Vec<f32>) {
        let stat = fs::read_to_string("/proc/stat").unwrap_or_default();
        self.sample_from(&stat)
    }

    fn sample_from(&mut self, stat: &str) -> (f32, Vec<f32>) {
        let current: Vec<Times> =
            stat.lines().take_while(|l| l.starts_with("cpu")).filter_map(parse_line).collect();
        let usage: Vec<f32> = current
            .iter()
            .enumerate()
            .map(|(i, now)| {
                let before = self.previous.get(i).copied().unwrap_or_default();
                let total = now.total.saturating_sub(before.total);
                let busy = now.busy.saturating_sub(before.busy);
                if total == 0 || before.total == 0 { 0.0 } else { busy as f32 / total as f32 }
            })
            .collect();
        self.previous = current;
        let overall = usage.first().copied().unwrap_or(0.0);
        (overall, usage.into_iter().skip(1).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utilisation_between_samples() {
        let mut sampler = CpuSampler::new();
        let first = "cpu  100 0 100 800 0 0 0 0\ncpu0 50 0 50 400 0 0 0 0\nintr 1\n";
        let second = "cpu  150 0 150 900 0 0 0 0\ncpu0 100 0 50 450 0 0 0 0\nintr 1\n";
        assert_eq!(sampler.sample_from(first), (0.0, vec![0.0]));
        let (overall, cores) = sampler.sample_from(second);
        assert!((overall - 0.5).abs() < 1e-6);
        assert!((cores[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn real_proc_stat_is_readable() {
        let mut sampler = CpuSampler::new();
        sampler.sample();
        let (overall, cores) = sampler.sample();
        assert!((0.0..=1.0).contains(&overall));
        assert!(!cores.is_empty());
    }
}
