#!/bin/bash
set -e

# Ensure data directory is writable
# This is a brute-force approach to fix "readonly database" issues with Docker volumes
echo "Fixing permissions for /app/data..."
chmod 777 /app/data || true

# If the database file exists, ensure it is also writable
if [ -f /app/data/w9_search.db ]; then
    chmod 666 /app/data/w9_search.db || true
fi

# Also ensure WAL/SHM files are writable if they exist
if [ -f /app/data/w9_search.db-wal ]; then
    chmod 666 /app/data/w9_search.db-wal || true
fi
if [ -f /app/data/w9_search.db-shm ]; then
    chmod 666 /app/data/w9_search.db-shm || true
fi

echo "Starting w9-search..."
exec "$@"
