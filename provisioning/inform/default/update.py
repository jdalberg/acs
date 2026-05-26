#!/usr/bin/env python3
"""
provisioning/inform/default/update.py
=======================================
Runs every time a KNOWN device sends an Inform (periodic, boot, etc.).

This stub currently emits no actions.  Add logic here when you want to
re-apply or verify config on returning devices.
"""

import sys
import os

# This script lives three levels deep: inform/default/update.py
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

from acs_sdk import load_payload, emit_no_actions

payload = load_payload()

# Example: uncomment to log (to stderr — stdout is reserved for JSON actions)
# print(f"[update] {payload.device_id} events={payload.events}", file=sys.stderr)

emit_no_actions()
