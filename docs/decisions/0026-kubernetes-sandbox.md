# 0026: Running in a Kubernetes sandbox

## Context

SPEC §2 plans the server as a Docker Compose stack on its own small VPS, with Caddy for TLS on ports 80 and 443. The group's VPS is shared. A minikube cluster with Traefik already holds 80 and 443, and other apps are deployed by ArgoCD from a cluster repository. Our account gets a namespace whose sandbox directory accepts only Deployments, StatefulSets, Jobs, CronJobs, Services, ConfigMaps, PVCs, ServiceAccounts and SealedSecrets, under Pod Security `baseline`, with a quota of 8 pods, 512 Mi/1 Gi of memory, 0.5/1 CPU and 5 Gi of storage. Routes and hostnames are added by the cluster admin.

## Decision

- **`deploy/k8s/beholder.yml`:**
  - A 2 Gi PVC.
  - One server Deployment. Its `Recreate` strategy means two servers never open the SQLite file. It runs as uid 10001 with a read-only root, no capabilities, `RuntimeDefault` seccomp, and an `emptyDir` at `/tmp` for SQLite's temporary files.
  - A ClusterIP Service.
  - A nightly backup CronJob.
  - The same release image as Docker Compose, pinned to a version tag.
- **`beholder-server backup [keep]`:** a new admin command. It writes `VACUUM INTO backups/beholder-<time>.db` while the server keeps running, and prunes to the newest `keep` copies. The image is built on `scratch`, so the CronJob runs the server binary itself rather than a shell or `sqlite3`.
- **TLS and hostname:** the cluster's Traefik terminates TLS. The admin adds the route, and an `sslip.io` name works when there's no DNS record. The README includes an example Ingress for them.
- The Compose stack stays the default for a VPS of its own.

## Alternatives

- **Compose next to the cluster:** ports 80 and 443 are taken, so it would get no certificate.
- **Binding to the VPN address only:** works without TLS, but every player would have to use that VPN.
- **A sidecar backup with `sqlite3`:** another image to keep up to date, when the server can write the copy itself.

## Consequences

- Updates go through a PR to the cluster repository, which changes the image tag.
- Backups live on the same single node as the data. They protect against corruption and bad updates, not disk loss, so copies should be pulled off regularly with `kubectl cp`.
- Admin commands need `kubectl exec` access to the namespace.
