#!/usr/bin/env bash
# End-to-end test of the Ansible role against a disposable systemd container.
# Runs in .github/workflows/test-ansible.yml; works locally with Docker + ansible-core.
#
#   deploy/ansible/tests/run.sh <nebuladns-binary> <nebulactl-binary>
#
# Binaries must be Linux builds for the Docker host's architecture.
set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 <nebuladns-binary> <nebulactl-binary>" >&2
  exit 2
fi

here="$(cd "$(dirname "$0")" && pwd)"
ansible_dir="$(dirname "$here")"
target=nebuladns-ansible-target
image=nebuladns-ansible-target:local
work="$(mktemp -d)"

cleanup() {
  docker rm -f "$target" >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

# Fact caching in ansible.cfg would carry facts across containers between local runs.
export ANSIBLE_CACHE_PLUGIN=memory
export ANSIBLE_HOST_KEY_CHECKING=False

mkdir -p "$work/artifacts" "$work/zones"
cp "$1" "$work/artifacts/nebuladns"
cp "$2" "$work/artifacts/nebulactl"
chmod 0755 "$work/artifacts/"*

step() { printf '\n=== %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; docker exec "$target" journalctl -u nebuladns --no-pager -n 50 >&2 || true; exit 1; }

write_zone() {
  sed "s/@WWW_IP@/$1/" "$here/zones/ci.test.toml.in" > "$work/zones/ci.test.toml"
}

deploy() {
  local version="$1"
  shift
  (cd "$ansible_dir" && ansible-playbook -i tests/inventory.ini playbook.yml \
    -e @environments/private.yml \
    -e nebuladns_version="$version" \
    -e nebuladns_artifact_dir="$work/artifacts" \
    -e "{\"nebuladns_zones\": [{\"name\": \"ci.test\", \"src\": \"$work/zones/ci.test.toml\"}]}" \
    "$@")
}

in_target() { docker exec "$target" "$@"; }
main_pid() { in_target systemctl show -p MainPID --value nebuladns; }
# Only A-record lines count: when nothing listens, dig prints its error on stdout.
answer() {
  in_target dig +short +time=2 +tries=1 "@$1" www.ci.test A 2>/dev/null \
    | grep -E '^[0-9]+(\.[0-9]+){3}$' || true
}

expect_answer() {
  local want="$1" got=""
  for _ in $(seq 1 20); do
    got="$(answer "$target_ip")"
    [ "$got" = "$want" ] && return 0
    sleep 0.5
  done
  fail "www.ci.test A: want '$want', got '$got'"
}

step "Start systemd target container"
docker build -q -t "$image" "$here" >/dev/null
docker rm -f "$target" >/dev/null 2>&1 || true
docker run -d --name "$target" --hostname "$target" \
  --privileged --cgroupns=host -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  --tmpfs /run --tmpfs /run/lock "$image" >/dev/null
for _ in $(seq 1 30); do
  state="$(in_target systemctl is-system-running 2>/dev/null || true)"
  case "$state" in running|degraded) break ;; esac
  sleep 1
done
target_ip="$(in_target ip -4 -o addr show dev eth0 | awk '{split($4, a, "/"); print a[1]}')"
[ -n "$target_ip" ] || fail "target has no eth0 IPv4 address"
echo "target eth0 = $target_ip"

step "1. Fresh deploy binds DNS to the eth0 address only"
write_zone 192.0.2.1
deploy ci-1
expect_answer 192.0.2.1
[ -z "$(answer 127.0.0.1)" ] || fail "DNS answered on 127.0.0.1; it should be bound to eth0 only"
in_target grep -q 'After=wg-quick@eth0.service' \
  /etc/systemd/system/nebuladns.service.d/10-dns-interface.conf \
  || fail "interface ordering drop-in missing"

step "2. Re-running with no changes is idempotent"
deploy ci-1 | tee "$work/rerun.log"
grep -Eq 'changed=0 +unreachable=0 +failed=0' "$work/rerun.log" || fail "second run reported changes"

step "3. Zone-only change reloads (same PID) and serves the new answer"
pid_before="$(main_pid)"
write_zone 192.0.2.2
deploy ci-1
expect_answer 192.0.2.2
[ "$(main_pid)" = "$pid_before" ] || fail "zone-only deploy restarted nebuladns (PID $pid_before -> $(main_pid))"

step "4. A bad zone fails the deploy and the old zone keeps serving"
printf 'this is not a zone\n' > "$work/zones/ci.test.toml"
if deploy ci-1 >"$work/bad.log" 2>&1; then
  cat "$work/bad.log"
  fail "deploy with a bad zone succeeded"
fi
grep -q 'parse zone file' "$work/bad.log" || { cat "$work/bad.log"; fail "deploy failed for the wrong reason"; }
in_target grep -q 192.0.2.2 /etc/nebuladns/zones/ci.test.toml || fail "bad zone reached the host"
expect_answer 192.0.2.2

step "5. New binary + zone change restarts once and serves the new zone"
pid_before="$(main_pid)"
write_zone 192.0.2.3
deploy ci-2
expect_answer 192.0.2.3
[ "$(main_pid)" != "$pid_before" ] || fail "binary change did not restart nebuladns"
[ "$(in_target readlink /usr/local/bin/nebuladns)" = /usr/local/bin/nebuladns-ci-2 ] \
  || fail "symlink not flipped to ci-2"

step "6. rollback.yml flips back to the previous binary and keeps serving"
(cd "$ansible_dir" && ansible-playbook -i tests/inventory.ini rollback.yml -e @environments/private.yml)
[ "$(in_target readlink /usr/local/bin/nebuladns)" = /usr/local/bin/nebuladns-ci-1 ] \
  || fail "rollback did not flip the symlink to ci-1"
expect_answer 192.0.2.3

printf '\nAll Ansible role checks passed.\n'
