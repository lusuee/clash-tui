#!/bin/bash
# Forwarder to install.sh --autostart-off
exec "$(dirname "$0")/install.sh" --autostart-off "$@"
