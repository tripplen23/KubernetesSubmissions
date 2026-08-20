# Exercise 2.9 — The project, step 12 (CronJob)

## Goal

> Create a **CronJob** that generates a new todo every hour to remind
> you to do **'Read \<URL\>'**, where `<URL>` is a Wikipedia article that
> was decided by the job randomly. It does not have to be a hyperlink,
> the user can copy-paste the URL from the todo.
>
> <https://en.wikipedia.org/wiki/Special:Random> responds with a
> **redirect** to a random Wikipedia page, so you can ask it to provide
> a random article for you to read. **TIP: Check location header**.

This is a **self-contained** lab: it builds on the full project from
2.8 (todo-app, todo-backend, Postgres) **and** adds the new CronJob.
Everything is included here so you can run it from a clean cluster.

## Job vs CronJob (quick recap from the course page)

- **Job** = a workload that runs **once from start to finish** and exits
  (it is not a server). Status is saved so you can inspect it after.
- **CronJob** = a Job on a **schedule**. Every hour it
  launches a fresh Job that does the task and exits.

Our task runs a tiny script that:

1. Calls `https://en.wikipedia.org/wiki/Special:Random`
2. Reads the **`Location` header** of the 302 redirect → the random
   article's URL
3. `POST`s `{"title": "Read <url>"}` to the todo-backend
4. Exits

## What the folder contains

```
part2/2.9/
├── todo-app/          (complete source from 2.6 — unchanged)
├── todo-backend/      (complete source from 2.8 — unchanged)
├── todo-cron/         (NEW — the CronJob script)
│   └── generate-todo.sh
└── manifests/         (full project + the CronJob)
```

## Step 1 — build + push the Dockerfiles

Three images. `todo-app` and `todo-backend` are reused from earlier
(their code is unchanged); `todo-cron` is new.

Build and push:

```bash
cd todo-app
docker build -t tripplen63/todo-app:2.6 .
docker push tripplen63/todo-app:2.6

cd ../todo-backend
docker build -t tripplen63/todo-backend:2.8 .
docker push tripplen63/todo-backend:2.8

cd ../todo-cron
docker build -t tripplen63/todo-cron:2.9 .
docker push tripplen63/todo-cron:2.9
```

> The todo-cron script uses `#!/usr/bin/env sh` (POSIX), because
> `alpine:3.19` has **no bash**. `curl -w '%{redirect_url}'` reads the
> **Location header** of the 302 from `Special:Random` (the random
> article's URL) without following the redirect.

## Step 2 — Apply everything and verify manifest

```bash
docker exec k3d-mycluster-agent-0 mkdir -p /tmp/kube

kubectl apply -f manifests/pv/persistentvolume.yaml
kubectl apply -f manifests/

kubectl get statefulset,pods -n project
# pod/postgres-ss-0         1/1  Running
# pod/todo-backend-xxx      1/1  Running
# pod/todo-app-xxx          1/1  Running

kubectl get cronjobs -n project
# NAME        SCHEDULE    SUSPEND   ACTIVE   LAST SCHEDULE
# todo-cron   0 * * * *   False     0        <none>
```

Run the one-off Job to verify the logic immediately:

```bash
kubectl apply -f manifests/job-test.yaml
sleep 20
kubectl get jobs -n project
# NAME             COMPLETIONS   DURATION   AGE
# todo-cron-test   1/1           7s         20s

kubectl logs job/todo-cron-test -n project
# [todo-cron] starting at ...
# [todo-cron] random article URL: https://en.wikipedia.org/wiki/Some_Article
# [todo-cron] POST http://todo-backend-svc:2345/todos -> 201
# [todo-cron] created todo: Read https://en.wikipedia.org/wiki/Some_Article

# confirm it landed in the UI / Postgres
curl -s http://localhost:8081/ | grep '<span>Read'
kubectl exec postgres-ss-0 -n project -- psql -U postgres -c "SELECT * FROM todos ORDER BY id;"
```

## Step 3 — Prove the CronJob fires (optional quick check)

Once we know the one-off Job works, the CronJob itself will fire on the
next hour boundary (`0 * * * *`). You can watch it appear:

```bash
kubectl get jobs -n project -w
# a new todo-cron-xxxxxxx job appears at the top of the hour
```

To confirm the schedule is active without waiting:

```bash
kubectl get cronjob todo-cron -n project
# NAME        SCHEDULE    SUSPEND   ACTIVE   LAST SCHEDULE
# todo-cron   0 * * * *   False     0        <none>
```

## Step 4 — Clean up

```bash
kubectl delete cronjob todo-cron -n project
kubectl delete job todo-cron-test -n project
kubectl delete -f manifests/
kubectl delete -f manifests/pv/persistentvolume.yaml
kubectl delete pvc -n project -l app=postgres 2>/dev/null
kubectl get cronjobs,jobs,pods -n project
# No resources found
```

## P/S:

1. **Job** = run-once workload (vs Deployment/StatefulSet which run
   forever). `restartPolicy` must be `OnFailure`/`Never`, not `Always`.
2. **CronJob** = Job on a schedule (`schedule: "0 * * * *"`), fires a
   fresh Job each hour.
3. **Redirect + Location header**: `curl -w '%{redirect_url}'` reads
   where a 302 points without following it — a web/API trick.
4. The CronJob container is **ephemeral**: it does its one task, exits,
   and a new one starts next interval.
5. **Self-contained lab**: the CronJob depends on the whole 2.8 project,
   so all its source and manifests live here too.
