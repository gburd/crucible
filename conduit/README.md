# crucible-conduit

An observability conduit for a standalone fleet of `crucible-downstairs`.

## Why

A `crucible-downstairs` is an Oximeter *pull* producer: it registers with a
collector and then serves its samples on an OS-assigned ephemeral port. In an
Omicron deployment that collector is Nexus + Oximeter. In a standalone fleet
(for example one spawned by `dsc` for local testing), there is no Nexus to
register with and no Oximeter to collect, so the metrics go nowhere.

`crucible-conduit` fills that gap. It:

1. Serves a Nexus-compatible registration endpoint, so each downstairs can
   announce its real (ephemeral) address and producer id.
2. Periodically pulls each registered producer's samples.
3. Re-exposes the latest samples on a single `/metrics` endpoint in
   Prometheus / OpenMetrics text format, ready to scrape into Grafana.

## Usage

Start the conduit:

```
crucible-conduit --bind 0.0.0.0:9000
```

Point a `dsc`-managed fleet at it (the `--oximeter` flag tells each spawned
downstairs to register with and publish to the conduit):

```
dsc start --create --oximeter <conduit_addr> ...
```

Then scrape:

```
curl http://<conduit_addr>/metrics
```

### Flags

| Flag | Default | Description |
|---|---|---|
| `--bind` | `0.0.0.0:9000` | Address to bind. Producers register here (`POST /metrics/producers`) and Prometheus scrapes `GET /metrics` here. |
| `--collect-interval` | `10` | Seconds between pulls from each registered producer. |
| `--lease` | `120` | Lease (seconds) returned to producers on registration; they renew at roughly lease/4. |

## Endpoints

- `POST /metrics/producers` — Nexus-compatible registration. A producer sends
  its `ProducerEndpoint` (id, address, interval); the conduit records it and
  returns a `ProducerRegistrationResponse` carrying the lease duration.
- `GET /metrics` — Prometheus/OpenMetrics scrape endpoint exposing the most
  recently collected samples as `text/plain`.

## Metric mapping

Crucible downstairs emit `Cumulative<i64>` counters (connect / write / read /
flush), which are rendered as Prometheus `counter`s. Other scalar datum types
are rendered as `gauge`s. Histograms, byte blobs, strings, and missing data
have no single scalar value and are skipped. The Oximeter timeseries name
(`target:metric`) is sanitized into a valid Prometheus name (the `:` separator
becomes `_`). Each sample line carries its measurement timestamp in
milliseconds.

## Limitations

- A producer that fails to respond during a collection cycle is omitted from
  that cycle's output but stays registered, so it reappears once reachable.
- The conduit holds only the most recently collected batch; it is a scrape
  bridge, not a time-series store. Durable history lives in whatever scrapes
  `/metrics` (Prometheus).
