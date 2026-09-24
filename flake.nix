{
  inputs = {
    v_flakes.url = "github:valeratrades/v_flakes?ref=v1.6";
  };
  outputs = { self, v_flakes }:
    let
      inherit (v_flakes) flake-utils;
    in
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import v_flakes.default_nixpkgs { inherit system; };
        rust = v_flakes.rs.default_nightly system; # scripts/ are `cargo -Zscript`
        github = v_flakes.github {
          inherit pkgs;
          pname = "playbook";
          enable = true;
          lfs = true;
          jobs.errors.augment = [{ name = "flake-app"; args.app = "check"; }];
          containerRelease = { registry = "ghcr.io/service-arb"; };
        };
        combined = v_flakes.utils.combine { inherit rust; modules = [ github ]; };

        mcpPort = "59082";
        # the member surface; build.rs bakes in skill/, structured/ and ref/**/*.md, and nothing else of the tree reaches the image
        mcp = (pkgs.makeRustPlatform { rustc = rust; cargo = rust; }).buildRustPackage {
          pname = "mcp";
          version = "0.1.0";
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./playbook # a workspace member, so cargo reads its manifest
              ./mcp
              ./skill
              ./structured
              (pkgs.lib.fileset.fileFilter (f: f.hasExt "md") ./ref)
            ];
          };
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "mcp" ];
          cargoTestFlags = [ "-p" "mcp" ];
          nativeBuildInputs = [ pkgs.cmake ]; # aws-lc, under reqwest's rustls
          # std's panic locations name its source inside the toolchain, which would pull all of it into the image
          RUSTFLAGS = "--remap-path-prefix=${rust}=/rust";
          disallowedReferences = [ rust ];
          SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"; # the tests build the Google client; the image sets its own
        };
        containerStd = v_flakes.container.implement {
          inherit pkgs;
          pname = "service-arb-mcp";
          containers."" = {
            port = pkgs.lib.toInt mcpPort;
            mounts = [ "/data" ];
            healthPath = "/health";
            criticality = "normal";
            entrypoint = [ "${mcp}/bin/mcp" ];
            workingDir = "/data";
            imageEnv = [ "HOME=/data" "PORT=${mcpPort}" ];
          };
        };
      in
      {
        packages = containerStd.packages // { inherit mcp; };
        containers = containerStd.containers;

        apps.check = {
          type = "app";
          program = "${pkgs.writeShellApplication {
            name = "check";
            runtimeInputs = [ rust pkgs.git ];
            text = "./scripts/call-pull.rs --check && ./scripts/cite-check.rs";
          }}/bin/check";
        };

        devShells.default = pkgs.mkShell {
          shellHook = combined.shellHook;
          packages = [ rust pkgs.sqlite ] ++ combined.enabledPackages;
          env.PORT = mcpPort;
        };
      }
    );
}
