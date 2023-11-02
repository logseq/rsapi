//! A DoH client for the sync crate
//!
//! To be used in ClientBuilder.dns_resolver
//!
//! Ratioonale: Some Proxy servers has special handling of DNS requests, so we need to use DoH to bypass it.

use futures::lock::Mutex;
use once_cell::sync::Lazy;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::RwLock,
    time::{Duration, Instant},
};

use hyper::client::connect::dns::Name;
use reqwest::{
    dns::{Addrs, Resolve, Resolving},
    Client,
};
use serde::Deserialize;

static DNS_CACHE: Lazy<DNSCache> = Lazy::new(DNSCache::new);
static DNS_HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .http2_prior_knowledge()
        .http2_keep_alive_interval(Duration::from_secs(5))
        .http2_keep_alive_timeout(Duration::from_secs(5))
        .build()
        .unwrap()
});
// Avoid concurrent queries
static QUERY_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub struct DoHResolver;

impl Resolve for DoHResolver {
    fn resolve(&self, name: Name) -> Resolving {
        // also 1.1.1.1
        let url = format!("https://1.12.12.12/dns-query?name={}&type=A", name);
        Box::pin(async move {
            let lock = QUERY_LOCK.lock().await;

            let mut ret = vec![];
            if let Some(mut addrs) = DNS_CACHE.get(&name) {
                ret.append(&mut addrs);
            } else {
                let resp = DNS_HTTP_CLIENT.get(&url).send().await?;
                let resp = resp.json::<DnsResponse>().await?;

                if let Some(ans) = resp.answer {
                    for record in ans {
                        if record.typ == 1 {
                            // A record
                            let addr = SocketAddr::new(record.data.parse().unwrap(), 443);
                            DNS_CACHE.insert(name.clone(), addr, record.ttl);
                            ret.push(addr);
                        }
                    }
                }
                log::debug!("DoH resolved: {}: {:?}", name, ret);
            }

            drop(lock);
            let addrs: Addrs = Box::new(ret.into_iter());
            Ok(addrs)
        })
    }
}

#[derive(Debug)]
pub struct Entry {
    addr: SocketAddr,
    expiration: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct DNSCache {
    cache: RwLock<HashMap<Name, Vec<Entry>>>,
}

unsafe impl Send for DNSCache {}
unsafe impl Sync for DNSCache {}

impl DNSCache {
    pub fn new() -> Self {
        Self {
            cache: Default::default(),
        }
    }

    // if ttl is expired, remove it from cache
    pub fn get(&self, name: &Name) -> Option<Vec<SocketAddr>> {
        let cache = self.cache.read().unwrap();
        let now = Instant::now();
        let mut addrs = HashSet::new();
        if let Some(entries) = cache.get(name) {
            for e in entries {
                if let Some(expiration) = e.expiration {
                    if expiration >= now {
                        addrs.insert(e.addr);
                    }
                    if addrs.len() > 5 {
                        // enough
                        break;
                    }
                }
            }
            if entries.is_empty() {
                return None;
            }
        }
        if addrs.is_empty() {
            None
        } else {
            Some(addrs.into_iter().collect())
        }
    }

    pub fn insert(&self, name: Name, addr: SocketAddr, ttl: Option<u32>) {
        let now = Instant::now();
        let expiration =
            ttl.map(|ttl| now + Duration::from_secs(ttl.into()) + Duration::from_secs(300));
        let entry = Entry { addr, expiration };

        let mut cache = self.cache.write().unwrap();
        let entries = cache.entry(name).or_default();
        entries.retain(|e| e.expiration.is_none() || e.expiration > Some(now) || e.addr != addr);
        entries.push(entry);
    }
}

#[derive(Deserialize, Debug)]
pub struct DnsResponse {
    #[serde(rename = "Status")]
    pub status: u8,
    #[serde(rename = "TC")]
    pub truncated: bool,

    // "Always true for Google Public DNS"
    #[serde(rename = "RD")]
    pub recursion_desired: bool,
    #[serde(rename = "RA")]
    pub recursion_available: bool,

    #[serde(rename = "AD")]
    pub dnssec_validated: bool,
    #[serde(rename = "CD")]
    pub dnssec_disabled: bool,

    #[serde(rename = "Question")]
    pub question: Vec<DnsQuestion>,

    #[serde(rename = "Answer")]
    pub answer: Option<Vec<DnsAnswer>>,

    #[serde(rename = "Comment")]
    pub comment: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct DnsQuestion {
    pub name: String,
    #[serde(rename = "type")]
    pub typ: u16,
}

#[derive(Deserialize, Debug)]
pub struct DnsAnswer {
    pub name: String,
    #[serde(rename = "type")]
    pub typ: u16,
    #[serde(rename = "TTL")]
    pub ttl: Option<u32>,
    pub data: String,
}
