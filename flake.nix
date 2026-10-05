{
  description = "conspectus — AI work graph status tool (dev shell)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # Work tracker CLI (ADR 0109), pinned to the v1.53.0 release. Upstream
    # tags before its CI bumps package.json, so this is the tag plus that
    # bump; a build of the bare tag reports itself as 1.52.0. It keeps its
    # own nixpkgs, which supplies the Bun runtime it is tested against.
    backlog-md = {
      url = "github:MrLesk/Backlog.md/c310b7087c3d8d618520bfe4b9918e1c8bc468c4";
      inputs.flake-utils.follows = "flake-utils";
    };
    # Supplies the Rust toolchain pinned in rust-toolchain.toml (ADR 0110).
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, backlog-md, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
          overlays = [ (import rust-overlay) ];
        };

        # rust-toolchain.toml is the single source of the Rust version
        # (ADR 0110); editors also get the standard library source and
        # rust-analyzer.
        rustToolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        rustTools = [
          rustToolchain
          pkgs.cargo-nextest
        ];

        devTools = with pkgs; [
          git
          gh
          just
          pre-commit
          tmux
          which
        ];

        darwinInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libiconv
        ];

        # Backlog.md ships no x86_64-darwin build.
        backlogTools = pkgs.lib.optionals (backlog-md.packages ? ${system}) [
          backlog-md.packages.${system}.backlog-md
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = rustTools ++ devTools ++ backlogTools ++ darwinInputs;

          shellHook = ''
            echo "conspectus dev shell (system: ${system})"
            echo
            printf "  rustc:  %s\n" "$(rustc --version 2>/dev/null | sed 's/^rustc //')"
            printf "  cargo:  %s\n" "$(cargo --version 2>/dev/null | sed 's/^cargo //')"
            if command -v cargo-nextest >/dev/null 2>&1; then
              printf "  nextest: %s\n" "$(cargo nextest --version 2>/dev/null | head -n1)"
            fi
            if command -v backlog >/dev/null 2>&1; then
              printf "  backlog: %s\n" "$(backlog --version 2>/dev/null)"
            fi
            echo
            echo "Common checks:"
            echo "  cargo fmt -- --check"
            echo "  cargo clippy --all-targets --all-features -- -D warnings"
            echo "  cargo test --all-targets --all-features"
            echo "  cargo nextest run --all-targets --all-features"
            echo "  git diff --check"
          '';
        };
      });
}
