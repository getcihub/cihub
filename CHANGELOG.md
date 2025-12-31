# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v0.2.1]

### Fixed

* Fixed web installation script to require sudo permissions.

## [v0.2.0]

### Added

* Added support for labels on machines. #16
* Added active installation count metric. #13
* Added new RPC endpoint to indicate runner has started. #17
* Added ability to update resource limits on machines. #13
* Added ability to pause and unpause machines. #12

### Changed

* Improved runner status tracking and reporting. #17
* Updated runner status and reaper method. #17
* Moved machine accept method for better organization. #15

### Fixed

* Fixed runner status not being set correctly. #17
* Fixed Redis service wiring.
* Fixed agent installation script.
* Fixed error handling when GitHub OAuth is invalid.
* Fixed server to use `config.yaml` as default configuration file.

### Removed

* Removed billing functionality. #14
* Removed unhealthy machine state. #11
