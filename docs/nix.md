---
date: February 9, 2024
---

# Using Nix for Reproducible Development Environments (and Beyond)
Chris Marshall (https://github.com/ChrisRx)

```
          ▗▄▄▄       ▗▄▄▄▄    ▄▄▄▖
          ▜███▙       ▜███▙  ▟███▛ 
           ▜███▙       ▜███▙▟███▛ 
            ▜███▙       ▜██████▛       
     ▟█████████████████▙ ▜████▛     ▟▙ 
    ▟███████████████████▙ ▜███▙    ▟██▙ 
           ▄▄▄▄▖           ▜███▙  ▟███▛
          ▟███▛             ▜██▛ ▟███▛
         ▟███▛               ▜▛ ▟███▛           ░█▀█░▀█▀░█░█
▟███████████▛                  ▟██████████▙     ░█░█░░█░░▄▀▄
▜██████████▛                  ▟███████████▛     ░▀░▀░▀▀▀░▀░▀
      ▟███▛ ▟▙               ▟███▛        
     ▟███▛ ▟██▙             ▟███▛        
    ▟███▛  ▜███▙           ▝▀▀▀▀        
    ▜██▛    ▜███▙ ▜██████████████████▛     
     ▜▛     ▟████▙ ▜████████████████▛     
           ▟██████▙       ▜███▙           
          ▟███▛▜███▙       ▜███▙          
         ▟███▛  ▜███▙       ▜███▙
         ▝▀▀▀    ▀▀▀▀▘       ▀▀▀▘
                              
```

---

## What is Nix?

1. [Operating System](https://nixos.org/)
2. [Package Manager](https://github.com/NixOS/nix)
3. [Programming Language](https://nixos.wiki/wiki/Overview_of_the_Nix_Language)

---

## What is Nix?

1. [Operating System](https://nixos.org/)
2. [Package Manager](https://github.com/NixOS/nix)
3. [Programming Language](https://nixos.wiki/wiki/Overview_of_the_Nix_Language)

### Nix Package Manager

#### Features

* Packages defined through the functional Nix language (`flake.nix`)
* Build and store packages in isolation (`nix build`)
  * Doesn't rely on global packages
  * Allows multiple versions to be installed simultaneously
* Automatically build development environments for packages (`nix develop`)
* Upgrades can rollback (if needed)
* Standalone mode for non-NixOS systems

---

### Nix Package Manager (cont)

#### Installation

[Nix](https://github.com/NixOS/nix) is easy to install in standalone mode on many different platforms:
  * Linux (any distro)
  * macOS
  * Windows (WSL2)

_NOTE: `I'm using it on Pop!_OS and Arch Linux atm`_

There are a couple ways to install. The official nix installer script:

```zsh
sh <(curl -L https://nixos.org/nix/install) --daemon
```

or the Determinate Systems installer:

```zsh
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

(fwiw I used the determinate systems installer)

Both install Nix for multi-user environments (leveraging systemd).

---

### Home Manager

Home Manager enables managing per-user environments. So all of the base packages I use on my system are defined (declaratively) in Home Manager.

#### Installing Home Manager

Home Manager can be installed via Nix in standalone mode:

```zsh
nix run home-manager/master -- init --switch
```
Nix is cool enough to let you install Home Manager as a package, and even cooler that it lets Home Manager manage its own installation. Once initially installed, home-manager configuration updates can be triggered by:

```zsh
home-manager switch
```

(in standalone mode, Home Manager defaults to managing itself)

##### ~/.config/home-manager/home.nix:
```
  # Let Home Manager install and manage itself.
  programs.home-manager.enable = true;
```
#### Home Manager configuration

##### ~/.config/home-manager/home.nix:

[Home Manager Options Search](https://mipmip.github.io/home-manager-option-search/) is helpful for searching through the thousands of options available in Home Manager.

---

### Nix flakes

Flakes are the newer form of Nix packages, and are expressed in a single file. A project can define a Nix `flake.nix` with all of the dependencies necessary to build and run the project successful. Any packages added to `flake.nix` will be instantly available upon opening a development shell:

```zsh
nix develop -c $SHELL
```

#### Using with direnv

Making it even more seamless, Nix can be paired with [direnv](https://direnv.net/) to automatically load/unload the flake development environment as you `cd <project>`:

```zsh
❯ cd quake-kube
direnv: loading ~/src/ChrisRx/quake-kube/.envrc
direnv: using flake
direnv: nix-direnv: using cached dev shell
direnv: export ~CONFIG_SHELL ~GOTOOLDIR ~HOST_PATH ~NIX_BINTOOLS ~NIX_CC ~NIX_CFLAGS_COMPILE ~NIX_LDFLAGS ~PATH ~XDG_DATA_DIRS ~buildInputs ~builder ~nativeBuildInputs ~stdenv
```

This is based upon what is specified in the `.envrc`:

```
use flake .
```

which tells direnv to load the `flake.nix` in that directory.

---
### Nix vs. Docker

They might not seem similar at first, but both provide reproducible environments. However, containers have some pretty big drawbacks:

* Resources are isolated into namespaces which adds complexity to common system abstractions like networking and storage
* Relies on features specific to the Linux kernel (why it runs like crap on macOS and Windows)
* Not running in a higher privileged mode (i.e. rootless) is sometimes difficult

```
                    ##        .            
              ## ## ##       ==            
           ## ## ## ##      ===            
       /""""""""""""""""\___/ ===        
  ~~~ {~~ ~~~~ ~~~ ~~~~ ~~ ~ /  ===- ~~~   
       \______ o          __/            
         \    \        __/             
          \____\______/
```

#### What Nix does better

* Better reproducibility guarantees
  * Nix takes extra steps building environments, such as disabling networking (_no apt-get here_)
* Composability
  * Not restricted to container runtime, can compose with any hypervisor
    * e.g. [astro/microvm.nix](https://github.com/astro/microvm.nix)
  * More flexibility and ease-of-use in CI/CD
* Home Manager is a perfect solution for managing (declaratively) per-user packages
* Flakes are great at handling per-project dependencies
* Paired with direnv, it feels magical

---

### Build containers without `docker`/`podman`

OCI container images can be defined directly in your project and then built with `nix build`:

```Nix
# flake.nix
...
      packages.container = pkgs.dockerTools.buildLayeredImage {
        name = "quake-kube";
        tag = "latest";
        created = "now";
        contents = [
          packages.default
          pkgs.ioquake3
        ];
        config.Cmd = [ "${packages.default}/bin/q3" ];
      };
...
```

This container includes the flake default package and an external dependency from nixpkgs. It can then be built with:

```zsh
nix build .#container
```

---

### Run in GitHub Actions workflows

```yaml
jobs:
  x86_64:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - name: Checkout
        uses: actions/checkout@v3
      - uses: cachix/install-nix-action@v21
      - name: Build
        run: |
          nix build .#container
          skopeo login --username "${{ github.actor }}" --password "${{ secrets.GITHUB_TOKEN }}" ghcr.io
          skopeo copy docker-archive://$(readlink -f ./result) docker://ghcr.io/chrisrx/quake-kube:latest

```

---

# Some rough edges

##### (╯°□°)╯︵ ┻━┻ 

---

### It's a little complicated

The language and tools can be difficult to figure out, as they combine a lot of concepts that might not be intuitive to everyone.

---

### Pinning specific packages

While you can easily pin the version of nixpkgs, pinning specific versions of packages is not always as easy to accomplish. Some are really easy:
* Dependency is broken up into multiple packages to support distinct software lifecycles, e.g. `go_1_21`
* An overlay has been created to allow for, among many other things, easy version selection (such as `rust-overlay`).

However, let's say you want a particular version of protoc plugins, that requires defining an alternative derivation of nixpkgs at the commit with the desired version. There are some (kinda hacky) tools the community has created, such as the [Nix package versions search](https://lazamar.co.uk/nix-versions/), that help finding the right nixpkgs commit. Then your `flake.nix` ends up looking something like this:

```nix
# flake.nix
let
  oldPkgs = import
    (builtins.fetchTarball {
      url = "https://github.com/NixOS/nixpkgs/archive/9957cd48326fe8dbd52fdc50dd2502307f188b0d.tar.gz";
      sha256 = "sha256:1l2hq1n1jl2l64fdcpq3jrfphaz10sd1cpsax3xdya0xgsncgcsi";
    })
    {
      inherit system;
    };
in
  buildInputs = with pkgs; [
    oldPkgs.protoc-gen-go # v1.31.0
    oldPkgs.protoc-gen-go-grpc # v1.3.0
  ];
```

_NOTE: This requires finding the sha256 yourself to keep your flake pure_

---

### Defining Go packages is challenging ʕ•ᴥ•ʔ

Go introduces a few challenges:
* Dependencies in go.sum are a flat list, instead of graph (slow)
* go.mod/go.sum both use a custom file format
* The way Go creates hashes in go.sum is odd (and incompatible)
* It uses networking during the build 

[gomod2nix](https://github.com/nix-community/gomod2nix) is an overlay that helps handle this, but it isn't perfect. It is missing some features and doesn't reuse Go module or build cache so each build is relatively slow (unsure if this can be fixed).

---

#  Further reading

* [My Nix Journey - Use Nix on Ubuntu](https://tech.aufomm.com/my-nix-journey-use-nix-with-ubuntu/)
* [Searching and installing old versions of Nix Packages](https://lazamar.github.io/download-specific-package-version-with-nix/)
* [Building Go Programs With Nix Flakes](https://xeiaso.net/blog/nix-flakes-go-programs/)
* [Generating a docker image with nix](https://fasterthanli.me/series/building-a-rust-service-with-nix/part-11)
* [Tutorial: Getting started with Home Manager for Nix](https://ghedam.at/24353/tutorial-getting-started-with-home-manager-for-nix)
* [Announcing gomod2nix](https://www.tweag.io/blog/2021-03-04-gomod2nix/)
* [Zero-to-Nix](https://zero-to-nix.com/)
* [Using Nix with Dockerfiles](https://mitchellh.com/writing/nix-with-dockerfiles)
