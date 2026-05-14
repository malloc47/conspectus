{
  description = "conspectus — AI work graph status tool (dev shell)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };

        rustTools = with pkgs; [
          cargo
          rustc
          rust-analyzer
          clippy
          rustfmt
          cargo-nextest
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
      in
      {
        devShells.default = pkgs.mkShell {
          packages = rustTools ++ devTools ++ darwinInputs;

          shellHook = ''
            echo "conspectus dev shell (system: ${system})"
            echo
            printf "  rustc:  %s\n" "$(rustc --version 2>/dev/null | sed 's/^rustc //')"
            printf "  cargo:  %s\n" "$(cargo --version 2>/dev/null | sed 's/^cargo //')"
            if command -v cargo-nextest >/dev/null 2>&1; then
              printf "  nextest: %s\n" "$(cargo nextest --version 2>/dev/null | head -n1)"
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
