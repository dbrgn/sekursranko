#!/usr/bin/env bash
set -euo pipefail

echo "[entrypoint.sh] Start"

# Patch a config variable
#
# Args: <var> <type>
#   var: The name of the config variable (e.g. `MAX_BACKUP_BYTES`)
#   type: One of `string` or `number`
function patch_var() {
    var=$1
    type=$2
    if [ -n "${!var:-}" ]; then
        echo "[entrypoint.sh] Patching config ${var,,}"
        if [ "$type" = "string" ]; then
            sed "s%^${var,,}[ =].*$%${var,,} = \"${!var}\"%" /etc/sekursranko/config.toml | sponge /etc/sekursranko/config.toml
        elif [ "$type" = "number" ]; then
            sed "s%^${var,,}[ =].*$%${var,,} = ${!var}%" /etc/sekursranko/config.toml | sponge /etc/sekursranko/config.toml
        else
            echo "[entrypoint.sh] Error: Invalid type \"$type\""
            exit 1
        fi
    fi
}

patch_var MAX_BACKUP_BYTES number
patch_var RETENTION_DAYS number
patch_var BACKUP_DIR string
patch_var LISTEN_ON string

echo "[entrypoint.sh] Done"
exec sekursranko --config /etc/sekursranko/config.toml
