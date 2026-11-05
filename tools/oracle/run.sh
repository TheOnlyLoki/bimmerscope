#!/bin/sh
# Runs EdiabasLib's EdiabasTest (built by Oracle.csproj) in the dotnet SDK
# container. ECU dir and sim dir are mounted read-only.
# usage: run.sh <ecu-dir> <sim-dir> <EdiabasTest args...>
ecu=$1; sim=$2; shift 2
exec docker run --rm -v "$ecu":/ecu:ro,z -v "$sim":/sim:ro,z -v "$(dirname "$0")/out":/app:ro,z \
  -w /ecu mcr.microsoft.com/dotnet/sdk:8.0 dotnet /app/ediabas-oracle.dll \
  --cfg="EcuPath=/ecu;Simulation=1;SimulationPath=/sim" "$@"
