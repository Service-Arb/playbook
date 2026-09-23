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
          jobs.errors.augment = [{ name = "flake-app"; args.app = "check"; }];
        };
        combined = v_flakes.utils.combine { inherit rust; modules = [ github ]; };
      in
      {
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
          packages = [ rust ] ++ combined.enabledPackages;
        };
      }
    );
}
