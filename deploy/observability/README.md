# Crucible observability stack (Prometheus + Grafana)

A generic Prometheus + Grafana stack that scrapes a running
[`crucible-conduit`](../../conduit/README.md) and visualizes the downstairs
fleet's metrics. Everything here is a template — there are no real addresses or
credentials. Substitute your own before deploying.

## Layout

```
docker-compose.yml                              Prometheus + Grafana services
prometheus.yml                                  scrape config (templated target)
grafana/provisioning/datasources/prometheus.yml Grafana -> Prometheus wiring
grafana/provisioning/dashboards/dashboards.yml  dashboard provider
grafana/dashboards/crucible-downstairs.json     starter dashboard
```

## Quick start

1. Point the scrape config at your conduit's `--bind` address by replacing the
   `__CONDUIT_ADDR__` placeholder in `prometheus.yml`:

   ```
   sed -i 's/__CONDUIT_ADDR__/HOST:PORT/' prometheus.yml
   ```

   (e.g. `HOST:PORT` = the address you passed to `crucible-conduit --bind` and
   to `dsc start --oximeter`.)

2. Set Grafana admin credentials (don't ship the `admin`/`admin` default on a
   shared network):

   ```
   export GF_ADMIN_USER=... GF_ADMIN_PASSWORD=...
   ```

3. Bring it up:

   ```
   docker compose up -d
   ```

   - Prometheus: http://localhost:9090
   - Grafana: http://localhost:3000 (dashboard "Crucible Downstairs" is
     auto-provisioned under the Crucible folder)

## Deploying on TrueNAS SCALE

TrueNAS SCALE runs Docker apps. Install this as a **Custom App → Install via
YAML** using `docker-compose.yml`, or run it under `dockge`/`portainer`. The
provisioning and dashboard files must be reachable at the bind-mount paths in
the compose file, so copy this `deploy/observability/` directory to a dataset
and adjust the volume source paths to that location.

## Dashboard

The starter dashboard charts the four counters the conduit exposes, broken out
by `downstairs_uuid` (a templated multi-select variable):

| Metric | Panel |
|---|---|
| `crucible_downstairs_write` | Write ops/sec (`rate(...[5m])`) |
| `crucible_downstairs_read` | Read ops/sec |
| `crucible_downstairs_flush` | Flush ops/sec |
| `crucible_downstairs_connect` | Cumulative connects |

These names come from the downstairs Oximeter target/metric
(`CrucibleDownstairs` / `Connect`,`Write`,`Read`,`Flush`) as rendered by the
conduit (`:` separator becomes `_`).

## Note

This stack has not been smoke-tested end-to-end in CI; it is a starting point.
Verify scrape health on the Prometheus *Targets* page and that the Grafana
datasource resolves before relying on the dashboards.
