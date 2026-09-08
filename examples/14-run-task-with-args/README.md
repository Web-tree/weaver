# 14 - Run Task with Arguments

Demonstrates `wvr run <app> <task> [args...]` using the app working directory and
forwarding positional arguments to the task command.

## What this covers

- `wvr run` task lookup from module manifest
- Running task commands in `apps[].path`
- Forwarding CLI args to the underlying command

## Before state

- Module defines a `deploy` task (`sh scripts/deploy.sh`)
- App path is `services/payments`
- Deployment script exists in the app workspace and writes `deploy.log`

## How to run

```sh
cd before
wvr run payments deploy production us-east-1
```

## Expected result

After `wvr run`, the workspace should match `after/`:
- `services/payments/deploy.log` is created by the task command
- Log content includes forwarded args (`production`, `us-east-1`)
