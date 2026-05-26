#!/usr/bin/env python3
"""
provisioning/inform/default/add.py
====================================
Runs for every NEW device (first Inform ever seen by this ACS).

On a BOOTSTRAP event this script performs an initial configuration push by
emitting a SetParameterValues action that the acs-cwmp pod will translate into
a TR-069 SetParameterValues RPC sent back to the device.

This is the "hello world" of provisioning — the simplest possible working
example.  Extend it or add sibling / child scripts for more specific logic.
"""

import sys
import os

# Allow importing acs_sdk from the provisioning root regardless of cwd.
# This script lives three levels deep: inform/default/add.py
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

from acs_sdk import (
    load_payload,
    set_parameter_values,
    emit_actions,
    emit_no_actions,
)

# ── Load the payload the controller fed us via stdin ──────────────────────────

payload = load_payload()

# ── Decision logic ────────────────────────────────────────────────────────────
#
# We only want to push initial config on the very first contact (BOOTSTRAP).
# Subsequent periodic Informs from existing devices will hit update.py,
# NOT this file, so the check is extra safety rather than the primary guard.

if not payload.has_event("0 BOOTSTRAP"):
    # Not a bootstrap — nothing to do from this script.
    emit_no_actions()

# ── Build the actions we want the device to execute ───────────────────────────
#
# Hello-world payload: enable periodic informs every hour and stamp a custom
# management tag so we can see in logs that provisioning ran.
#
# Replace / extend these parameters with whatever your real devices need.

actions = [
    set_parameter_values({
        # Enable scheduled check-ins
        "Device.ManagementServer.PeriodicInformEnable":   "true",
        "Device.ManagementServer.PeriodicInformInterval": "3600",

        # A simple breadcrumb so you can see provisioning happened
        "Device.DeviceInfo.ProvisioningCode": f"acs-bootstrap-{payload.serial_number}",
    })
]

# ── Emit ──────────────────────────────────────────────────────────────────────

emit_actions(actions)
