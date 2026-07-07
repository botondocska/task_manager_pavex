#!/bin/sh
set -e
envsubst < configuration/base.yml > /tmp/base.yml
cp /tmp/base.yml configuration/base.yml
exec ./bin