{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    protobuf
    pkg-config
    openssl
    ollama
    mold          # fast linker; wired via .cargo/config.toml rustflags
  ];

  PROTOC = "${pkgs.protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${pkgs.protobuf}/include";
}
