"""
acs_sdk.py — Shared helper library for ACS provisioning scripts.

Every provisioning script should import this module to:
  - Parse the InformPayload from stdin.
  - Build well-typed action dicts.
  - Emit actions to stdout.

Usage pattern
-------------
  import sys, os
  sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
  from acs_sdk import load_payload, set_parameter_values, emit_actions

  payload = load_payload()
  if payload.has_event("0 BOOTSTRAP"):
      emit_actions([
          set_parameter_values({"Device.Foo.Bar": "baz"})
      ])
"""

import json
import sys
from dataclasses import dataclass, field
from typing import Any


# ── Payload ──────────────────────────────────────────────────────────────────

@dataclass
class InformPayload:
    """Typed wrapper around the JSON payload the controller sends via stdin."""
    session_id: str
    device_id: str
    oui: str
    serial_number: str
    manufacturer: str
    product_class: str
    events: list[str]
    parameter_list: dict[str, str]
    protocol: str = "cwmp"

    # ── Convenience helpers ──────────────────────────────────────────────────

    def has_event(self, event_code: str) -> bool:
        """Return True if *event_code* appears in the events list.

        Matching is case-insensitive and strips surrounding whitespace so
        both ``"0 BOOTSTRAP"`` and ``"0 bootstrap"`` work.
        """
        needle = event_code.strip().lower()
        return any(e.strip().lower() == needle for e in self.events)

    def param(self, path: str, default: str | None = None) -> str | None:
        """Look up a parameter by TR-181 or TR-098 path."""
        return self.parameter_list.get(path, default)

    def software_version(self) -> str | None:
        return (
            self.param("Device.DeviceInfo.SoftwareVersion")
            or self.param("InternetGatewayDevice.DeviceInfo.SoftwareVersion")
        )

    def hardware_version(self) -> str | None:
        return (
            self.param("Device.DeviceInfo.HardwareVersion")
            or self.param("InternetGatewayDevice.DeviceInfo.HardwareVersion")
        )


def load_payload() -> InformPayload:
    """Parse the JSON InformPayload from stdin and return a typed object."""
    raw = json.load(sys.stdin)
    return InformPayload(
        session_id=raw["session_id"],
        device_id=raw["device_id"],
        oui=raw["oui"],
        serial_number=raw["serial_number"],
        manufacturer=raw["manufacturer"],
        product_class=raw["product_class"],
        events=raw.get("events", []),
        parameter_list=raw.get("parameter_list", {}),
        protocol=raw.get("protocol", "cwmp"),
    )


# ── Action builders ───────────────────────────────────────────────────────────

def set_parameter_values(parameters: dict[str, str]) -> dict[str, Any]:
    """Build a SetParameterValues action."""
    return {"SetParameterValues": {"parameters": parameters}}


def get_parameter_values(paths: list[str]) -> dict[str, Any]:
    """Build a GetParameterValues action."""
    return {"GetParameterValues": {"paths": paths}}


def get_parameter_names(path_prefix: str, next_level: bool = True) -> dict[str, Any]:
    """Build a GetParameterNames action."""
    return {"GetParameterNames": {"path_prefix": path_prefix, "next_level": next_level}}


def add_object(path: str) -> dict[str, Any]:
    """Build an AddObject action."""
    return {"AddObject": {"path": path}}


def delete_object(path: str) -> dict[str, Any]:
    """Build a DeleteObject action."""
    return {"DeleteObject": {"path": path}}


def reboot() -> dict[str, Any]:
    """Build a Reboot action."""
    return {"Reboot": {}}


def factory_reset() -> dict[str, Any]:
    """Build a FactoryReset action."""
    return {"FactoryReset": {}}


def download(url: str, file_type: str, file_size: int = 0, target_filename: str = "") -> dict[str, Any]:
    """Build a Download action."""
    return {"Download": {
        "url": url,
        "file_type": file_type,
        "file_size": file_size,
        "target_filename": target_filename,
    }}


# ── Output ────────────────────────────────────────────────────────────────────

def emit_actions(actions: list[dict[str, Any]]) -> None:
    """Serialise *actions* as JSON to stdout and exit 0.

    The controller expects exactly one JSON array on stdout.  Call this once
    at the end of your script.  If you have no actions to emit, call with an
    empty list (or simply do not call at all — an empty stdout is also valid).
    """
    print(json.dumps(actions))
    sys.exit(0)


def emit_no_actions() -> None:
    """Signal that this script has no actions to perform and exit cleanly."""
    emit_actions([])
