# ioquake3, pinned so upstream changes don't break the build, with our patches.
# The dedicated server runs in the game server image, and the web client, built
# with Emscripten, is served by the gateway.
{ lib, fetchFromGitHub }:

let
  version = "1.36-unstable-87a99d1";

  src = fetchFromGitHub {
    owner = "ioquake";
    repo = "ioq3";
    rev = "87a99d18206e878a20289f05ac3c16005dcae5c3";
    hash = "sha256-C4ExSNdyWb1SyHjNygiTXatZjQ9KoeSwUS6nZx91Xms=";
  };

  patches = lib.filesystem.listFilesRecursive ../controllers/client/patches;
in
{
  # Takes the stdenv of the platform the server runs on, so it can be cross
  # compiled.
  server =
    { stdenv, cmake }:
    stdenv.mkDerivation {
      pname = "ioq3ded";
      inherit version src patches;
      nativeBuildInputs = [ cmake ];
      cmakeFlags = [
        "-DBUILD_CLIENT=OFF"
        "-DBUILD_RENDERER_GL1=OFF"
        "-DBUILD_RENDERER_GL2=OFF"
        "-DBUILD_GAME_LIBRARIES=OFF"
        "-DBUILD_GAME_QVMS=OFF"
        # Not a standalone build, so the server also speaks the legacy
        # protocol (68) of original Quake 3, Quake3e and OpenArena clients.
      ];
      buildFlags = [ "ioq3ded" ];
      # ioquake3's x86 assembly uses SSE2, which 32-bit builds don't enable
      # by default.
      env.NIX_CFLAGS_COMPILE = lib.optionalString stdenv.hostPlatform.isi686 "-msse2 -mfpmath=sse";
      installPhase = ''
        runHook preInstall
        install -Dm755 "$(find . -type f -name ioq3ded | head -1)" $out/bin/ioq3ded
        runHook postInstall
      '';
      meta.mainProgram = "ioq3ded";
    };

  # The output is WebAssembly, so it's the same whichever platform builds it.
  webClient =
    {
      stdenvNoCC,
      fetchzip,
      emscripten,
      cmake,
      ninja,
      python3,
    }:
    let
      # Emscripten downloads its SDL2 port while building unless it has a local copy. This
      # is the release Emscripten's version uses.
      sdl2 = fetchzip {
        url = "https://github.com/libsdl-org/SDL/archive/release-2.32.10.zip";
        hash = "sha256-aG8q/z7kGy2CUHe+sR20ze55zMVF5ZqIArgbZB1CJDo=";
      };
    in
    stdenvNoCC.mkDerivation {
      pname = "ioquake3-web";
      inherit version src patches;
      nativeBuildInputs = [
        emscripten
        cmake
        ninja
        python3
      ];
      configurePhase = ''
        runHook preConfigure
        export HOME=$TMPDIR
        # Emscripten writes to its cache, so start from a copy of the packaged one.
        export EM_CACHE=$TMPDIR/emcache
        cp -r ${emscripten}/share/emscripten/cache $EM_CACHE
        chmod -R u+w $EM_CACHE
        export EMCC_LOCAL_PORTS=sdl2=${sdl2}
        # Upstream fixes memory at 256 MB, which mods such as Urban Terror
        # outgrow, so it's allowed to grow.
        emcmake cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release \
          -DCMAKE_EXE_LINKER_FLAGS="-sALLOW_MEMORY_GROWTH=1 -sMAXIMUM_MEMORY=2GB"
        # A second build reads paks on demand, which needs JSPI. JSPI can't
        # suspend through JavaScript frames, so longjmp uses WebAssembly
        # exceptions instead of JavaScript's.
        emcmake cmake -S . -B build-jspi -G Ninja -DCMAKE_BUILD_TYPE=Release \
          -DCMAKE_C_FLAGS="-DQUAKE_KUBE_JSPI -sSUPPORT_LONGJMP=wasm" \
          -DCMAKE_EXE_LINKER_FLAGS="-sALLOW_MEMORY_GROWTH=1 -sMAXIMUM_MEMORY=2GB -sSUPPORT_LONGJMP=wasm -sJSPI -sJSPI_EXPORTS=main,quake_kube_frame"
        runHook postConfigure
      '';
      buildPhase = ''
        runHook preBuild
        cmake --build build --target ioquake3
        cmake --build build-jspi --target ioquake3
        runHook postBuild
      '';
      installPhase = ''
        runHook preInstall
        install -Dm644 -t $out build/Release/ioquake3.js build/Release/ioquake3.wasm
        install -Dm644 -t $out/jspi build-jspi/Release/ioquake3.js build-jspi/Release/ioquake3.wasm
        runHook postInstall
      '';
    };
}
