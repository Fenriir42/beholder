# Beholder on Kubernetes (ArgoCD sandbox)

`beholder.yml` runs the server in a namespace where only Deployments, Services, PVCs and CronJobs may be created (Pod Security `baseline`). It holds:

- **`beholder-data`**, a 2 Gi volume for `beholder.db`, `images/` and `backups/`;
- **`beholder`**, one server pod with `Recreate` updates, because two servers must never share the SQLite file;
- **`beholder`**, a ClusterIP Service on port 8080;
- **`beholder-backup`**, a CronJob that runs `beholder-server backup 14` at 04:30 every day. It keeps the last 14 copies in `/data/backups`.

It uses at most 384 Mi of memory and 0.7 CPU of the namespace quota, and only 384 Mi while the backup job runs.

## Install

This is what runs in the `julien` sandbox of the cluster repo (split there into one file per resource, the repo's convention).

1. Add the manifests to `kubernetes/sandbox/julien/` in the cluster repo and list them in its `kustomization.yml`. ArgoCD syncs them once the PR is merged.
2. The cluster owner adds what the sandbox can't create:
   - a Cloudflare DNS record in `ansible/roles/cloudflare_dns/files/dns_records.toml`, proxied like the other public apps with their own accounts:

     ```toml
     ["beholder.hosquet.com"]
     type = "A"
     content = "45.88.180.140"
     proxied = true
     ```

   - a Traefik `IngressRoute` in `kubernetes/overlays/vps/sandbox-julien/`, not gated by Authelia (the desktop client can't follow a browser login redirect):

     ```yaml
     apiVersion: traefik.io/v1alpha1
     kind: IngressRoute
     metadata:
       name: beholder
       namespace: julien
     spec:
       entryPoints:
         - web
         - websecure
       tls:
         certResolver: letsencrypt
       routes:
         - match: Host(`beholder.hosquet.com`)
           kind: Rule
           services:
             - name: beholder
               port: 8080
     ```

   WebSockets go through Cloudflare's proxy; the client pings every 15 s, under its 100 s idle limit.
3. In the app, the server address is `https://beholder.hosquet.com`. The documentation is at `https://beholder.hosquet.com/docs/`.

## Update

Change the image tag (`0.1.4`) in both places in `beholder.yml` and merge. The release workflow tags images without the `v`.

## Admin commands

```sh
kubectl exec -n julien deploy/beholder -- /beholder-server users
kubectl exec -n julien deploy/beholder -- /beholder-server reset-password <username>
kubectl exec -n julien deploy/beholder -- /beholder-server backup
```

## Copy a backup off the cluster

The volume lives on the cluster's only node, so keep copies elsewhere too:

```sh
kubectl exec -n julien deploy/beholder -- ls /data/backups
kubectl cp julien/$(kubectl get pod -n julien -l app=beholder -o name | cut -d/ -f2):/data/backups/<file> ./<file>
```
