//! The QuakeServers that are running, with their latest info, kept up to date by polling them.
//! Shared by the master server and the gateway.

use crate::prelude::*;
use crate::protocol;
use crate::quake_server::GAME_PORT;
use anyhow::anyhow;
use kube::runtime::reflector::{ObjectRef, Store};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::RwLock;
use tokio::sync::Notify;
use tokio::time::{timeout, Instant};
use tracing::*;

/// How often servers are queried when no heartbeats arrive.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Servers that haven't answered for this long are dropped from the list.
const EXPIRY: Duration = Duration::from_secs(90);

const QUERY_TIMEOUT: Duration = Duration::from_secs(2);

/// A running server's latest info.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Address native clients connect to, once the server has one. The master only lists
    /// servers with one.
    pub address: Option<SocketAddr>,
    /// The server's latest getinfo response.
    pub info: BTreeMap<String, String>,
    pub(crate) updated: Instant,
}

/// The running servers' info for the master and the web app, kept up to date by polling them.
pub struct Registry {
    store: Store<QuakeServer>,
    entries: RwLock<HashMap<ObjectRef<QuakeServer>, Entry>>,
    refresh: Notify,
}

impl Registry {
    pub fn new(store: Store<QuakeServer>) -> Arc<Self> {
        Arc::new(Registry {
            store,
            entries: RwLock::default(),
            refresh: Notify::new(),
        })
    }

    /// Queries the servers again soon, e.g. after one becomes ready.
    pub fn refresh(&self) {
        self.refresh.notify_one();
    }

    /// Returns the listed servers, leaving out any deleted or no longer ready since the last
    /// refresh.
    pub fn entries(&self) -> Vec<Entry> {
        self.entries
            .read()
            .unwrap()
            .iter()
            .filter(|(key, _)| {
                self.store
                    .get(key)
                    .is_some_and(|qs| qs.status.as_ref().is_some_and(|s| s.ready))
            })
            .map(|(_, entry)| entry.clone())
            .collect()
    }

    /// Returns the latest info for a server, if it's ready and answering.
    pub fn get(&self, key: &ObjectRef<QuakeServer>) -> Option<Entry> {
        self.entries.read().unwrap().get(key).cloned()
    }

    /// Queries every ready server on an interval, or soon after a heartbeat arrives.
    pub async fn poll(self: Arc<Self>) {
        // The controller also waits for its store to be ready, and kube-runtime only wakes one
        // of the waiters, so check again on an interval rather than waiting for the wakeup.
        loop {
            match timeout(Duration::from_secs(1), self.store.wait_until_ready()).await {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    error!("server list unavailable: {error}");
                    return;
                }
                Err(_) => {}
            }
        }
        loop {
            self.refresh_all().await;
            tokio::select! {
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                _ = self.refresh.notified() => {
                    // Heartbeats tend to arrive in bursts, e.g. when servers change maps
                    // together, so wait for the rest before querying.
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn refresh_all(&self) {
        let servers = self.store.state();
        let results = futures::future::join_all(servers.iter().filter_map(|qs| {
            let address = qs.status.as_ref().filter(|s| s.ready)?.address.clone();
            let namespace = qs.namespace()?;
            let name = qs.name_any();
            Some(async move {
                let result = query(&name, &namespace, address).await;
                (ObjectRef::from_obj(qs.as_ref()), result)
            })
        }))
        .await;

        let now = Instant::now();
        let mut entries = self.entries.write().unwrap();
        let previous = std::mem::take(&mut *entries);
        let queried = results.len();
        for (key, result) in results {
            match result {
                Ok(entry) => {
                    entries.insert(key, entry);
                }
                Err(error) => {
                    debug!(server = %key, "query failed: {error}");
                    // Ride out a missed reply or two before dropping the server.
                    if let Some(entry) = previous.get(&key).filter(|e| now - e.updated < EXPIRY) {
                        entries.insert(key, entry.clone());
                    }
                }
            }
        }
        debug!(
            cached = servers.len(),
            queried,
            listed = entries.len(),
            "refreshed server list"
        );
    }
}

/// Queries a server through its internal Service, since its external address may not be
/// reachable from inside the cluster.
async fn query(name: &str, namespace: &str, address: Option<String>) -> anyhow::Result<Entry> {
    let internal = lookup(&format!("{name}.{namespace}.svc:{GAME_PORT}")).await?;
    let info = protocol::get_info(internal, QUERY_TIMEOUT).await?;
    let address = match address {
        Some(address) => Some(lookup(&address).await?),
        None => None,
    };
    Ok(Entry {
        address,
        info,
        updated: Instant::now(),
    })
}

/// Resolves host:port, preferring IPv4 since every client can use it.
pub async fn lookup(addr: &str) -> anyhow::Result<SocketAddr> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(addr).await?.collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| anyhow!("{addr} has no addresses"))
}
