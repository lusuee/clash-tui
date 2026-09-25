#!/bin/bash
# Forwarder to install.sh --uninstall
exec "$(dirname "$0")/install.sh" --uninstall "$@"
