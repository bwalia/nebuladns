# NebulaDNS Ansible deployment

This directory ships an Ansible role, playbook, environment overrides, and a rollback
playbook. The `.github/workflows/deploy.yml` workflow invokes them to deploy a built
NebulaDNS musl binary to a Debian/Ubuntu host over SSH.

## Layout

```
deploy/ansible/
├── ansible.cfg
├── playbook.yml              # main deploy playbook
├── rollback.yml              # emergency rollback (symlink flip)
├── requirements.yml          # Ansible Galaxy collections
├── inventory.example.ini     # template — the real inventory lives in a GitHub secret
├── environments/
│   ├── staging.yml
│   └── production.yml
└── roles/nebuladns/
    ├── defaults/main.yml
    ├── handlers/main.yml
    ├── meta/main.yml
    ├── tasks/main.yml
    └── templates/nebuladns.toml.j2
```

## What the role does

1. Creates the `nebuladns` system user + group (non-login).
2. Creates `/etc/nebuladns`, `/var/lib/nebuladns`, `/var/log/nebuladns` with tight perms.
3. Stages the new binary at `/usr/local/bin/nebuladns-<version>` and smoke-tests it
   (`--print-default-config`). If the binary won't even start, nothing else changes.
4. Atomically flips `/usr/local/bin/nebuladns` + `/usr/local/bin/nebulactl` symlinks to
   the new version. The previous versioned binary stays on disk for fast rollback.
5. Grants `CAP_NET_BIND_SERVICE` so the server can bind `:53` without running as root.
6. Renders `/etc/nebuladns/nebuladns.toml` from the Jinja template.
7. Copies zone TOML files declared in `nebuladns_zones` (search path: role's
   `files/zones/` and `deploy/ansible/files/zones/`).
8. Installs the systemd unit from `deploy/systemd/nebuladns.service` (the single source
   of truth — no duplicated unit file).
9. `daemon-reload` + restart as needed (only when files actually changed).
10. Probes `http://<api_bind>/livez` until 200 OK; fails the deploy if it never goes
    green. `/livez` on the admin API is the contract from M0.
11. Prunes old versioned binaries, keeping the current + one previous.

## GitHub secrets required

Configure these under **Settings → Secrets and variables → Actions**. You can scope them
per environment (`staging`, `production`).

| Secret               | What                                                                                          |
|----------------------|-----------------------------------------------------------------------------------------------|
| `DEPLOY_SSH_KEY`     | Private SSH key (PEM) for a `deploy` user on each target host. Must be authorized in `~deploy/.ssh/authorized_keys` and have passwordless sudo (`deploy ALL=(ALL) NOPASSWD:ALL`). |
| `DEPLOY_KNOWN_HOSTS` | Output of `ssh-keyscan ns1.example.com ns2.example.com`. Strict host checking is on.          |
| `DEPLOY_INVENTORY`   | Full Ansible inventory file (ini format). See `inventory.example.ini`.                        |

Repository **Variables** (same screen):

| Variable                   | What                                                  |
|----------------------------|-------------------------------------------------------|
| `DEPLOY_ARCH`              | `amd64` (default) or `arm64`. Selects the musl target.|
| `NEBULADNS_API_BIND`       | Optional. Used by the post-deploy external probe.     |
| `NEBULADNS_DASHBOARD_URL`  | Optional. Shown in the workflow environment URL.      |

## Preparing a Debian target host

```bash
# On the target (run once, as root):
adduser --disabled-password --gecos "NebulaDNS deploy" deploy
mkdir -p /home/deploy/.ssh && chmod 700 /home/deploy/.ssh
echo "<PUBLIC KEY>" >> /home/deploy/.ssh/authorized_keys
chmod 600 /home/deploy/.ssh/authorized_keys
chown -R deploy:deploy /home/deploy/.ssh

# Passwordless sudo for the deploy user.
echo 'deploy ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/deploy-nebuladns
chmod 0440 /etc/sudoers.d/deploy-nebuladns

# systemd 247+ and a Python 3 interpreter are already present on Debian 12/Ubuntu 22.04+.
# If selinux-utils is present, either disable or configure it for the nebuladns unit.
```

## Running the workflow

- **Tag push** (`git tag v1.2.3 && git push origin v1.2.3`) → production deploy.
- **Manual** → `Actions → deploy → Run workflow` and pick an environment.

The workflow order: `test` → `build musl` → `ansible-deploy`. Production gets a
`--check --diff` dry-run before the apply so you can see what will change in the job
logs.

## Local invocation (dev loop)

You can run the same playbook against a local VM or test host without GitHub:

```bash
cd deploy/ansible
ansible-galaxy collection install -r requirements.yml

# Build once (cross-rs) into the location the role expects.
( cd ../..; cross build --release --target x86_64-unknown-linux-musl \
    --bin nebuladns --bin nebulactl )
mkdir -p roles/nebuladns/files
cp ../../target/x86_64-unknown-linux-musl/release/nebuladns  roles/nebuladns/files/
cp ../../target/x86_64-unknown-linux-musl/release/nebulactl  roles/nebuladns/files/
( cd roles/nebuladns/files && sha256sum nebuladns nebulactl > SHA256SUMS )

# Inventory — copy + edit inventory.example.ini into inventory.ini.
ansible-playbook -i inventory.ini playbook.yml \
  -e nebuladns_version=dev-$(git rev-parse --short HEAD) \
  -e @environments/staging.yml
```

## Rollback

```bash
# Flip back to the previous binary on every host in the inventory.
ansible-playbook -i inventory.ini rollback.yml

# Or pick an exact version (must still be present at /usr/local/bin/nebuladns-<v>).
ansible-playbook -i inventory.ini rollback.yml -e rollback_version=v1.2.2
```

The rollback playbook never downloads anything — it flips the symlink to an already-staged
binary and restarts systemd. Mean time to roll back is a few seconds per host.

## Why this shape

- **Binary versioning + symlink swap** is the simplest atomic primitive available on a
  bare systemd host. We deliberately did not build Debian packages — `.deb` handling is
  in the longer-term plan (deliverables §13.1), but for now this gives us identical
  rollback semantics without the packaging loop.
- **Smoke-test on the target before flipping** catches architecture / libc mismatches
  *before* we replace a running binary.
- **Health probe gates the deploy** so a broken config surfaces as a workflow failure,
  not as quiet downtime. This is the same contract as the Kubernetes readiness probe.
- **`serial: 25%`** in production prevents a bad deploy from taking the whole fleet
  offline simultaneously.
