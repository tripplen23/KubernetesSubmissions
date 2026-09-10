# Exercise 3.9 — DBaaS vs DIY (GKE features)

> **This is a writing exercise, not a deployment one.** The course asks for
> a pros/cons comparison of the two ways to run a database in a cloud
> Kubernetes setup, written in the project README.

## The two solutions

- **DBaaS — Database as a Service** (e.g. Google Cloud SQL):
  a fully managed database instance rented from Google. The provider
  provisions the server, runs the software, applies patches and takes
  backups. The application connects over the network, through auth
  (workload identity / service account), never touching storage directly.
- **DIY — self-hosted in our own cluster** (what this project does):
  our own `postgres:16` image running as a StatefulSet inside the GKE
  cluster, with a PersistentVolumeClaim; GKE provisions a PersistentVolume
  (disk) under the hood and the Postgres data lives on it.

Both are widely used — the course says so at the "intersection". The rest
of this README compares them on the dimensions the exercise names:
**initialization work, initialization cost, maintenance, and backups.**

---

## Comparison

### Initialization — work

| | DBaaS (Cloud SQL) | DIY (Postgres + PVC in GKE) |
|---|---|---|
| **What you do** | Create instance in console/CLI, pick version/size/location, allowlist the cluster's access, get a connection string | Write manifests (StatefulSet, PVC, Secret, ConfigMap), push a Postgres image to the registry, `kubectl apply`, sort out routing/secrets |
| **k8s resources** | A Secret/ConfigMap for the connection string — nothing else. No storage manifests | A whole stateful stack: PersistentVolumeClaim, StatefulSet with volume mount, init scripts, secret for the password |
| **Who does the heavy lifting** | Google (server provisioning, instance sizing, networking) | You (every YAML decision, image management, cluster-side config) |
| **Verdict** | Less than an hour, mostly clicks | Doable in an evening, but many moving parts — and every one falls on you |

### Initialization — cost

| | DBaaS (Cloud SQL) | DIY (Postgres + PVC in GKE) |
|---|---|---|
| **What you pay for** | The instance itself: fixed hourly/monthly fee per instance tier + storage, **regardless of load** | Just what the cluster already costs: an extra small stateful workload on existing nodes + the PVC disk (paid per GB). No new product line |
| **Cost profile** | Predictable but **additive** — a second bill on top of the cluster; idle instances still cost | Nearly invisible when the cluster is already running for the apps; the disk is the only extra line item |
| **Cheap/free tier** | Cloud SQL has no meaningful free tier — you pay from day one | The course's free GCP credits comfortably cover extra PVC disks |
| **Verdict** | Simple and predictable, but a permanent new bill | Marginal cost ≈ 0 with an existing cluster — which is exactly why we ran Postgres in-cluster |

### Maintenance

| | DBaaS (Cloud SQL) | DIY (Postgres + PVC in GKE) |
|---|---|---|
| **Patches & upgrades** | Google handles minor versions, security patches, and major upgrades (with scheduled downtime windows/announcements) | **You.** Pull a new `postgres` image, restart the StatefulSet, run `ALTER`/dumps for major jumps — all manual, all your risk |
| **High availability** | Built-in replicas/failover as a config option | Not automatic: a StatefulSet has a single primary; HA needs hand-rolled replication (or a separate coordinator) |
| **Monitoring** | Provider dashboards, built-in metrics, retention | You wire your own (Prometheus/Loki/Alloy as in part 2) and watch it yourself |
| **Scaling** | Resize instance / add replicas via API | Stateful workloads scale badly — you're mostly stuck with what you sized |
| **Tuning** | Provider defaults, and experts to blame/ask | Full control (good and bad) — every knob is yours, and so is every mistake |
| **Verdict** | You Rent ops; the vendor owns the pager | You own the pager; full control and no vendor ops |

### Backups — method & ease of use

| | DBaaS (Cloud SQL) | DIY (Postgres + PVC in GKE) |
|---|---|---|
| **Default behavior** | **Automatic**: Google takes point-in-time backups, keeps them, restores with clicks / one command | **Nothing.** By default your data exists exactly once, on the PVC — delete the disk and it is gone forever |
| **How you back up** | Nothing to build; choose retention in the console | You must build it: `pg_dump` (or `pg_basebackup`) run by a CronJob, pushed out of the cluster to object storage |
| **Restore** | One command / console — Google matches the point in time | You script `psql < backup.sql` yourself; restore drill is your project |
| **Ease of use** | Excellent — it's the product's selling point | Achievable (exercise 3.10 does exactly this with a CronJob → Google Object Storage) but it is 100% your code to build and 0% batteries included |
| **Verdict** | The main reason to pay for DBaaS — backup is the hardest thing DIY gets wrong | The single biggest DIY gap; nothing protects you until you write it yourself |

---

## Summary

- **DBaaS wins** when: you want ops-not-included-and-proud, you value
  turnkey backups and replication more than the extra bill, or your
  team has no one who wants to be a Postgres DBA on the side.
- **DIY wins** when: the cluster already exists (marginal cost ≈ 0), you
  need full control over versions/extensions/tuning, or you want to
  avoid another vendor dependency (Google Cloud SQL is a lock-in by
  design).
- **The honest middle**: hybrid is common in production — app containers
  in the cluster, database rented as DBaaS — precisely because the
  backup/HA story is the expensive, risky part.

**In this project we went DIY** (Postgres StatefulSet + PVC since
part 2), because the course cluster is already paid for and an extra
instance bill would be silly. The trade-off is exactly the one in the
table: no automatic backups yet — that gap is what **exercise 3.10**
closes (a CronJob dumping the todo database to Google Object Storage).