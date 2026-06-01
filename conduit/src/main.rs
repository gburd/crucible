// Copyright 2026 Oxide Computer Company

//! crucible-conduit: an observability conduit for a Crucible downstairs fleet.
//!
//! A fleet of `crucible-downstairs` (typically spawned by `dsc`) emits metrics
//! through Oximeter. Each downstairs is a *pull* producer: it registers with a
//! collector and then serves its samples on an OS-assigned ephemeral port. In
//! a standalone (non-Omicron) deployment there is no Nexus to register with and
//! no Oximeter to collect, so the metrics go nowhere.
//!
//! This conduit fills that gap. It impersonates the Nexus registration
//! endpoint so producers can announce their real address, periodically pulls
//! each producer's samples, and re-exposes them on a single `/metrics`
//! endpoint in Prometheus/OpenMetrics format for scraping into Grafana.
//!
//! Start each downstairs with `--oximeter <conduit_addr>` (via
//! `dsc start --oximeter <conduit_addr>`) pointing at this server's bind
//! address.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use dropshot::ConfigDropshot;
use dropshot::ConfigLogging;
use dropshot::ConfigLoggingLevel;
use dropshot::HttpServerStarter;
use slog::info;

mod collector;
mod registry;
mod render;
mod server;

use collector::SampleCache;
use registry::Registry;
use server::Conduit;

#[derive(Debug, Parser)]
#[clap(name = "crucible-conduit", term_width = 80)]
#[clap(
    about = "Observability conduit for a Crucible downstairs fleet",
    long_about = None
)]
struct Args {
    /// Address to bind the conduit on. Producers register here
    /// (POST /metrics/producers) and Prometheus scrapes GET /metrics here.
    #[clap(long, default_value = "0.0.0.0:9000", action)]
    bind: SocketAddr,

    /// How often (seconds) to pull samples from each registered producer.
    #[clap(long, default_value = "10", action)]
    collect_interval: u64,

    /// Lease duration (seconds) returned to producers on registration. They
    /// renew at roughly lease/4.
    #[clap(long, default_value = "120", action)]
    lease: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    let log = config_logging
        .to_logger("crucible-conduit")
        .context("failed to create logger")?;

    let registry = Arc::new(Registry::new());
    let cache = Arc::new(SampleCache::new());

    tokio::spawn(collector::run(
        registry.clone(),
        cache.clone(),
        Duration::from_secs(args.collect_interval),
        log.clone(),
    ));

    let conduit = Arc::new(Conduit {
        registry,
        cache,
        lease: Duration::from_secs(args.lease),
        log: log.clone(),
    });

    let config_dropshot = ConfigDropshot {
        bind_address: args.bind,
        ..Default::default()
    };

    let server =
        HttpServerStarter::new(&config_dropshot, server::build_api(), conduit, &log)
            .map_err(|e| anyhow::anyhow!("failed to create server: {e}"))?
            .start();

    info!(log, "conduit listening";
        "bind" => %args.bind,
        "collect_interval_s" => args.collect_interval);

    server
        .await
        .map_err(|e| anyhow::anyhow!("server error: {e}"))
}
