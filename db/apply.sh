#!/usr/bin/env bash

# This script applies all SQL files in the correct dependency order.
# It uses `psql` to execute the scripts against a PostgreSQL database.

set -e

# Default to a local database if DATABASE_URL is not provided
DB_URL="${DATABASE_URL:-postgres://acsuser:${DBPASS}@localhost:5432/acs}"

# The directory containing the SQL files (relative to this script)
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "Applying SQL schema to database at: $DB_URL"
echo "------------------------------------------------"

# Array of SQL files in correct dependency order
FILES=(
    "domains.sql"
    "users.sql"
    "domain_memberships.sql"
    "provisioning_profiles.sql"
    "property_definitions.sql"
    "devices.sql"
    "device_protocols.sql"
    "device_parameters.sql"
    "device_properties.sql"
    "device_desired_config.sql"
    "device_profile_assignments.sql"
    "device_events.sql"
)

for FILE in "${FILES[@]}"; do
    if [ -f "$DIR/$FILE" ]; then
        echo "Applying: $FILE"
        psql "$DB_URL" -v ON_ERROR_STOP=1 -f "$DIR/$FILE" -q
    else
        echo "Error: File not found: $DIR/$FILE"
        exit 1
    fi
done

echo "------------------------------------------------"
echo "Database schema applied successfully!"
