# ACS Database Schema

## Dependency / creation order

```
1. domains
2. users
3. domain_memberships          (→ domains, users)
4. provisioning_profiles       (→ domains)
5. property_definitions
6. devices                     (→ domains)
7. device_protocols            (→ devices)
8. device_parameters           (→ devices)
9. device_properties           (→ devices)
10. device_desired_config      (→ devices)
11. device_profile_assignments (→ devices, provisioning_profiles)
12. device_events              (→ devices)
```

## Tenancy

```
domains
├── domain_memberships  (users ↔ domains, with role)
├── devices
│   ├── device_protocols
│   ├── device_parameters
│   ├── device_properties
│   ├── device_desired_config
│   ├── device_profile_assignments
│   └── device_events
└── provisioning_profiles  (domain_id NULL = shared/system)
```

## User roles

| Role            | Scope    | Can do |
|-----------------|----------|--------|
| `super_admin`   | Global   | Create/delete domains, manage all users, manage shared profiles |
| `domain_admin`  | Domain   | Invite/remove domain members, manage domain profiles, delete devices |
| `domain_editor` | Domain   | Push commands, update device config, assign profiles |
| `domain_viewer` | Domain   | Read-only access to devices, events, parameters |

## Observed Reality
- `devices`, `device_events`, `device_parameters`

## Desired Intent
- `provisioning_profiles`, `device_properties`, `device_desired_config`

## Execution
- `tasks`, `task_results`, `provisioning_runs` *(planned)*