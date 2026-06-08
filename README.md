# grafana-cli

A small Rust CLI for [Grafana](https://grafana.com/). It can:

- 🔑 Authenticate with a service-account token from a config file **or** the
  `GRAFANA_SERVICE_ACCOUNT_TOKEN` environment variable (CLI flag > env > config).
- 🚀 `init` — probe Grafana, walk all dashboards, and write a local **config map**
  cache at `~/.config/grafana-cli/cache/<profile>.toml`.
- 📂 `dashboards list | find | show` — browse the cache or the live API.
- 🛠️ `generate metric | chart | dashboard` — emit raw PromQL/LogQL for a panel,
  ASCII-render a Prometheus chart, or produce a runnable shell script that
  reproduces every panel of a dashboard via `curl` / `logcli`.

## Install

```bash
cargo build --release
sudo install -m 0755 target/release/grafana-cli /usr/local/bin/grafana-cli
grafana-cli --help
```

## Configure

`~/.config/grafana-cli/config.toml`:

```toml
default_profile = "prod"

[profiles.prod]
url = "https://grafana.example.com"
# token may be omitted — falls back to GRAFANA_SERVICE_ACCOUNT_TOKEN
token = "glsa_xxxxxxxxxxxxxxxxxxxx"
```

Or skip the file entirely:

```bash
export GRAFANA_SERVICE_ACCOUNT_TOKEN=glsa_...
grafana-cli --url https://grafana.example.com init
```

## Common usage

```bash
grafana-cli init                          # build local cache
grafana-cli dashboards list               # list from cache
grafana-cli dashboards find "node"        # fuzzy search
grafana-cli dashboards show abc123        # panel inspector
grafana-cli generate metric abc123 --panel 4
grafana-cli generate chart  abc123 --panel 4 --range 3600
grafana-cli generate dashboard abc123 > repro.sh
```

See `plan.html` for the full design.
