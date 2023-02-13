# Testing with MiniKube

1. Download and start minikube with the registry addon enabled.

https://minikube.sigs.k8s.io/docs/start/

    $ minikube addons enable registry
    $ minikube start

Then verify it is running:

    $ minikube kubectl -- get pods -A

2. Make the container registry accessible.

One in terminal start:

    $ minikube kubectl -- port-forward --namespace kube-system service/registry 5000:80

In another terminal start:

    $ docker run --rm -it --network=host alpine ash -c "apk add socat && socat TCP-LISTEN:5000,reuseaddr,fork TCP:host.docker.internal:5000"

3. Create a certificate

    $ openssl req -x509 -newkey rsa:2048 -keyout k8s-consul-mutator-rs.key -out k8s-consul-mutator-rs.pem -days 365  -nodes -subj "/CN=k8s-consul-mutator-rs.k8s-consul-mutator-rs.svc" -extensions EXT -config <(printf "[dn]\nCN=k8s-consul-mutator-rs.k8s-consul-mutator-rs.svc\n[req]\ndistinguished_name = dn\n[EXT]\nsubjectAltName=DNS:k8s-consul-mutator-rs.k8s-consul-mutator-rs.svc\nkeyUsage=digitalSignature\nextendedKeyUsage=serverAuth")

4. Build and push the container.

    $ docker build --build-arg GIT_HASH=`git rev-parse HEAD` -t localhost:5000/k8s-consul-mutator-rs:`git rev-parse HEAD` .
    $ docker push localhost:5000/k8s-consul-mutator-rs:`git rev-parse HEAD`

5. Prepare the k8s-consul-mutator-rs namespace and resources

    $ minikube kubectl -- create ns k8s-consul-mutator-rs

6. Create the k8s-consul-mutator-rs deployment and verify it is running

First, update the `CONSUL_HTTP_ADDR` environment variable value in `minikube_deployment.yaml` to point to your consul server.

    $ minikube kubectl -- apply -f minikube_deployment.yaml
    $ minikube kubectl -- get all -n k8s-consul-mutator-rs
    $ minikube kubectl -- describe -n k8s-consul-mutator-rs pod/k8s-consul-mutator-rs-5ddfbd86c-562cb
    $ minikube kubectl -- logs -n k8s-consul-mutator-rs pod/k8s-consul-mutator-rs-5ddfbd86c-562cb

7. Create a the k8s-consul-mutator-rs service

    $ minikube kubectl -- apply -f minikube_service.yaml
    $ minikube kubectl -- port-forward --namespace k8s-consul-mutator-rs service/k8s-consul-mutator-rs 8080:80
    $ curl -vv http://localhost:8080/
    $ minikube kubectl -- port-forward --namespace k8s-consul-mutator-rs service/k8s-consul-mutator-rs 8443:443
    $ curl -vv https://localhost:8443/

8. Install the admission controller

First, run the following command and replace the content of "PLACEHOLDER" in the `minikube_admission.yaml` file.

    $ cat k8s-consul-mutator-rs.pem | base64 | tr -d '\n'

Next, create the admission controller resource.

    $ minikube kubectl -- apply -f minikube_admission.yaml

9. Create the demo service

    $ minikube kubectl -- create ns demo
    $ minikube kubectl -- label ns/demo k8s-consul-mutator-rs=enabled
    $ minikube kubectl -- apply -f minikube_demo.yaml

You'll see some stuff in the pods logs. Hurray
