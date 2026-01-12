# Build and Run

## Build
```bash
cargo build --release
```

## Install
```bash
sudo install -m 755 target/release/dev-backup /usr/local/bin/
```

## Config
Copy the template and adjust paths/credentials:
```bash
sudo mkdir -p /etc/dev-backup
sudo cp docs/config.example.toml /etc/dev-backup/config.toml
```

## Initialize LS
```bash
sudo dev-backup --config /etc/dev-backup/config.toml init ls
```

## Initialize WS
```bash
dev-backup --config /etc/dev-backup/config.toml init ws
```
