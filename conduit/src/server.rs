// Copyright 2026 Oxide Computer Company

//! Dropshot server exposing the two faces of the conduit:
//!
//! * `POST /metrics/producers` — the Nexus-compatible registration endpoint
//!   that crucible-downstairs producers call. We answer with a
//!   [`ProducerRegistrationResponse`] carrying a lease duration, exactly as
//!   Nexus would, so the producer's renew loop is satisfied.
//! * `GET /metrics` — Prometheus/OpenMetrics scrape endpoint exposing the
//!   most recently collected samples.

use std::sync::Arc;

use dropshot::ApiDescription;
use dropshot::Body;
use dropshot::HttpError;
use dropshot::HttpResponseCreated;
use dropshot::RequestContext;
use dropshot::TypedBody;
use dropshot::endpoint;
use http::Response;
use omicron_common::api::internal::nexus::ProducerEndpoint;
use omicron_common::api::internal::nexus::ProducerRegistrationResponse;
use slog::Logger;
use slog::info;

use crate::collector::SampleCache;
use crate::registry::Registry;
use crate::render;

/// Shared state for the conduit's HTTP handlers.
pub struct Conduit {
    pub registry: Arc<Registry>,
    pub cache: Arc<SampleCache>,
    /// Lease duration handed back to producers on registration. They renew at
    /// roughly lease/4, so this also bounds how stale the producer set gets.
    pub lease: std::time::Duration,
    pub log: Logger,
}

pub fn build_api() -> ApiDescription<Arc<Conduit>> {
    let mut api = ApiDescription::new();
    api.register(register_producer).unwrap();
    api.register(metrics).unwrap();
    api
}

/// Nexus-compatible producer registration. The producer announces its UUID and
/// the real address+port its collect server bound to; we record it and return
/// a lease so the producer keeps renewing.
#[endpoint {
    method = POST,
    path = "/metrics/producers",
}]
async fn register_producer(
    rqctx: RequestContext<Arc<Conduit>>,
    body: TypedBody<ProducerEndpoint>,
) -> Result<HttpResponseCreated<ProducerRegistrationResponse>, HttpError> {
    let conduit = rqctx.context();
    let endpoint = body.into_inner();
    info!(conduit.log, "producer registered";
        "producer" => %endpoint.id,
        "address" => %endpoint.address);
    conduit.registry.upsert(&endpoint);
    Ok(HttpResponseCreated(ProducerRegistrationResponse {
        lease_duration: conduit.lease,
    }))
}

/// Prometheus/OpenMetrics scrape endpoint. Returns the latest collected
/// samples as a `text/plain` exposition document.
#[endpoint {
    method = GET,
    path = "/metrics",
}]
async fn metrics(
    rqctx: RequestContext<Arc<Conduit>>,
) -> Result<Response<Body>, HttpError> {
    let conduit = rqctx.context();
    let samples = conduit.cache.snapshot();
    let document = render::to_prometheus(&samples);
    Response::builder()
        .status(http::StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(document.into())
        .map_err(|e| HttpError::for_internal_error(e.to_string()))
}
