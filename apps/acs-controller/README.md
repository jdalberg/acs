# ACS Controller

The `acs-controller` is the brain of the ACS platform. It subscribes to all device
events emitted by the protocol pods (like `acs-cwmp` and `acs-usp`), updates the
central database state, and drives device provisioning.

---

## Provisioning Engine

The controller features a Python-based provisioning engine. Instead of hardcoding
device logic in Rust, the controller delegates decision-making to user-provided Python
scripts discovered from a filesystem hierarchy.

### Directory Hierarchy

By default the controller looks in `./provisioning` (configurable via `PROVISIONING_ROOT`).
**Event type is the top-level directory**; scripts are then matched by domain, hardware
version, software version, and finally individual device — from most generic to most
specific:

```
{PROVISIONING_ROOT}/
└── {event_type}/                    e.g. "inform"
    └── {domain_slug}/               e.g. "default", "acme"
        ├── add.py                   ← new device
        ├── update.py                ← known device (returning Inform)
        ├── delete.py                ← decommission (future internal event)
        └── {hardware_version}/
            ├── add.py
            └── {software_version}/
                ├── add.py
                └── {oui}-{serial}/  e.g. "AABB00-1234567"
                    └── add.py
```

The controller executes **all** scripts that exist at each matching level (general → specific)
and merges their returned actions into a single list.

#### Domain slug note

Newly discovered devices are placed in the `default` domain until a validation step
assigns them to their real domain. Scripts in `inform/default/` therefore act as the
universal bootstrap layer.

### Script Naming

| File | When it runs |
|------|-------------|
| `add.py` | Device is new (first Inform seen by this ACS) |
| `update.py` | Device is known (subsequent Informs) |
| `delete.py` | Reserved for a future internal decommission event |

### Script Execution Model

For every matching script the controller:

1. Spawns `python3 {script_path}`.
2. Writes the raw JSON event payload (`InformPayload`) to the script's **stdin**.
3. Reads a JSON array of actions from **stdout**.
4. Wraps the actions into `DeviceCommand`s and publishes them to NATS for the
   appropriate protocol pod to execute.

A non-zero exit code is logged and the script is skipped — subsequent scripts still run.

### Using the SDK

Every script should import `acs_sdk` from the provisioning root:

```python
import sys, os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

from acs_sdk import load_payload, set_parameter_values, emit_actions, emit_no_actions

payload = load_payload()

if not payload.has_event("0 BOOTSTRAP"):
    emit_no_actions()

emit_actions([
    set_parameter_values({
        "Device.ManagementServer.PeriodicInformEnable":   "true",
        "Device.ManagementServer.PeriodicInformInterval": "3600",
    })
])
```

See [`provisioning/README.md`](../../provisioning/README.md) for the full SDK reference
and available action builders.

---

## HTTP API

The controller exposes a synchronous HTTP API for inventory management and real-time
device commands. CORS is enabled for all origins to support GUI development.

Base URL: `http://{host}:{API_PORT}/api/v1`

---

### Inventory — Devices

#### `GET /inventory/devices`

List all devices. Optionally filter by domain slug.

| Query param | Example | Description |
|-------------|---------|-------------|
| `domain` | `?domain=acme` | Return only devices in this domain |

**Response `200`** — array of device objects:

```json
[
  {
    "id":               "uuid",
    "device_uid":       "AABB00-1234567",
    "domain_id":        "uuid",
    "manufacturer":     "ExampleCorp",
    "oui":              "AABB00",
    "product_class":    "ExampleDevice",
    "serial_number":    "1234567",
    "current_protocol": "cwmp",
    "software_version": "1.2.3",
    "hardware_version": "1.0",
    "tags":             ["production", "site-a"],
    "metadata":         {"location": "rack-3"},
    "first_seen":       "2026-01-01T00:00:00Z",
    "last_seen":        "2026-05-26T04:00:00Z"
  }
]
```

---

#### `GET /inventory/devices/:uid`

Get a single device by `device_uid` (e.g. `AABB00-1234567`).

**Response `200`** — single device object (same shape as above).  
**Response `404`** — device not found.

---

#### `PATCH /inventory/devices/:uid`

Update mutable inventory fields. All body fields are optional; only supplied fields
are modified.

**Request body:**
```json
{
  "tags":      ["production", "site-a"],
  "metadata":  {"location": "rack-3"},
  "domain_id": "uuid"
}
```

**Response `204`** — updated.  
**Response `400`** — empty body (nothing to update).  
**Response `404`** — device not found.

> **Note:** `domain_id` can be used to move a device to a different domain after
> validation. The target domain must already exist.

---

#### `DELETE /inventory/devices/:uid`

Hard-delete a device. Cascade rules automatically remove all associated
`device_protocols` and `device_properties` rows.

**Response `204`** — deleted.  
**Response `404`** — device not found.

---

### Inventory — Device Properties

Custom key/value properties attached to a device. Used as input for desired-state
config generation.

#### `GET /inventory/devices/:uid/properties`

List all properties for a device.

**Response `200`:**
```json
[
  {
    "property_name":  "ntp_server",
    "property_value": "pool.ntp.org",
    "source":         "api",
    "priority":       100,
    "created_at":     "...",
    "updated_at":     "..."
  }
]
```

---

#### `PUT /inventory/devices/:uid/properties/:name`

Set (upsert) a property. If it already exists the value, source, and priority are
updated.

**Request body:**
```json
{
  "value":    "pool.ntp.org",
  "source":   "api",
  "priority": 100
}
```

`source` defaults to `"api"` if omitted. `priority` defaults to `100`.  
Lower priority number = higher precedence during config generation.

**Response `204`** — set.  
**Response `404`** — device not found.

---

#### `DELETE /inventory/devices/:uid/properties/:name`

Remove a property from a device.

**Response `204`** — deleted.  
**Response `404`** — device or property not found.

---

### Inventory — Device Protocols

Read-only view of the protocol connection details the ACS has recorded for a device.

#### `GET /inventory/devices/:uid/protocols`

**Response `200`:**
```json
[
  {
    "protocol":               "cwmp",
    "endpoint_id":            null,
    "connection_request_url": "http://192.168.1.10:7547/connection",
    "username":               null,
    "last_session_at":        "2026-05-26T04:00:00Z",
    "metadata":               {}
  }
]
```

---

### Inventory — Domains

#### `GET /inventory/domains`

List all domains.

**Response `200`:**
```json
[
  {
    "id":          "uuid",
    "name":        "Default",
    "slug":        "default",
    "description": null,
    "created_at":  "...",
    "updated_at":  "..."
  }
]
```

---

#### `GET /inventory/domains/:slug`

Get a single domain by slug.

**Response `200`** — single domain object.  
**Response `404`** — not found.

---

#### `POST /inventory/domains`

Create a new domain.

**Request body:**
```json
{
  "name":        "Acme Corp",
  "slug":        "acme",
  "description": "Optional description"
}
```

Slug must match `^[a-z0-9][a-z0-9\-]*[a-z0-9]$`.

**Response `201`** — created domain object.  
**Response `409`** — name or slug already exists.  
**Response `422`** — slug format invalid.

---

#### `PATCH /inventory/domains/:slug`

Update a domain's `name` and/or `description`. The slug itself is immutable.

**Request body (all optional):**
```json
{
  "name":        "Acme Corporation",
  "description": "Updated description"
}
```

**Response `204`** — updated.  
**Response `404`** — not found.  
**Response `409`** — name already taken.

---

#### `DELETE /inventory/domains/:slug`

Delete a domain.

**Response `204`** — deleted.  
**Response `404`** — not found.  
**Response `409`** — domain still has devices assigned (move or delete devices first).

---

### Device Commands

#### `POST /device/:uid/command`

Send a real-time command to a connected device and synchronously await its response.

**Request body** — a `nats_common::Action`, for example:
```json
{"GetParameterValues": {"paths": ["Device.DeviceInfo."]}}
```

**Behavior:**
1. Checks if the device has an active session. If not, attempts a connection request
   and polls for up to 15 seconds.
2. Publishes the command to NATS (`acs.sessions.{session_id}.command`).
3. Awaits the `command_response` on the NATS event stream.
4. Returns the device response, or `504 Gateway Timeout` after 30 seconds.

**Response `200`** — device response payload.  
**Response `504`** — device offline or did not respond in time.

---

## Running the Controller

### Prerequisites

- A running NATS server.
- A running PostgreSQL database with schema applied (see `db/apply.sh`).
- Python 3 available on the host (for provisioning scripts).

### Configuration

| Environment Variable | CLI Flag | Default | Description |
|----------------------|----------|---------|-------------|
| `NATS_URL` | `--nats-url` | `nats://127.0.0.1:4222` | NATS server URL |
| `DATABASE_URL` | `--database-url` | *(required)* | PostgreSQL connection string |
| `DEFAULT_DOMAIN_ID` | `--default-domain-id` | *(required)* | UUID of the domain for newly discovered devices |
| `PROVISIONING_ROOT` | `--provisioning-root` | `./provisioning` | Path to provisioning scripts |
| `API_PORT` | `--api-port` | `8080` | HTTP API listen port |
| `DB_MAX_CONNECTIONS` | `--db-max-connections` | `5` | PostgreSQL connection pool size |

### Startup Example

```bash
# Get the UUID of your default domain
DEFAULT_DOMAIN=$(psql postgres://acsuser:pass@localhost:5432/acs -t -c \
  "SELECT id FROM domains WHERE slug='default'" | xargs)

# Run the controller
DEFAULT_DOMAIN_ID=$DEFAULT_DOMAIN \
DATABASE_URL="postgres://acsuser:pass@localhost:5432/acs" \
cargo run -p acs-controller
```
