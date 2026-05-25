# MVT Server on FreeBSD

This guide provides instructions for deploying MVT Server as a system service on FreeBSD.

## Directory Structure

Following FreeBSD `hier(7)` conventions:

| Location | Purpose |
| :--- | :--- |
| `/usr/local/bin/mvt-server` | The binary executable |
| `/usr/local/etc/mvt-server/` | Configuration files (`.toml`, `.json`, `.env`, `mvtrs.db`, `config.yaml`) |
| `/usr/local/etc/rc.d/mvtserver` | The service script |
| `/var/cache/mvt-server/` | Tile cache storage |
| `/usr/local/etc/mvt-server/assets/` | Static map assets |
| `/usr/local/etc/mvt-server/plugins/` | Lua plugin files (`.lua`) |
| `/var/log/mvt-server.log` | Service logs |

## Installation Steps

1. **Install Binary**:
   Copy the compiled binary to `/usr/local/bin/mvt-server` and ensure it is executable:
   ```sh
   cp target/release/mvt-server /usr/local/bin/mvt-server
   chmod +x /usr/local/bin/mvt-server
   ```

2. **Setup Directories**:
   ```sh
   mkdir -p /usr/local/etc/mvt-server/assets
   mkdir -p /usr/local/etc/mvt-server/plugins
   mkdir -p /var/cache/mvt-server
   ```

3. **Install Service Script**:
   Copy the `mvtserver` script to `/usr/local/etc/rc.d/` and ensure it is executable:
   ```sh
   cp freebsd/rc.d/mvtserver /usr/local/etc/rc.d/
   chmod +x /usr/local/etc/rc.d/mvtserver
   ```

4. **Configuration**:
   - Place your configuration files and `mvtrs.db` in `/usr/local/etc/mvt-server/`.
   - Create your `.env` file in `/usr/local/etc/mvt-server/.env`.

5. **Enable Service**:
   Add the following to `/etc/rc.conf`:
   ```sh
   mvtserver_enable="YES"
   ```

6. **Start Service**:
   ```sh
   service mvtserver start
   ```
