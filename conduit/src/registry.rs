// Copyright 2026 Oxide Computer Company

//! Tracks the set of Oximeter producers (crucible-downstairs) that have
//! registered with the conduit.
//!
//! A downstairs started with `--oximeter <conduit_addr>` POSTs a
//! [`ProducerEndpoint`] to `/metrics/producers`. The endpoint carries the
//! producer's UUID and the *actual* address+port its collect server bound to
//! (the downstairs binds port 0, so this is the only way to learn the real
//! port). We record it here so the collector can later pull
//! `GET http://{address}/{producer_id}`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Duration;

use omicron_common::api::internal::nexus::ProducerEndpoint;
use uuid::Uuid;

/// One registered producer and the moment we last heard from it.
#[derive(Clone, Debug)]
pub struct Producer {
    pub id: Uuid,
    pub address: SocketAddr,
    pub interval: Duration,
}

/// Shared, mutable set of registered producers keyed by producer UUID.
///
/// Re-registration (the producer renews its lease periodically) simply
/// overwrites the existing entry, which also picks up an address change after
/// a downstairs restart lands on a different ephemeral port.
#[derive(Debug, Default)]
pub struct Registry {
    producers: Mutex<HashMap<Uuid, Producer>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record (or refresh) a producer from its registration endpoint.
    pub fn upsert(&self, endpoint: &ProducerEndpoint) {
        let producer = Producer {
            id: endpoint.id,
            address: endpoint.address,
            interval: endpoint.interval,
        };
        self.producers
            .lock()
            .expect("registry mutex poisoned")
            .insert(endpoint.id, producer);
    }

    /// Snapshot of all currently known producers.
    pub fn snapshot(&self) -> Vec<Producer> {
        self.producers
            .lock()
            .expect("registry mutex poisoned")
            .values()
            .cloned()
            .collect()
    }
}
