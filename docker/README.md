# Docker

A development docker image is used in CI to build and test the project.

## Development

To build the development image run

```
make development
```

To push to dockerhub, assuming you have push access, run

```
make development-push
```

To run the development image locally (if you don't want to install all the
dependencies yourself), run

```
make development-shell
```
