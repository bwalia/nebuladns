# Disposable systemd host for tests/run.sh: the role needs a real init, sudo + python3 for
# Ansible modules, and dig for the DNS assertions.
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
# hadolint ignore=DL3008
RUN apt-get update \
 && apt-get install -y --no-install-recommends systemd systemd-sysv sudo python3 dnsutils iproute2 \
 && rm -rf /var/lib/apt/lists/* \
 && systemctl mask systemd-resolved.service getty.target console-getty.service
STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
