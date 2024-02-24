FROM alpine:3 as quake-n-bake

RUN apk add --no-cache git gcc make libc-dev
RUN git clone https://github.com/ioquake/ioq3
RUN cd /ioq3 && make BUILD_MISSIONPACK=0 BUILD_BASEGAME=0 BUILD_CLIENT=0 BUILD_SERVER=1 BUILD_GAME_SO=0 BUILD_GAME_QVM=0 BUILD_RENDERER_OPENGL2=0 BUILD_STANDALONE=1
RUN cp /ioq3/build/release-linux-$(uname -m)/ioq3ded.$(uname -m) /usr/local/bin/ioq3ded

FROM golang:1.21 as builder

WORKDIR /workspace
COPY go.mod go.mod
COPY go.sum go.sum
ARG GOPROXY
ARG GOSUMDB
RUN go mod download

COPY cmd cmd/
COPY internal internal/
COPY pkg pkg/

# Previously, container images were being cross-compiled with buildx which uses
# QEMU for non-native platforms, which has unfortunately had long-running
# issues running the Go compiler:
# * [golang/go#24656](https://github.com/golang/go/issues/24656)
# * [https://bugs.launchpad.net/qemu/+bug/1696773](https://bugs.launchpad.net/qemu/+bug/1696773)
#
# This issue can be circumvented by ensuring that the Go compiler does not run
# across multiple hardware threads by limiting the affinity. I haven't noticed
# it since moving to nix, but keeping this line handy regardless:
#
# RUN CGO_ENABLED=0 GOOS=linux GO111MODULE=on taskset -c 1 /usr/local/go/bin/go build -a -o q3 ./cmd/q3
RUN CGO_ENABLED=0 GOOS=linux GO111MODULE=on go build -a -o q3 ./cmd/q3

RUN CGO_ENABLED=0 go install github.com/grpc-ecosystem/grpc-health-probe@latest

FROM alpine:3

COPY --from=builder /workspace/q3 /usr/local/bin
COPY --from=builder /go/bin/grpc-health-probe /usr/local/bin/grpc-health-probe
COPY --from=quake-n-bake /usr/local/bin/ioq3ded /usr/local/bin
COPY --from=quake-n-bake /lib/ld-musl-*.so.1 /lib

CMD ["/usr/local/bin/q3", "/usr/local/bin/grpc-health-probe"]
