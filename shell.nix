{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    protobuf
    pkg-config
    openssl
    ollama
  ];

  PROTOC = "${pkgs.protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${pkgs.protobuf}/include";
}
