.DEFAULT_GOAL:=help

BIN_DIR            ?= bin
TOOLS_DIR          := hack/tools
TOOLS_BIN_DIR      := $(TOOLS_DIR)/bin
PROTOC_GEN_GO      := $(TOOLS_BIN_DIR)/protoc-gen-go
PROTOC_GEN_GO_GRPC := $(TOOLS_BIN_DIR)/protoc-gen-go-grpc

BIN_DIR ?= bin
LDFLAGS := -s -w
GOFLAGS = -gcflags "all=-trimpath=$(PWD)" -asmflags "all=-trimpath=$(PWD)"
GO_BUILD_ENV_VARS := GO111MODULE=on CGO_ENABLED=0


##@ Build

.PHONY: build generate docs

build: ## Build q3 binary
	@$(GO_BUILD_ENV_VARS) go build -o $(BIN_DIR)/q3 $(GOFLAGS) -ldflags '$(LDFLAGS)' ./cmd/q3

generate: $(PROTOC_GEN_GO) $(PROTOC_GEN_GO_GRPC) ## Generate Go protobuf files
	protoc \
		--proto_path=proto \
		--go_out=. \
		--go_opt=module=github.com/ChrisRx/quake-kube \
		--go-grpc_out=. \
		--go-grpc_opt=module=github.com/ChrisRx/quake-kube \
		--plugin protoc-gen-go="$(PROTOC_GEN_GO)" \
		$(shell find proto -name '*.proto')

docs: ## Build mdBook docs
	mdbook build docs

dev: ## Run tilt dev environment
	-@ctlptl create cluster kind --registry=ctlptl-registry
	@tilt up --context kind-kind


##@ Helpers

.PHONY: lint test clean help

lint: ## Run golangci-lint linters
	@golangci-lint run

test: ## Run Go tests
	@go test -v ./internal/... ./pkg/...

$(PROTOC_GEN_GO): $(TOOLS_DIR)/go.mod ## Build protoc-gen-go from tools folder.
	cd $(TOOLS_DIR); go build -tags=tools -o bin/protoc-gen-go google.golang.org/protobuf/cmd/protoc-gen-go

$(PROTOC_GEN_GO_GRPC): $(TOOLS_DIR)/go.mod ## Build protoc-gen-go-grpc from tools folder.
	cd $(TOOLS_DIR); go build -tags=tools -o bin/protoc-gen-go-grpc google.golang.org/grpc/cmd/protoc-gen-go-grpc

clean: ## Cleanup the project folders
	@rm -f $(BIN_DIR)/*
	@rm -f $(TOOLS_BIN_DIR)/*
	@rm -f result

help:  ## Display this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)
