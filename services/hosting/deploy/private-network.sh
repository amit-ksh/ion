#!/usr/bin/env bash
# Operator-run setup on the dedicated worker VM. Review before running.
set -euo pipefail
[[ $(id -u) == 0 ]] || { echo "Run as root on the dedicated worker VM." >&2; exit 1; }
docker info >/dev/null
if ! docker network inspect ion-apps >/dev/null 2>&1; then
  docker network create --internal --subnet 172.31.0.0/24 \
    --opt com.docker.network.bridge.name=ion-apps0 ion-apps >/dev/null
fi
[[ $(docker network inspect ion-apps --format '{{.Internal}}') == true ]] || exit 1
[[ $(docker network inspect ion-apps --format '{{index .Options "com.docker.network.bridge.name"}}') == ion-apps0 ]] || exit 1
[[ $(docker network inspect ion-apps --format '{{(index .IPAM.Config 0).Subnet}}') == 172.31.0.0/24 ]] || exit 1
# Idempotent, dedicated chains. No general host firewall chains are flushed.
iptables -N ION_HOST_INPUT 2>/dev/null || true
iptables -F ION_HOST_INPUT
iptables -A ION_HOST_INPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
iptables -A ION_HOST_INPUT -p tcp --dport 3128 -j ACCEPT
iptables -A ION_HOST_INPUT -j DROP
iptables -C INPUT -i ion-apps0 -j ION_HOST_INPUT 2>/dev/null ||
  iptables -I INPUT 1 -i ion-apps0 -j ION_HOST_INPUT
iptables -N ION_APP_FORWARD 2>/dev/null || true
iptables -F ION_APP_FORWARD
iptables -A ION_APP_FORWARD -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
iptables -A ION_APP_FORWARD -j DROP
iptables -C DOCKER-USER -i ion-apps0 -j ION_APP_FORWARD 2>/dev/null ||
  iptables -I DOCKER-USER 1 -i ion-apps0 -j ION_APP_FORWARD
echo "Dedicated network policy installed. Persist it with your host firewall service before enabling the worker."
