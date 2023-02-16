# k8s-consul-mutator-rs

A Kubernetes mutating webhook that patches resources with checksums of consul keys.

# Usage

This Kubernetes mutating webhook will write the checksum of a Consul key into into a resource annotation.

```yaml
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: app
  annotations:
    k8s-consul-mutator.io/key: app/config
```

The above annotation for the app deployment would result in the following mutation.

```yaml
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: app
  annotations:
    k8s-consul-mutator.io/key-app-config: app/config
    k8s-consul-mutator.io/checksum-app-config: md5-b1e46790875837e159c0787bac2e29be
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
- [ ] Start consul watchers for existing deployments
- [ ] Support sha checksums
- [ ] Support fnv checksums
- [ ] Support consul key index values as checksums
- [ ] Populate background workers at startup
- [ ] Configuration for consul tokens
- [ ] Configuration for default checksum type
- [ ] Configuration validation on start
- [ ] Helm chart
