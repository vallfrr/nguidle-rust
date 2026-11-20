#!/bin/bash
# .NET SDK 8 + ilspycmd in WSL (user level, no sudo).
set -e
if ! ~/.dotnet/dotnet --list-sdks 2>/dev/null | grep -q "^10"; then
  curl -sSL https://dot.net/v1/dotnet-install.sh -o /tmp/dotnet-install.sh
  bash /tmp/dotnet-install.sh --channel 10.0 --install-dir ~/.dotnet
fi
export DOTNET_ROOT=~/.dotnet PATH=~/.dotnet:~/.dotnet/tools:$PATH DOTNET_CLI_TELEMETRY_OPTOUT=1
dotnet --version
dotnet tool list -g | grep -q ilspycmd || dotnet tool install -g ilspycmd
ilspycmd --version
