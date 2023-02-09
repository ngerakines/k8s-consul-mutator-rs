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
