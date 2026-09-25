#!/bin/bash
# Forwarder to install.sh --autostart-on
exec "$(dirname "$0")/install.sh" --autostart-on "$@"
