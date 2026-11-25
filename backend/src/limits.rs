use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

// How often something may happen: `burst` at once, then one more every `every`
#[derive(Clone, Copy, Debug)]
pub struct Rate {
    burst: u32,
    every: Duration,
}

impl Rate {
    pub const fn new(burst: u32, every: Duration) -> Rate {
        Rate { burst, every }
    }

    // Whole seconds until one more try is back, for the Retry-After header
    pub fn retry_after(&self) -> u64 {
        self.every.as_secs_f64().ceil().max(1.0) as u64
    }
}

// Sized for a party of 10 phones on one Wi-Fi, which all share one address.
// A host makes a room now and then. Few rooms per address also means it takes many addresses
// to fill the server
pub const CREATE_ROOMS: Rate = Rate::new(3, Duration::from_secs(60));
// Looking up a code, by link or by joining. A real room gives its try back, so a party costs
// nothing and only guessing is slowed: about 1,800 wrong codes an hour
pub const ROOM_LOOKUPS: Rate = Rate::new(30, Duration::from_secs(2));
// Phones reconnect each time they come back to the app
pub const WEBSOCKETS: Rate = Rate::new(30, Duration::from_secs(2));
// Messages on one connection. Real play is a few taps a minute
pub const MESSAGES: Rate = Rate::new(20, Duration::from_millis(100));

// Above this many addresses, ones that are back to a full bucket are forgotten,
// at most once every PRUNE_EVERY so a crowd of addresses can't make every request slow
const PRUNE_AT: usize = 10_000;
const PRUNE_EVERY: Duration = Duration::from_secs(10);
// More than this many still counting means a flood from many addresses. Newcomers then share
// one bucket, so memory stays in check and nobody's count is wiped
const MAX_TRACKED: usize = 100_000;
const OVERFLOW: IpAddr = IpAddr::V6(Ipv6Addr::UNSPECIFIED);

// A bucket of tries that refills over time. Each try takes one
#[derive(Debug)]
pub struct Bucket {
    tokens: f64,
    updated: Instant,
}

impl Bucket {
    pub fn full(rate: Rate, now: Instant) -> Bucket {
        Bucket {
            tokens: f64::from(rate.burst),
            updated: now,
        }
    }

    fn tokens_at(&self, rate: Rate, now: Instant) -> f64 {
        let earned = now.duration_since(self.updated).as_secs_f64() / rate.every.as_secs_f64();
        (self.tokens + earned).min(f64::from(rate.burst))
    }

    // True if there was a try left, which is then used up
    pub fn take(&mut self, rate: Rate, now: Instant) -> bool {
        self.tokens = self.tokens_at(rate, now);
        self.updated = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn is_full(&self, rate: Rate, now: Instant) -> bool {
        self.tokens_at(rate, now) >= f64::from(rate.burst)
    }

    fn give_back(&mut self, rate: Rate, now: Instant) {
        self.tokens = (self.tokens_at(rate, now) + 1.0).min(f64::from(rate.burst));
        self.updated = now;
    }
}

// One bucket per address, or per block of IPv6 addresses
pub struct Limiter {
    rate: Rate,
    // How many leading bits of an IPv6 address make one block
    v6_block: u32,
    buckets: Mutex<Buckets>,
}

struct Buckets {
    by_address: HashMap<IpAddr, Bucket>,
    last_pruned: Instant,
}

impl Limiter {
    pub fn new(rate: Rate, v6_block: u32) -> Limiter {
        Limiter {
            rate,
            v6_block,
            buckets: Mutex::new(Buckets {
                by_address: HashMap::new(),
                last_pruned: Instant::now(),
            }),
        }
    }

    pub fn rate(&self) -> Rate {
        self.rate
    }

    // Uses a try if one is left, in one step, so requests at the same moment can't share a try
    pub fn allow(&self, ip: IpAddr) -> bool {
        self.with_bucket(ip, |bucket, rate, now| bucket.take(rate, now))
    }

    // Hands back a try that turned out not to count
    pub fn give_back(&self, ip: IpAddr) {
        self.with_bucket(ip, |bucket, rate, now| {
            bucket.give_back(rate, now);
            true
        });
    }

    fn with_bucket(
        &self,
        ip: IpAddr,
        use_it: impl FnOnce(&mut Bucket, Rate, Instant) -> bool,
    ) -> bool {
        // This computer: development, Vite's proxy and test scripts. Behind Render's proxy a
        // player's address is always a public one, so this never matches there
        if ip.is_loopback() {
            return true;
        }
        let now = Instant::now();
        let mut key = limit_key(ip, self.v6_block);
        let mut buckets = self.buckets.lock().unwrap();
        let Buckets {
            by_address,
            last_pruned,
        } = &mut *buckets;
        if by_address.len() >= PRUNE_AT && now.duration_since(*last_pruned) >= PRUNE_EVERY {
            by_address.retain(|_, bucket| !bucket.is_full(self.rate, now));
            *last_pruned = now;
        }
        if by_address.len() >= MAX_TRACKED && !by_address.contains_key(&key) {
            key = OVERFLOW;
        }
        let bucket = by_address
            .entry(key)
            .or_insert_with(|| Bucket::full(self.rate, now));
        use_it(bucket, self.rate, now)
    }
}
// Addresses in one IPv6 block share a bucket: one home usually gets a whole /64, and one
// customer often a /56 or /48
fn limit_key(ip: IpAddr, v6_block: u32) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let mask = u128::MAX.checked_shl(128 - v6_block).unwrap_or(0);
            IpAddr::V6(Ipv6Addr::from(u128::from(v6) & mask))
        }
        v4 => v4,
    }
}

// Every limit that's counted per address
pub struct Limits {
    pub create_rooms: Limiter,
    pub room_lookups: Limiter,
    pub websockets: Limiter,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            // A wider block for making rooms and looking them up, so one customer with a whole
            // block of addresses can't fill the server or guess codes faster
            create_rooms: Limiter::new(CREATE_ROOMS, 48),
            room_lookups: Limiter::new(ROOM_LOOKUPS, 48),
            websockets: Limiter::new(WEBSOCKETS, 64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: Rate = Rate::new(3, Duration::from_secs(10));
    const HOME: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 7));
    const OTHER: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(198, 51, 100, 9));

    #[tokio::test(start_paused = true)]
    async fn a_burst_is_allowed_then_one_more_each_time_it_refills() {
        let limiter = Limiter::new(RATE, 64);
        assert!((0..3).all(|_| limiter.allow(HOME)));
        assert!(!limiter.allow(HOME));

        tokio::time::advance(Duration::from_secs(9)).await;
        assert!(!limiter.allow(HOME), "not refilled yet");
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(limiter.allow(HOME));
        assert!(!limiter.allow(HOME));
    }

    #[tokio::test(start_paused = true)]
    async fn a_bucket_never_holds_more_than_the_burst() {
        let limiter = Limiter::new(RATE, 64);
        tokio::time::advance(Duration::from_secs(3600)).await;
        assert!((0..3).all(|_| limiter.allow(HOME)));
        assert!(!limiter.allow(HOME));
    }

    #[tokio::test]
    async fn each_address_has_its_own_bucket() {
        let limiter = Limiter::new(RATE, 64);
        assert!((0..3).all(|_| limiter.allow(HOME)));
        assert!(!limiter.allow(HOME));
        assert!(limiter.allow(OTHER));
    }

    #[tokio::test]
    async fn this_computer_is_never_limited() {
        let limiter = Limiter::new(RATE, 64);
        let local = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        assert!((0..100).all(|_| limiter.allow(local)));
    }

    #[tokio::test]
    async fn one_ipv6_block_shares_a_bucket() {
        let limiter = Limiter::new(RATE, 64);
        let first: IpAddr = "2001:db8:1:2::1".parse().unwrap();
        let second: IpAddr = "2001:db8:1:2:ffff::9".parse().unwrap();
        let elsewhere: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert!((0..3).all(|_| limiter.allow(first)));
        assert!(!limiter.allow(second), "same block, same bucket");
        assert!(limiter.allow(elsewhere));
    }

    fn tracked(limiter: &Limiter) -> usize {
        limiter.buckets.lock().unwrap().by_address.len()
    }

    fn many(n: usize) -> impl Iterator<Item = IpAddr> {
        (0..n as u32).map(|n| IpAddr::V4(std::net::Ipv4Addr::from(0x0b00_0000 + n)))
    }

    #[tokio::test(start_paused = true)]
    async fn full_buckets_are_forgotten_when_there_are_many() {
        let limiter = Limiter::new(RATE, 64);
        many(PRUNE_AT).for_each(|ip| {
            limiter.allow(ip);
        });
        tokio::time::advance(Duration::from_secs(30)).await;
        limiter.allow(HOME);
        assert_eq!(tracked(&limiter), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn forgetting_happens_at_most_every_few_seconds() {
        // Refills in a second, so every bucket is full long before the next look is due
        let limiter = Limiter::new(Rate::new(1, Duration::from_secs(1)), 64);
        many(PRUNE_AT).for_each(|ip| {
            limiter.allow(ip);
        });
        tokio::time::advance(Duration::from_secs(5)).await;
        limiter.allow(HOME);
        assert_eq!(
            tracked(&limiter),
            PRUNE_AT + 1,
            "too soon since the last look"
        );

        tokio::time::advance(Duration::from_secs(5)).await;
        limiter.allow(HOME);
        assert_eq!(tracked(&limiter), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn newcomers_share_one_bucket_when_too_many_are_counted() {
        let limiter = Limiter::new(RATE, 64);
        let first = many(1).next().unwrap();
        many(MAX_TRACKED).for_each(|ip| {
            limiter.allow(ip);
        });
        // Nobody's count is wiped: the first address still has its two tries, no more
        assert!(limiter.allow(first) && limiter.allow(first));
        assert!(!limiter.allow(first));

        // Two newcomers share the overflow bucket's three tries
        assert!(limiter.allow(HOME) && limiter.allow(OTHER) && limiter.allow(HOME));
        assert!(!limiter.allow(OTHER));
        assert_eq!(tracked(&limiter), MAX_TRACKED + 1);
    }

    #[tokio::test]
    async fn a_try_given_back_can_be_used_again() {
        let limiter = Limiter::new(RATE, 64);
        assert!((0..3).all(|_| limiter.allow(HOME)));
        assert!(!limiter.allow(HOME));
        limiter.give_back(HOME);
        assert!(limiter.allow(HOME));
        assert!(!limiter.allow(HOME));
    }

    #[tokio::test]
    async fn giving_back_never_overfills() {
        let limiter = Limiter::new(RATE, 64);
        (0..10).for_each(|_| limiter.give_back(HOME));
        assert!((0..3).all(|_| limiter.allow(HOME)));
        assert!(!limiter.allow(HOME));
    }

    #[tokio::test]
    async fn a_wider_block_shares_one_bucket_across_homes() {
        let limiter = Limiter::new(RATE, 48);
        let one: IpAddr = "2001:db8:1:2::1".parse().unwrap();
        let next_door: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert!((0..3).all(|_| limiter.allow(one)));
        assert!(!limiter.allow(next_door), "same /48");
    }

    #[test]
    fn an_ipv4_address_written_as_ipv6_is_the_same_address() {
        let wrapped: IpAddr = "::ffff:203.0.113.7".parse().unwrap();
        assert_eq!(limit_key(wrapped, 64), HOME);
    }

    #[test]
    fn retry_after_is_whole_seconds() {
        assert_eq!(CREATE_ROOMS.retry_after(), 60);
        assert_eq!(MESSAGES.retry_after(), 1);
    }
}
