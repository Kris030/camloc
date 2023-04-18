# camloc-aruco

This is the client implementation of: [system_protocol.txt](https://github.com/Kris030/camloc/blob/master/system_protocol.txt)

## docker for cross-compilation

### building

Camloc uses the 4.7.0 version of opencv which is not available yet on most stable release distros, so compilation
has to be done on a rolling-release one, like Arch. However, the official Archlinux docker image does not support
arm platforms, so [the archlinuxarm](https://github.com/agners/archlinuxarm-docker) image was used. Note that building
the image does not work with buildkit (refer to: [this](https://github.com/moby/buildkit/issues/1267), and [this](https://stackoverflow.com/questions/63652551/docker-build-vs-docker-run-dont-behave-the-same)), so it needs to be disabled.

```sh
DOCKER_BUILDKIT=0 docker build --platform linux/arm/v7 -t arch-build .
```

### running

```sh
docker run --rm -it -v $PWD:/app arch-build
```
