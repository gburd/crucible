// Copyright 2026 Oxide Computer Company

//! Pulls samples from every registered producer and caches the latest batch.
//!
//! Oximeter's collection model is pull-based: the collector issues
//! `GET http://{producer.address}/{producer.id}` and receives a
//! [`ProducerResults`] JSON body. We do exactly that on a fixed cadence and
//! stash the flattened samples so the `/metrics` endpoint can render them on
//! demand. A producer that fails to respond is left out of this cycle's cache
//! but stays registered, so it reappears once it is reachable again.

use std::sync::Mutex;
use std::time::Duration;

use oximeter::Sample;
use oximeter::types::ProducerResultsItem;
use slog::Logger;
use slog::{debug, warn};

use crate::registry::Registry;

/// Latest collected samples, replaced wholesale each collection cycle.
#[derive(Debug, Default)]
pub struct SampleCache {
    samples: Mutex<Vec<Sample>>,
}

impl SampleCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn store(&self, samples: Vec<Sample>) {
        *self.samples.lock().expect("sample cache poisoned") = samples;
    }

    pub fn snapshot(&self) -> Vec<Sample> {
        self.samples.lock().expect("sample cache poisoned").clone()
    }
}

/// Pull samples from one producer. Returns the producer's samples, or an empty
/// vec if it reported an error item (those carry no samples).
async fn collect_one(
    client: &reqwest::Client,
    base: &str,
    log: &Logger,
) -> anyhow::Result<Vec<Sample>> {
    let results: oximeter::types::ProducerResults =
        client.get(base).send().await?.error_for_status()?.json().await?;

    let mut samples = Vec::new();
    for item in results {
        match item {
            ProducerResultsItem::Ok(mut s) => samples.append(&mut s),
            ProducerResultsItem::Err(e) => {
                warn!(log, "producer reported collection error"; "error" => %e);
            }
        }
    }
    Ok(samples)
}

/// Run one collection cycle across all registered producers.
async fn collect_cycle(
    client: &reqwest::Client,
    registry: &Registry,
    cache: &SampleCache,
    log: &Logger,
) {
    let producers = registry.snapshot();
    let mut all = Vec::new();
    for producer in producers {
        let base = format!("http://{}/{}", producer.address, producer.id);
        match collect_one(client, &base, log).await {
            Ok(mut samples) => {
                debug!(log, "collected from producer";
                    "producer" => %producer.id,
                    "samples" => samples.len());
                all.append(&mut samples);
            }
            Err(e) => {
                warn!(log, "failed to collect from producer";
                    "producer" => %producer.id,
                    "address" => %producer.address,
                    "error" => %e);
            }
        }
    }
    cache.store(all);
}

/// Collection loop. Runs forever on `interval`, never returning.
pub async fn run(
    registry: std::sync::Arc<Registry>,
    cache: std::sync::Arc<SampleCache>,
    interval: Duration,
    log: Logger,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build reqwest client");

    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        collect_cycle(&client, &registry, &cache, &log).await;
    }
}
