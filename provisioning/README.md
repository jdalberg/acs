# Provisioning Scripts

This directory contains the Python provisioning scripts that `acs-controller`
executes in response to device events.

## Directory Layout

```
provisioning/
├── acs_sdk.py                      # Shared helper library — import in every script
├── test_bootstrap_provisioning.py  # Smoke-test (no NATS/DB needed)
│
└── {event_type}/                   # Top-level: one folder per event type
    └── {domain_slug}/              # Domain the device is currently assigned to
        ├── add.py                  # New device  (first Inform / device added)
        ├── update.py               # Known device (subsequent Informs)
        ├── delete.py               # Decommission (future internal/API event)
        │
        └── {hw_version}/           # (optional) narrower scope: hardware version
            ├── add.py
            └── {sw_version}/       # (optional) narrower scope: software version
                ├── add.py
                └── {oui}-{serial}/ # (optional) device-specific overrides
                    └── add.py
```

### Execution order

The controller walks **all** matching directories from most-general to
most-specific and runs every script it finds.  At each level it tries
`add.py`, `update.py`, `delete.py` in order — missing files are silently
skipped.  All returned actions are merged into a single list that is sent
back to the device in the same CWMP/USP session.

### Domain slug

Newly-seen devices land in `default` until a domain-validation step assigns
them to their real domain.  Scripts in `inform/default/` therefore act as the
catch-all bootstrap layer.

## Script Contract

| Aspect | Detail |
|--------|--------|
| **stdin** | `InformPayload` JSON object (see `acs_sdk.InformPayload`) |
| **stdout** | JSON array of `Action` objects, or empty |
| **stderr** | Free-form logging (controller captures it on error) |
| **exit code** | `0` = success; non-zero = controller logs error and skips this script |

## Available Actions

```python
set_parameter_values({"Device.Foo": "bar"})
get_parameter_values(["Device.Foo.", "Device.Bar."])
get_parameter_names("Device.Foo.", next_level=True)
add_object("Device.Hosts.Host.")
delete_object("Device.Hosts.Host.1.")
reboot()
factory_reset()
download(url="http://...", file_type="1 Firmware Upgrade Image")
```

## Running the smoke-test

```bash
python3 provisioning/test_bootstrap_provisioning.py
```

No running services required — the test harness pipes a fake InformPayload
directly to the scripts and validates the output.

## Hello World — what `inform/default/add.py` does

When a device sends its very first Inform and includes the `0 BOOTSTRAP` event,
the controller runs `add.py` and sends the resulting `SetParameterValues` back
to the device during the same CWMP session:

```
Device ──Inform (0 BOOTSTRAP)──► acs-cwmp ──NATS──► acs-controller
                                                          │
                              inform/default/add.py  ◄────┘
                                          │
                              SetParameterValues
                           PeriodicInformEnable = true
                          PeriodicInformInterval = 3600
                           ProvisioningCode = acs-bootstrap-<serial>
                                          │
acs-cwmp ──SetParameterValues RPC──► Device
```

## Adding your own logic

```
provisioning/
└── inform/
    └── acme/                   ← your real domain slug
        ├── add.py              ← all new Acme devices
        └── HW-2.0/             ← only HW revision 2.0
            └── add.py
```

Drop a script anywhere in the tree — the controller picks it up automatically
on the next restart (or live, if you set `PROVISIONING_ROOT` to a hot path).
