# Beholder on Kubernetes (ArgoCD sandbox)

`beholder.yml` runs the server in a namespace where only Deployments, Services, PVCs and CronJobs may be created (Pod Security `baseline`). It holds:

- **`beholder-data`**, a 2 Gi volume for `beholder.db`, `images/` and `backups/`;
- **`beholder`**, one server pod with `Recreate` updates, because two servers must never share the SQLite file;
- **`beholder`**, a ClusterIP Service on port 80;
- **`beholder-backup`**, a CronJob that runs `beholder-server backup 14` at 04:30 every day. It keeps the last 14 copies in `/data/backups`.

It uses at most 384 Mi of memory and 0.7 CPU of the namespace quota, and only 384 Mi while the backup job runs.

## Install

1. Copy `beholder.yml` into `kubernetes/sandbox/julien/` in the cluster repo and open a PR. ArgoCD syncs it once it's merged.
2. Ask the cluster admin for a public hostname. They need to add a Traefik route in front of the Service, which the sandbox can't create itself. For example, for Traefik with a Let's Encrypt resolver:

   ```yaml
   apiVersion: networking.k8s.io/v1
   kind: Ingress
   metadata:
     name: beholder
     namespace: julien
     annotations:
       traefik.ingress.kubernetes.io/router.entrypoints: websecure
       traefik.ingress.kubernetes.io/router.tls.certresolver: <resolver>
   spec:
     rules:
       - host: beholder.45-88-180-140.sslip.io
         http:
           paths:
             - path: /
               pathType: Prefix
               backend:
                 service:
                   name: beholder
                   port:
                     name: http
   ```

   `sslip.io` names resolve to the IP in them, so no DNS record is needed. Traefik passes WebSocket connections through as they are.
3. In the app, the server address is `https://<hostname>`. The documentation is at `https://<hostname>/docs/`.

## Update

Change the image tag (`0.1.0`) in both places in `beholder.yml` and merge. The release workflow tags images without the `v`.

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
