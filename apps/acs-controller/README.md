# ACS Controller

The `acs-controller` is the brain of the ACS platform. It subscribes to all device events emitted by the protocol pods (like `acs-cwmp` and `acs-usp`), updates the central database state, and drives device provisioning.

## Provisioning Engine

The controller features a powerful, Python-based provisioning engine. Instead of hardcoding device logic in Rust, the controller delegates decision-making to user-provided Python scripts mapped to a filesystem hierarchy.

### Directory Hierarchy

By default, the controller looks in `./provisioning` (configurable via `PROVISIONING_ROOT`). Scripts are matched based on the device's identity, cascading from the most generic (domain) to the most specific (serial number):

1. `{PROVISIONING_ROOT}/{domain_slug}/`
2. `{PROVISIONING_ROOT}/{domain_slug}/{hardware_version}/`
3. `{PROVISIONING_ROOT}/{domain_slug}/{hardware_version}/{software_version}/`
4. `{PROVISIONING_ROOT}/{domain_slug}/{hardware_version}/{software_version}/{serial_number}/`

The controller will execute **all** matching scripts it finds in these directories, in order.

### Script Naming

Scripts are named based on the event that triggered them and the lifecycle stage. For an `inform` event, the controller looks for:
- `inform_add.py`
- `inform_update.py`
- `inform_delete.py`

### Script Execution Model

When a relevant event occurs:
1. The controller spawns `python3 {script_name}`.
2. The raw JSON event payload (e.g., `InformPayload`) is written to the script's `stdin`.
3. The script analyzes the payload and prints a JSON array of actions to `stdout`.
4. The controller parses the `stdout`, wraps the actions into `DeviceCommand`s, and publishes them to NATS for the protocol pods to execute.

### Example Python Script

Here is an example `inform_update.py` that enables periodic informs:

```python
import sys
import json

# The controller passes the event payload via stdin
payload = json.load(sys.stdin)

# We can inspect the payload (e.g., payload["serial_number"], payload["parameter_list"])

# Output the actions we want the device to perform
actions = [
    {
        "SetParameterValues": {
            "parameters": {
                "Device.ManagementServer.PeriodicInformEnable": "true",
                "Device.ManagementServer.PeriodicInformInterval": "3600"
            }
        }
    }
]

# Print as a JSON array to stdout
print(json.dumps(actions))
```

## Running the Controller

### Prerequisites

- A running NATS server.
- A running PostgreSQL database with the schema initialized (see `db/apply.sh`).
- Python 3 installed on the host running the controller.

### Configuration

The controller is configured via environment variables or CLI flags:

| Environment Variable | CLI Flag | Default | Description |
|----------------------|----------|---------|-------------|
| `NATS_URL` | `--nats-url` | `nats://127.0.0.1:4222` | URL of the NATS server. |
| `DATABASE_URL` | `--database-url` | (Required) | PostgreSQL connection string. |
| `DEFAULT_DOMAIN_ID` | `--default-domain-id` | (Required) | UUID of the domain to assign to newly discovered devices. |
| `PROVISIONING_ROOT` | `--provisioning-root` | `./provisioning` | Path to the directory containing Python scripts. |
| `DB_MAX_CONNECTIONS` | `--db-max-connections` | `5` | Maximum number of open database connections. |

### Startup Example

```bash
# Get the UUID of your default domain
DEFAULT_DOMAIN=$(psql postgres://acsuser:pass@localhost:5432/acs -t -c "SELECT id FROM domains WHERE slug='default'" | xargs)

# Run the controller
DEFAULT_DOMAIN_ID=$DEFAULT_DOMAIN \
DATABASE_URL="postgres://acsuser:pass@localhost:5432/acs" \
cargo run -p acs-controller
```
