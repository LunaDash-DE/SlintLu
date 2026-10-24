//! Network throughput from `/proc/net/dev`.

use std::fs;
use std::time::Instant;

/// Bytes per second over all physical interfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NetworkTraffic {
    pub download: f64,
    pub upload: f64,
}

#[derive(Debug, Default)]
pub struct NetworkSampler {
    previous: Option<(Instant, u64, u64)>,
}

fn is_virtual(name: &str) -> bool {
    name == "lo"
        || ["veth", "docker", "br-", "virbr", "vnet", "tun", "tap"]
            .iter()
            .any(|p| name.starts_with(p))
}

fn totals(dev: &str) -> (u64, u64) {
    dev.lines()
        .skip(2)
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            if is_virtual(name.trim()) {
                return None;
            }
            let fields: Vec<u64> = rest.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            Some((*fields.first()?, *fields.get(8)?))
        })
        .fold((0, 0), |(rx, tx), (r, t)| (rx + r, tx + t))
}

impl NetworkSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Throughput since the previous call (zero on the first call).
    pub fn sample(&mut self) -> NetworkTraffic {
        let dev = fs::read_to_string("/proc/net/dev").unwrap_or_default();
        self.sample_from(&dev, Instant::now())
    }

    fn sample_from(&mut self, dev: &str, now: Instant) -> NetworkTraffic {
        let (rx, tx) = totals(dev);
        let traffic = match self.previous {
            Some((then, prx, ptx)) => {
                let secs = now.duration_since(then).as_secs_f64().max(1e-3);
                NetworkTraffic {
                    download: rx.saturating_sub(prx) as f64 / secs,
                    upload: tx.saturating_sub(ptx) as f64 / secs,
                }
            }
            None => NetworkTraffic::default(),
        };
        self.previous = Some((now, rx, tx));
        traffic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const HEADER: &str =
        "Inter-|   Receive\n face |bytes packets errs drop fifo frame compressed multicast|bytes\n";

    fn dev(rx: u64, tx: u64) -> String {
        format!(
            "{HEADER}    lo: 999 0 0 0 0 0 0 0 999 0 0 0 0 0 0 0\n  eth0: {rx} 0 0 0 0 0 0 0 {tx} 0 0 0 0 0 0 0\n"
        )
    }

    #[test]
    fn throughput_ignores_loopback() {
        let mut sampler = NetworkSampler::new();
        let t0 = Instant::now();
        assert_eq!(sampler.sample_from(&dev(1000, 500), t0), NetworkTraffic::default());
        let traffic = sampler.sample_from(&dev(3000, 1500), t0 + Duration::from_secs(2));
        assert_eq!(traffic, NetworkTraffic { download: 1000.0, upload: 500.0 });
    }
}
