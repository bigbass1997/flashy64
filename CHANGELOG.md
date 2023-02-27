# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)

## [0.2.0] - 2023-02-27
- Changed: Complete rewrite of crate structure.
- Changed: Replaced `clap` with `bpaf` for argument parsing.
- Added: `Cartridge` trait as common interface for different cartridges
- Added: `Cartridge` implementation for the 64drive (`SixtyFourDrive`)