---
name: grafana-cli
description: Custom Rust CLI for the Grafana HTTP API. Use when listing/searching dashboards, viewing panel queries/charts in the terminal, rendering ASCII charts of Prometheus data, building reproduction shell scripts from a dashboard, or bootstrapping a local config-map cache from a Grafana endpoint.
license: MIT
metadata:
  version: "0.1"
---

# grafana-cli

> **Install**: build the binary and copy it to `/usr/local/bin/grafana-cli` so the
> commands below work without path edits.
> ```bash
> cargo build --release
> sudo install -m 0755 target/release/grafana-cli /usr/local/bin/grafana-cli
> ```

Rust CLI for the Grafana REST API. Installed binary at `/usr/local/bin/grafana-cli`. Uses `clap` for argument parsing, `reqwest`+`tokio` for async HTTP, and `serde` for payloads. Authentication is via a Grafana service-account token.

```bash
grafana-cli dashboards list
grafana-cli dashboards view <uid>
```

## Build, Test & Run

```bash
cd <source-checkout>     # repo root

cargo build --release          # → target/release/grafana-cli
sudo install -m 0755 target/release/grafana-cli /usr/local/bin/grafana-cli
cargo test                     # run all tests (token-precedence unit tests)
cargo run -- <subcommand>      # e.g.: cargo run -- dashboards find kafka
cargo clippy -- -D warnings    # lint
cargo fmt                      # auto-format
```

## Architecture

```
src/
  main.rs                  - tokio entrypoint; loads config, resolves URL/token, dispatches
  cli.rs                   - clap subcommand/argument definitions
  config.rs                - ConfigFile/Profile/ResolvedConfig; flag>env>file precedence + tests
  auth.rs                  - bearer + X-Grafana-Org-Id header builder
  error.rs                 - GrafanaError enum (thiserror)
  client/
    mod.rs                 - GrafanaClient: health, search, folders, datasources,
                             get_dashboard, ds_query_prometheus
    models.rs              - SearchHit, Folder, Datasource, DashboardEnvelope, HealthResponse
  commands/
    cache.rs               - ConfigMap TOML serializer/loader (folders, datasources, dashboards)
    init.rs                - bootstrap flow + persist URL into config.toml
    config_cmd.rs          - `config show` / `config path`
    dashboards.rs          - list / find / show / view
    generate.rs            - metric / chart / dashboard subcommands
    panel.rs               - shared helpers: extract_panels, parse_panel, build_var_map,
                             expand_vars, resolve_ds_uid
  render/
    chart.rs               - extract_series + custom ASCII renderer (min/mean/max bands)
    shell.rs               - curl/promtool/logcli command emitters
```

- **Auth**: Bearer token in `Authorization` header. Token precedence is `--token` flag > `GRAFANA_SERVICE_ACCOUNT_TOKEN` env > active profile in `~/.config/grafana-cli/config.toml`.
- **URL**: Resolved from `--url` flag > active profile > cached config map (`~/.config/grafana-cli/cache/<profile>.toml`). `init` persists the URL into the config file automatically.
- **Cache**: `init` writes a per-profile TOML cache used by `dashboards list/find` and as a datasource lookup table by `generate`/`view`. Use `--refresh` to force a live API call.

## Key Conventions

- **Error handling**: `GrafanaError` enum (thiserror) bubbled via `Result<T, GrafanaError>`; CLI top-level prints `"error: …"` and exits 1.
- **Async runtime**: `tokio` with `#[tokio::main]`; all network calls async. Config/CLI parsing is sync.
- **Output modes**: human-friendly tables (comfy-table) by default; `--output json` emits pretty JSON for `dashboards list/find/show`.
- **Concurrency**: `init` fetches dashboard details with bounded concurrency via `FuturesUnordered` + a seed/refill iterator (default 8, override `--concurrency`).
- **Template variables**: `$var`, `${var}`, `${var:format}` are expanded from `dashboard.templating.list[].current.value`; `$__all` becomes `allValue` or `.*`.
- **Datasource resolution**: panel `datasource.uid` may be a template var (e.g. `${DS_PROMETHEUS}`); resolved first via var map, then by UID/name lookup in cache, finally fallback to first `prometheus` datasource.
- **Chart aggregation**: multi-series Prometheus responses are aggregated **per timestamp** into min/mean/max bands rather than overlapping polylines.

## Implemented Commands

| Command | Description |
|---|---|
| `init [--force] [--shallow] [--no-datasources] [--concurrency N]` | Bootstrap: ping `/api/health`, fetch folders, search all dashboards, optionally fetch each dashboard's panels/datasource refs, write cache, persist URL into config |
| `config show` | Print resolved profile, URL, redacted token, token source, cache path |
| `config path` | Print cache file path for the active profile |
| `dashboards list [--folder F] [--exclude-folder F ...] [--refresh]` | List dashboards from cache (or live API) |
| `dashboards find QUERY [--exclude-folder F ...] [--refresh]` | Substring match on title/tag/UID |
| `dashboards show UID` | Metadata + per-panel summary table (id, title, type, datasource, target count) |
| `dashboards view UID [--range S] [--step S] [--filter SUBSTR] [--skip N] [--limit N]` | Browser-like view: render every panel's first query as an inline ASCII chart |
| `generate metric UID --panel N` | Print raw PromQL/LogQL (datasource header + each target expression) |
| `generate chart UID --panel N [--range S] [--step S]` | Query Prometheus via `/api/ds/query`, render ASCII chart |
| `generate dashboard UID` | Emit a runnable bash script with one labeled `curl`/`logcli` block per panel |

## Global Flags

| Flag | Description |
|---|---|
| `--config <PATH>` | Override config file path (default: `~/.config/grafana-cli/config.toml`) |
| `--profile <NAME>` | Select profile (also `GRAFANA_CLI_PROFILE` env) |
| `--url <URL>` | Override Grafana base URL |
| `--token <TOKEN>` | Override service-account token |
| `--output table\|json` | Output format (default: `table`) |
| `-v / -vv / -vvv` | Verbose logging (info / debug / trace); also obeys `RUST_LOG` |

## CLI Examples

```sh
export GRAFANA_SERVICE_ACCOUNT_TOKEN=glsa_xxxxxxxxxxxx
CLI=/usr/local/bin/grafana-cli

# Bootstrap once per endpoint — writes URL into config file
$CLI --url https://grafana.example.com init

# Faster init when you only need the dashboard index
$CLI init --shallow --no-datasources

# Browsing
$CLI dashboards list --exclude-folder sandbox --exclude-folder dev
$CLI dashboards find "kafka cluster"
$CLI dashboards show abc123uid

# Browser-like view of every panel
$CLI dashboards view abc123uid --range 21600 --step 120
$CLI dashboards view abc123uid --filter cpu --limit 5

# Inspect/query a specific panel
$CLI generate metric abc123uid --panel 429
$CLI generate chart  abc123uid --panel 429 --range 3600

# Reproduce dashboard headlessly
$CLI generate dashboard abc123uid > repro.sh && bash repro.sh
```

## Config File (`~/.config/grafana-cli/config.toml`)

```toml
default_profile = "prod"

[profiles.prod]
url    = "https://grafana.example.com"
token  = "glsa_xxxxxxxxxxxxxxxxxxxx"   # optional; falls back to GRAFANA_SERVICE_ACCOUNT_TOKEN
org_id = 1
```

`init` will create/update this file with the resolved `url` (never the token).

## Cache File (`~/.config/grafana-cli/cache/<profile>.toml`)

```toml
generated_at    = "2026-06-03T07:55:00Z"
grafana_url     = "https://grafana.example.com"
grafana_version = "13.0.1"

[folders.infra]
uid = "fld-infra"
title = "Infra"

[datasources.prom_ds_uid]
uid  = "prom_ds_uid"
name = "Prometheus"
type = "prometheus"
url  = "http://prometheus-operated.monitoring:9090"

[[dashboards]]
uid    = "abc123uid"
title  = "Kafka Cluster"
folder = "infra"
tags   = []
panels = 35
datasources = ["prom_ds_uid"]
```

## Environment Variables

| Variable | Description |
|---|---|
| `GRAFANA_SERVICE_ACCOUNT_TOKEN` | Bearer token used in API requests; overrides config file token |
| `GRAFANA_CLI_PROFILE` | Default profile name (overridden by `--profile`) |
| `RUST_LOG` | Standard tracing-subscriber filter override |

## Grafana API Endpoints Used

| Endpoint | Used by |
|---|---|
| `GET /api/health` | `init` |
| `GET /api/folders` | `init` |
| `GET /api/search?type=dash-db&limit=5000&page=N` | `init`, `dashboards list/find --refresh` |
| `GET /api/datasources` | `init` |
| `GET /api/dashboards/uid/{uid}` | `dashboards show/view`, `generate *` |
| `POST /api/ds/query` | `generate chart`, `dashboards view` |

For ad-hoc PromQL queries against a Grafana-managed Prometheus datasource (no separate CLI subcommand yet):

```bash
PROM_UID=prom_ds_uid
curl -sG -H "Authorization: Bearer $GRAFANA_SERVICE_ACCOUNT_TOKEN" \
  "$GRAFANA_URL/api/datasources/proxy/uid/$PROM_UID/api/v1/query" \
  --data-urlencode 'query=up{job="kafka"}'
```

## ASCII Chart Rendering

`render::chart::render_ascii(series, width, height)` produces a custom grid (no `textplots` at runtime — see Pitfall #1). Multi-series queries are aggregated **per timestamp**:

| Glyph | Meaning |
|---|---|
| `▴` | max series value at that timestamp |
| `▾` | min series value |
| `●` | mean across series |
| `─` | mean polyline between samples |
| `│` | full min↔max spread at that timestamp |

Footer prints `y:[min,max]   raw=<points>   timestamps=<N>   max series/ts=<N>`.

## Public Helpers (`src/commands/panel.rs`)

```rust
pub fn extract_panels(dash: &Value) -> Vec<&Value>;        // recursively collect leaf panels
pub fn parse_panel(p: &Value) -> PanelInfo;                // id/title/type/ds/targets
pub fn build_var_map(dash: &Value) -> HashMap<String, String>;
pub fn expand_vars(s: &str, vars: &HashMap<String, String>) -> String;
pub fn resolve_ds_uid(raw: &str, vars: &HashMap<String, String>,
                      cache_map: Option<&cache::ConfigMap>) -> Option<String>;
```

## Known Pitfalls

1. **`textplots` Display quirk** — `Chart::to_string()` only renders axis labels, not the canvas. The canvas is drawn inside `display()` which calls `axis()`+`figures()` first. Worse, `Chart::lineplot(&'a mut self, &'a Shape)` ties chart's `&mut` borrow to the shape's lifetime, so you can't call `axis()`/`figures()` after binding the chart to a local. We replaced `textplots` entirely with a small hand-rolled grid renderer in `render/chart.rs`.
2. **Template variable `$__all`** — Grafana stores literal `"$__all"` in `current.value` when "All" is selected. Substitute via the variable's `allValue` if set, otherwise `.*` (safe for `instance=~"..."`). See `build_var_map`.
3. **Datasource UID may be a template variable** — `panel.datasource.uid` is often `${DS_PROMETHEUS}`. Always run through `resolve_ds_uid` (var map → cache lookup → fallback to first `prometheus` DS).
4. **macOS config path** — `dirs::config_dir()` returns `~/Library/Application Support/grafana-cli/` on macOS, not `~/.config/grafana-cli/`. The skill docs say `~/.config/...` for portability; the actual path is what `config_dir()` returns.
5. **Env vars always win** — `GRAFANA_SERVICE_ACCOUNT_TOKEN` overrides the token in the config file. Only `--token` flag beats env.
6. **URL fallback from cache** — if neither `--url` nor a config profile has a URL, the CLI reads `grafana_url` from the cache map written by a previous `init`. Without any of the three, commands return `error: missing Grafana URL`.
7. **`/api/search` pagination** — caps at 5000 results per page; loop with `page=N` until an empty array is returned (see `client::search_dashboards`).
8. **Reqwest temp value lifetime** — when calling `Shape::Lines(&data)` inline, bind the `Shape` to a `let` first (the temporary dies at the semicolon). Same trap with chained builder patterns.
9. **Aggregating multi-series ASCII charts** — drawing one polyline per series (e.g. 135 Kafka brokers) produces visual noise that masks the line. Always aggregate to min/mean/max per timestamp instead.
10. **`init` persists URL but never token** — by design. Tokens should live in env or keychain; `init` only updates `[profiles.<name>].url` and `default_profile`.
11. **Concurrent dashboard fetches** — `init` uses `FuturesUnordered` with seed+refill, not `buffer_unordered`, to keep dashboards in submission order for predictable progress output (`25/347, 50/347, ...`).
12. **Non-Prometheus panels** — `generate chart` and `dashboards view` only support `prometheus`. Other datasource types (`loki`, `grafana-amazonprometheus-datasource`, SQL, etc.) print a `skip:` warning and continue.
13. **`dashboards list --refresh`** — pulls fresh `/api/search` hits but does NOT re-fetch per-dashboard details, so panel counts and datasource refs come from the cache (or are blank if no cache exists).
14. **No write operations** — the CLI is read-only; there are no create/update/delete commands for dashboards, folders, or datasources. Safe to use with any token scope.
