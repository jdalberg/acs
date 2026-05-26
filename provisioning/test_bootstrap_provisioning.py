#!/usr/bin/env python3
"""
test_bootstrap_provisioning.py
================================
Standalone smoke-test for the provisioning scripts.

Simulates what acs-controller does:
  1. Serialises a fake InformPayload to JSON.
  2. Pipes it to the target script via stdin.
  3. Parses stdout as a JSON action array.
  4. Prints the result.

Run from the repo root:
  python3 provisioning/test_bootstrap_provisioning.py
"""

import json
import subprocess
import sys
from pathlib import Path

PROVISIONING_ROOT = Path(__file__).parent
SCRIPT            = PROVISIONING_ROOT / "inform" / "default" / "add.py"

# ── Helper ────────────────────────────────────────────────────────────────────

def run_script(script: Path, payload: dict) -> list:
    """Execute *script* with *payload* on stdin; return parsed actions."""
    result = subprocess.run(
        [sys.executable, str(script)],
        input=json.dumps(payload).encode(),
        capture_output=True,
    )

    if result.returncode != 0:
        stderr = result.stderr.decode(errors="replace")
        raise RuntimeError(
            f"Script exited with code {result.returncode}:\n{stderr}"
        )

    stdout = result.stdout.strip()
    if not stdout:
        return []

    return json.loads(stdout)


# ── Test cases ────────────────────────────────────────────────────────────────

def make_payload(events: list[str]) -> dict:
    return {
        "session_id":     "test-session-abc123",
        "device_id":      "AABB00-1234567",
        "oui":            "AABB00",
        "serial_number":  "1234567",
        "manufacturer":   "ExampleCorp",
        "product_class":  "ExampleDevice",
        "events":         events,
        "parameter_list": {
            "Device.DeviceInfo.SoftwareVersion": "1.2.3",
            "Device.DeviceInfo.HardwareVersion": "1.0",
            "Device.ManagementServer.ConnectionRequestURL":
                "http://192.168.1.10:7547/connection",
        },
        "protocol": "cwmp",
    }


def test_bootstrap():
    print("=== TEST: BOOTSTRAP event ===")
    payload = make_payload(["0 BOOTSTRAP", "1 BOOT"])
    actions = run_script(SCRIPT, payload)
    assert actions, "Expected at least one action for BOOTSTRAP"
    assert actions[0].get("SetParameterValues"), "Expected SetParameterValues action"
    params = actions[0]["SetParameterValues"]["parameters"]
    assert "Device.ManagementServer.PeriodicInformEnable" in params
    assert params["Device.ManagementServer.PeriodicInformEnable"] == "true"
    print("  Actions returned:", json.dumps(actions, indent=2))
    print("  ✓ PASSED\n")


def test_periodic_no_bootstrap():
    print("=== TEST: Periodic inform (no BOOTSTRAP) ===")
    payload = make_payload(["2 PERIODIC"])
    actions = run_script(SCRIPT, payload)
    assert actions == [], f"Expected no actions for periodic, got {actions}"
    print("  Actions returned: []")
    print("  ✓ PASSED\n")


def test_boot_no_bootstrap():
    print("=== TEST: Boot event only (no BOOTSTRAP) ===")
    payload = make_payload(["1 BOOT"])
    actions = run_script(SCRIPT, payload)
    assert actions == [], f"Expected no actions for plain boot, got {actions}"
    print("  Actions returned: []")
    print("  ✓ PASSED\n")


# ── Runner ────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    try:
        test_bootstrap()
        test_periodic_no_bootstrap()
        test_boot_no_bootstrap()
        print("All tests passed ✓")
    except AssertionError as e:
        print(f"\nFAIL: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"\nERROR: {e}")
        sys.exit(1)
