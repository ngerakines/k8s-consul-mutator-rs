# k8s-consul-mutator-rs

A Kubernetes mutating webhook that patches resources with checksums of consul keys.

# Configuration

This application uses the following environment variables:

* `RUST_LOG` - Sets logging configuration. The default value is `k8s_consul_mutator_rs=debug,tower_http=debug`.
* `PORT` - Setting `PORT` to 0 disables the insecure (HTTP) interface. The default value is 8080.
* `SECURE_PORT` - Setting `SECURE_PORT` to 0 disables the secure (HTTPS) interface. The default value is 8443.
* `CERTIFICATE` - Both the `CERTIFICATE` and `CERTIFICATE_KEY` values must be set to file path values in order for the secure (HTTPS) interface to start.
* `CERTIFICATE_KEY`
* `CONSUL_HTTP_ADDR` - The `CONSUL_HTTP_ADDR` environment variable is used to configure which consul endpoint is used for key subscriptions. The default value is `http://127.0.0.1:8500`.
* `CONSUL_HTTP_TOKEN`
* `CONSUL_HTTP_SSL_VERIFY`
* `UPDATE_DEBOUNCE` - The amount of time to wait before making updates to deployments when checksums change.
* `WATCH_DISPATCHER_FIRST_RECONCILE` - The amount of time to wait when the application starts before performing deployment reconciliation.
* `WATCH_DISPATCHER_RECONCILE` - The amount of time to wait inbetween deployment reconcillation.
* `WATCH_DISPATCHER_DEBOUNCE` - The amount of time to wait for consul watch create and delete actions to settle.
* `CHECK_KEY_TIMEOUT` - The amount of time to poll consul for key updates.
* `CHECK_KEY_IDLE` - The amount of time to allow the consul key watcher to idle before shutting down.
* `CHECK_KEY_ERROR_WAIT` - The amount of time to skip in between cycles when an error is encountered polling consul keys.
* `SET_DEPLOYMENT_ANNOTATIONS` - Adds the checksum annotations to deployments if set to true. Default true.
* `SET_DEPLOYMENT_SPEC_ANNOTATIONS` - Adds the checksum annotations to deployment specs if set to true. Default true.
* `SET_DEPLOYMENT_TIMESTAMP` - Adds the `last-updated` annotation to deployments if set to true. Default true.
* `SET_DEPLOYMENT_SPEC_TIMESTAMP` - Adds the `last-updated` annotation to deployment specs if set to true. Default false.

The default values are ideal for a verbose and insecure production environment. For production use, start with the following and tune them accordingly:

```
RUST_LOG=k8s_consul_mutator_rs=warning
PORT=0
CERTIFICATE=/path/to/your/certificate.crt
CERTIFICATE_KEY=/path/to/your/certificate.key
CONSUL_HTTP_ADDR=https://consul.consul.svc:8501
CONSUL_HTTP_TOKEN=68a1a640-96d9-4c33-8074-5b49378aa881
CONSUL_HTTP_SSL_VERIFY=true
UPDATE_DEBOUNCE=60
WATCH_DISPATCHER_FIRST_RECONCILE=120
WATCH_DISPATCHER_RECONCILE=600
WATCH_DISPATCHER_DEBOUNCE=60
CHECK_KEY_TIMEOUT=10s
CHECK_KEY_IDLE=600
CHECK_KEY_ERROR_WAIT=30
```

It is important to understand how these values will impact shutdown time. Specifically, the `CHECK_KEY_ERROR_WAIT` introduces an async sleep for that number of seconds. That means, if an error occurs with consul and a shutdown is initiated, shutdown will be blocked for at least that amount of time.

# Usage

This Kubernetes mutating webhook will write the checksum of a Consul key into into a resource annotation.

```yaml
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: app
  annotations:
    k8s-consul-mutator.io/key-config: app/config
```

The above annotation for the app deployment would result in the following mutation.

```yaml
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: app
  annotations:
    k8s-consul-mutator.io/key-config: app/config
    k8s-consul-mutator.io/checksum-config: md5-e80ca0bfa6b0c57f6360e22f1aebabc5
    k8s-consul-mutator.io/last-updated: 2023-02-17T21:51:13.479453+00:00
spec:
  template:
    metadata:
      annotations:
        k8s-consul-mutator.io/checksum-config: md5-e80ca0bfa6b0c57f6360e22f1aebabc5
        k8s-consul-mutator.io/last-updated: 2023-02-17T21:51:13.479453+00:00
```

# Disclosures

GitHub Copilot contributed to code in this repository.

Commits that were influenced by GitHub Copilot will have the `[copilot]` tag appended.

# Roadmap

- [X] Project stubbed out
- [X] HTTP endpoint for status
- [X] HTTP endpoint for mutate
- [X] Key manager for checksums
- [X] Consul configuration at start
- [X] Background workers for polling consul kv reads
- [X] Track kubernetes resources to update
- [X] Update kubernetes resources on consul kv change
- [X] Stop consul watchers for keys that are no longer used
- [X] Start consul watchers for existing deployments
- [X] Support sha checksums
- [ ] Support fnv checksums
- [ ] Populate background workers at startup
- [X] Configuration for consul tokens
- [X] Configuration for default checksum type
- [X] Configuration validation on start
- [ ] Helm chart
